"""Trusted stage executors; agents propose code, never acceptance or publication."""
from __future__ import annotations

import contextlib
import fcntl
import hashlib
import json
import math
import os
import pathlib
import re
import shutil
import subprocess
import time

from . import runtime
from .model import digest, overlaps, within
from .runtime import Failure
from .store import WorkspaceSlotContention


class Git:
    def __init__(self, root, valid=lambda: True):
        self.root = pathlib.Path(root).resolve()
        self.valid = valid

    def run(self, *args, check=True, env=None, timeout=120):
        # Never inherit a caller's repository/index selector.  In a linked
        # worktree Git's implicit discovery is correct only while no
        # GIT_DIR/GIT_INDEX_FILE leaks from the parent process.  Bind every
        # trusted invocation to this exact worktree's private gitdir/index.
        gitfile = self.root / ".git"
        if gitfile.is_file():
            text = gitfile.read_text(encoding="utf-8").strip()
            gitdir = pathlib.Path(text.split(":", 1)[1].strip())
            if not gitdir.is_absolute():
                gitdir = (self.root / gitdir).resolve()
        else:
            gitdir = gitfile.resolve()
        bound = runtime.environment()
        if env:
            bound.update({k: v for k, v in env.items() if not k.startswith("GIT_")})
        bound["GIT_DIR"] = str(gitdir)
        # `git worktree add` is an administrative common-repository command.
        # Supplying the caller worktree's index to it makes Git stage/update
        # the source worktree while registering the new linked worktree.  Keep
        # the selector binding for ordinary file/index operations, but let the
        # administrative command use its own worktree/index discovery.
        if args and args[0] in {"worktree", "fetch"}:
            # Administrative common-repository operations must discover their
            # own index/context.  In particular, do not pass even GIT_DIR
            # from a linked worktree into `worktree add`/`fetch`; Git may run
            # checkout/update hooks in that inherited context.
            for key in ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE"):
                bound.pop(key, None)
        else:
            bound.update({"GIT_WORK_TREE": str(self.root),
                          "GIT_INDEX_FILE": str(gitdir / "index")})
        # Commit identity and gc suppression come from the environment, never
        # from `git config`.  Every managed workspace is a linked worktree of the
        # same repository, so all of them write the one shared `.git/config`;
        # with several jobs starting at once, concurrent `git config` calls raced
        # on `.git/config.lock` and failed a job with `could not lock config
        # file ... File exists`.  Environment settings are per-invocation and
        # cannot collide.
        bound.update({"GIT_AUTHOR_NAME": "OpenCode-RK Delivery",
                      "GIT_AUTHOR_EMAIL": "delivery@localhost.invalid",
                      "GIT_COMMITTER_NAME": "OpenCode-RK Delivery",
                      "GIT_COMMITTER_EMAIL": "delivery@localhost.invalid",
                      "GIT_CONFIG_COUNT": "1",
                      "GIT_CONFIG_KEY_0": "gc.auto",
                      "GIT_CONFIG_VALUE_0": "0"})
        result = runtime.run(["git", *args], self.root, valid=self.valid, timeout=timeout,
                             max_bytes=8388608, env=bound)
        if result["truncated"]:
            raise Failure("git output exceeded bound", "authority")
        if check and result["returncode"]:
            raise Failure("git " + args[0] + ": " + result["output"][-4000:], "git")
        return result["output"].strip() if check else result

    def head(self, branch="HEAD"):
        return self.run("rev-parse", branch)

    def try_run(self, *args, env=None, timeout=120):
        return self.run(*args, check=False, env=env, timeout=timeout)


def common_dir(repo):
    raw = Git(repo).run("rev-parse", "--git-common-dir")
    p = pathlib.Path(raw)
    return p.resolve() if p.is_absolute() else (pathlib.Path(repo)/p).resolve()


def source_gate(store, valid=lambda: True):
    """Current product safety, not obsolete legacy worktree census policy."""
    cfg = store.meta("config")
    source = Git(store.meta("repo"), valid)
    issues = []
    if source.run("symbolic-ref", "--quiet", "--short", "HEAD", check=False)["output"].strip() != cfg["branch"]:
        issues.append("canonical worktree is not on the configured branch")
    if source.run("status", "--porcelain=v1", "--untracked-files=normal"):
        issues.append("canonical worktree is dirty; existing work must be preserved")
    current = source.head(cfg["branch"])
    pin = store.meta("policy_base_sha")
    if pin:
        if source.run("merge-base", "--is-ancestor", pin, current, check=False)["returncode"]:
            issues.append("canonical history no longer contains the approved policy base")
        else:
            for name in source.run("diff", "--name-only", pin, current, "--").splitlines():
                if any(within(name, p) for p in cfg["protected_paths"]):
                    issues.append("protected policy changed since owner approval: " + name)
                elif any(within(name, p) for p in cfg["frozen_tests"]):
                    if source.run("cat-file", "-e", pin+":"+name, check=False)["returncode"] == 0:
                        issues.append("existing acceptance test changed since owner approval: " + name)
    last = store.meta("last_verified_sha")
    if last and source.run("merge-base", "--is-ancestor", last, current, check=False)["returncode"]:
        issues.append("canonical history lost a verified commit; explicit reconciliation required")
    if cfg.get("use_recovery_capability"):
        token = common_dir(store.meta("repo")) / "recovery-state" / "canonical-write.token"
        if not token.is_file():
            issues.append("existing Recovery Mode publication capability is missing")
    return issues


def recovery_publication_guard(store, cfg):
    """Fail closed before merge when an installed Recovery policy is active."""
    repo = pathlib.Path(store.meta("repo")).resolve()
    common = common_dir(repo)
    runtime = common / "recovery-state"
    policy_path = runtime / "ref-policy.json"
    hook = common / "hooks" / "reference-transaction"
    policy_present = policy_path.exists()
    hook_present = hook.exists()
    if not policy_present and not hook_present:
        return
    # Partial activation is not an unguarded repository.  Validate the same
    # trusted artifacts/identity that recovery_guard uses before permitting a
    # publication; malformed or ineffective installation fails closed.
    if not policy_path.is_file() or not hook.is_file() or not os.access(hook, os.X_OK):
        raise Failure("Recovery publication guard installation is incomplete", "authority")
    try:
        policy = json.loads(policy_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        raise Failure("Recovery publication policy is malformed", "authority") from exc
    if (policy.get("canonicalHead") != cfg.get("branch") or
            not isinstance(policy.get("mutableHeads"), list) or
            not isinstance(policy.get("protectedPaths"), list) or
            not isinstance(policy.get("canonicalWriteTokenSha256"), str) or
            not policy.get("canonicalWriteTokenSha256")):
        raise Failure("Recovery publication policy is incomplete", "authority")
    identity_policy_path = runtime / "policy.json"
    try:
        identity_policy = json.loads(identity_policy_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        raise Failure("Recovery publication hook identity policy is missing or malformed", "authority") from exc
    expected_wrapper = identity_policy.get("referenceHookSha256")
    expected_driver = identity_policy.get("referenceHookDriverSha256")
    driver = common / "hooks" / "recovery_ref_hook.driver.py"
    if not isinstance(expected_wrapper, str) or not expected_wrapper or not isinstance(expected_driver, str) or not expected_driver:
        raise Failure("Recovery publication hook identity policy is incomplete", "authority")
    if hook.is_symlink() or driver.is_symlink() or not driver.is_file():
        raise Failure("Recovery publication hook identity is unsafe", "authority")
    if hashlib.sha256(hook.read_bytes()).hexdigest() != expected_wrapper:
        raise Failure("Recovery publication wrapper identity mismatch", "authority")
    if hashlib.sha256(driver.read_bytes()).hexdigest() != expected_driver:
        raise Failure("Recovery publication driver identity mismatch", "authority")
    wrapper = hook.read_text(encoding="utf-8", errors="replace")
    if str(driver) not in wrapper:
        raise Failure("Recovery publication wrapper does not bind installed driver", "authority")
    try:
        configured = Git(repo).run("config", "--get", "core.hooksPath", check=False)["output"].strip()
        hook_root = pathlib.Path(configured) if configured else None
        if hook_root is not None and not hook_root.is_absolute():
            hook_root = (repo / hook_root).resolve()
        if hook_root is None or not (hook_root / "reference-transaction").resolve().samefile(hook.resolve()):
            raise Failure("Recovery publication hook is not effective", "authority")
    except OSError as exc:
        raise Failure("Recovery publication hook identity cannot be validated", "authority") from exc
    token = runtime / "canonical-write.token"
    if not cfg.get("use_recovery_capability", False):
        raise Failure("Recovery publication capability is disabled while guarded ref-policy is active", "authority")
    if not token.is_file():
        raise Failure("Recovery publication capability is missing", "authority")



def _ws_cfg(store):
    return (store.meta("config") or {}).get("workspaces") or {}


def _managed_required(store):
    # New managed dispatch requires durable workspace config; no detached fallback.
    cfg = store.meta("config") or {}
    return bool(cfg.get("workspaces"))


def _real(p):
    return pathlib.Path(p).resolve(strict=False)


def _is_relative_to(path, other):
    try:
        path.resolve().relative_to(other.resolve())
        return True
    except Exception:
        return False


def _durable_path_ok(path, repo, common):
    p = _real(path)
    for forbidden in (repo, common):
        if p == forbidden or _is_relative_to(p, forbidden):
            return False
    tmpish = {"/tmp", "/private/tmp"}
    env_roots = {os.environ.get(k) for k in ("TMPDIR", "TEMP", "TMP") if os.environ.get(k)}
    env_roots |= {"/private/var/folders", "/var/folders"}
    home = pathlib.Path.home().resolve()
    cache_roots = {home / "Library" / "Caches"}
    if str(p) in tmpish or any(str(p).startswith(x + "/") for x in tmpish | {str(x) for x in env_roots} | {str(x) for x in cache_roots}):
        return False
    for part in p.parts:
        if part in {".git", "node_modules"}:
            return False
    return True


def _validate_managed_admission(store, stage):
    ws = _ws_cfg(store)
    cfg = store.meta("config") or {}
    if not ws:
        # New managed implementation dispatch must not silently restore detached-clone fallback.
        if stage in {"implement"} and cfg.get("require_managed_workspaces"):
            raise Failure("managed durable workspace configuration required for implementation dispatch", "authority")
        return None
    repo = pathlib.Path(store.meta("repo")).resolve()
    root = _real(ws["root"])
    if not root.is_absolute() or not root.is_dir():
        raise Failure("persistent workspace root must be an existing absolute path", "authority")
    common = common_dir(repo).resolve()
    if not _durable_path_ok(root, repo, common):
        raise Failure("workspace root must be outside canonical/common Git/temp/cache/RAM locations", "authority")
    # durable evidence/build storage: if configured, enforce; if missing, require managed config
    # to explicitly provide durable locations for new dispatch (source/evidence/build).
    evidence_root = ws.get("evidence_root")
    build_root = ws.get("build_root")
    if stage == "implement" and (not evidence_root or not build_root):
        raise Failure("managed implementation requires explicit durable evidence_root and build_root", "authority")
    for name, value in (("evidence_root", evidence_root), ("build_root", build_root)):
        if value is not None:
            vp = _real(value)
            if not vp.is_absolute():
                raise Failure(name + " must be an absolute durable path", "authority")
            if not _durable_path_ok(vp, repo, common):
                raise Failure(name + " must be durable and outside canonical/common Git/temp/cache/RAM locations", "authority")
            vp.mkdir(parents=True, exist_ok=True)
    # missing explicit evidence/build config is preserved as diagnosable in engine/docs; do not silently detach-clone.
    allowed = set(ws.get("allowed_refs", []))
    if not allowed:
        raise Failure("explicit allowed refs are required", "authority")
    author_ref = ws.get("author_ref")
    if author_ref and author_ref not in allowed:
        raise Failure("author ref is not in explicitly allowed existing refs", "authority")
    feature_refs = list(ws.get("feature_refs", []))
    for ref in feature_refs:
        if ref not in allowed:
            raise Failure("feature ref is not in explicitly allowed existing refs", "authority")
    return ws


def _worktree_porcelain(repo):
    out = Git(repo).run("worktree", "list", "--porcelain")
    entries = []
    current = {}
    for line in out.splitlines() + [""]:
        if not line:
            if current:
                entries.append(current)
                current = {}
            continue
        if " " in line:
            k, v = line.split(" ", 1)
            current[k] = v
        else:
            current[line] = True
    return entries


def _registered_worktree_entry(repo, path):
    target = _real(path)
    for entry in _worktree_porcelain(repo):
        wt = entry.get("worktree")
        if wt and _real(wt) == target:
            return entry
    return None


def _repo_identity_matches(repo, path):
    # exact registration in this repository/common git dir only
    entry = _registered_worktree_entry(repo, path)
    if not entry:
        return False
    gitdir = entry.get("gitdir")
    common = common_dir(repo).resolve()
    if gitdir:
        gd = pathlib.Path(gitdir)
        if not gd.is_absolute():
            gd = (pathlib.Path(repo) / gd).resolve()
        else:
            gd = gd.resolve()
        # linked worktree gitdir lives under common git dir/worktrees
        if common not in gd.parents and gd != common:
            # main worktree case
            if gd.resolve() != (pathlib.Path(repo) / ".git").resolve() and gd.resolve() != common:
                return False
    return True


# Controller-owned metadata written into author workspaces. These are not
# author work bytes and must never make a retained workspace look dirty.
_CONTROLLER_OWNED_PATHS = frozenset({
    ".delivery-owner.json",
    ".opencode/opencode.json",
    ".opencode/opencode.jsonc",
})


# Regenerable build-output directories.  They are git-ignored, can never enter
# a candidate without `-f`, and are not author evidence, so a worker that ran a
# build must not make its own retained workspace un-adoptable.
_BUILD_OUTPUT_DIRS = frozenset({
    "target", "node_modules", ".venv", "venv", "__pycache__", ".pytest_cache",
    ".mypy_cache", ".ruff_cache", ".tox", "dist", "build", ".next", ".turbo",
})


def _controller_owned(name):
    return name.strip() in _CONTROLLER_OWNED_PATHS


def _build_output(name):
    stripped = name.strip().rstrip("/")
    if not stripped:
        return False
    return pathlib.PurePosixPath(stripped).parts[0] in _BUILD_OUTPUT_DIRS


def _dirty_state(path, valid, allowed=None):
    """Report whether a retained author workspace carries ambiguous bytes.

    Controller-owned metadata never counts.  When ``allowed`` (the task's own
    granted path set) is supplied, an unstaged/untracked/ignored path that lies
    inside that grant is the task's own in-progress work rather than foreign or
    ambiguous dirt, so it is preserved and adopted instead of refused.  Every
    path outside the grant still refuses, and the subsequent controller
    checkpoint re-validates the exact changed set with ``check_scope``.
    """
    git = Git(path, valid)

    def offending(raw):
        name = raw.strip()
        if " -> " in name:  # rename/copy porcelain: judge both sides
            name = name.split(" -> ", 1)[1].strip()
        if _controller_owned(name):
            return False
        if _build_output(name):
            return False
        if allowed and any(within(name, grant) for grant in allowed):
            return False
        return True

    def names(*args):
        # Read the *raw* porcelain stream: Git.run() strips the whole output,
        # which eats the leading status column of the first line and would
        # shift every path by one byte.  Keep the fixed XY<space> offset by
        # using the unstripped stream here.
        stream = git.try_run(*args)["output"]
        for line in stream.splitlines():
            if len(line) > 3 and line[2] == " ":
                yield line[3:]
            elif len(line) > 2 and line[1] == " ":
                # first line lost its leading status column to output stripping
                yield line[2:]

    for name in names("status", "--porcelain=v1", "--untracked-files=all"):
        if offending(name):
            return True
    for name in names("status", "--porcelain=v1", "--ignored=matching"):
        if offending(name):
            return True
    return False


def _retained_manifest_matches(path, package):
    """Validate a controller-admitted retained package without trusting dirt."""
    root = pathlib.Path(path).resolve()
    if root != pathlib.Path(package["workspace"]).resolve():
        return False
    try:
        actual_common = str(common_dir(root))
        actual_head = Git(root).head("HEAD")
        actual_branch = Git(root).try_run("symbolic-ref", "-q", "HEAD")["output"].strip()
    except Exception:
        return False
    if actual_common != package.get("common_git_dir"):
        return False
    if actual_head != package["source_head"]:
        return False
    if actual_branch != package["feature_ref"]:
        return False
    custody = package.get("custody", package["files"])
    if not isinstance(custody, dict):
        return False
    if any(p.is_symlink() for rel in package["files"] for p in [(root / rel), *list((root / rel).parents)] if p != root):
        return False
    for rel, expected in package["files"].items():
        target = root / rel
        if target.is_symlink() or not target.is_file():
            return False
        working_hash = hashlib.sha256(target.read_bytes()).hexdigest()
        if working_hash != expected:
            return False
        staged = subprocess.run(["git", "ls-files", "--error-unmatch", "--", rel], cwd=root, capture_output=True)
        if staged.returncode == 0:
            try:
                index_bytes = subprocess.check_output(["git", "show", ":" + rel], cwd=root, stderr=subprocess.DEVNULL)
            except subprocess.CalledProcessError:
                return False
            index_hash = hashlib.sha256(index_bytes).hexdigest()
            # A retained package is intentionally allowed to arrive with
            # legitimate unstaged edits.  The linked worktree index may still
            # contain the source checkout's baseline blob, or the controller-
            # approved working blob.  It must not contain a third-party staged
            # payload: accepting any other index value would let hidden staged
            # bytes cross the adoption boundary.
            source_hash = None
            source = subprocess.run(
                ["git", "show", f"{package['source_head']}:{rel}"],
                cwd=root, capture_output=True,
            )
            if source.returncode == 0:
                source_hash = hashlib.sha256(source.stdout).hexdigest()
            if index_hash not in {source_hash, expected}:
                return False
        elif working_hash != expected:
            return False
    for rel, expected in custody.items():
        target = root / rel
        if target.is_symlink() or not target.is_file() or hashlib.sha256(target.read_bytes()).hexdigest() != expected:
            return False
    allowed = set(custody)
    # Git.run() strips leading whitespace, which destroys the two-column
    # porcelain status prefix for an unstaged tracked edit (" M path").  Read
    # the raw stream so the path boundary remains unambiguous.
    status = subprocess.check_output(
        ["git", "status", "--porcelain=v1", "--untracked-files=all", "--ignored"],
        cwd=root, text=True,
    )
    for line in status.splitlines():
        name = line[3:] if len(line) > 3 else ""
        if name != ".delivery-owner.json" and name not in allowed and not any(name.startswith(x + "/") for x in allowed):
            return False
        if name in allowed and name not in package["files"]:
            return False
    return True


def _pinned_red_fixture(task, revision, base, workspace, cfg):
    """Return an owner-pinned RED probe, never a worker-supplied command."""
    auth = (cfg or {}).get("pinned_red_fixture_authorization")
    if not auth:
        return None
    if auth.get("task_id") != task.get("id") or auth.get("base") != base or auth.get("candidate") != revision:
        raise Failure("pinned RED fixture authorization does not match task/base/candidate", "authority")
    try:
        expiry = float(auth.get("expires_at", 0))
    except (TypeError, ValueError) as exc:
        raise Failure("pinned RED fixture authorization expiry is malformed", "authority") from exc
    if not math.isfinite(expiry) or expiry <= time.time():
        raise Failure("pinned RED fixture authorization expired", "authority")
    fixture = pathlib.Path(auth.get("fixture_source", pathlib.Path(workspace) / auth["fixture_path"]))
    if fixture.is_symlink() or not fixture.is_file():
        raise Failure("pinned RED fixture is missing or symlinked", "authority")
    if hashlib.sha256(fixture.read_bytes()).hexdigest() != auth["fixture_sha256"]:
        raise Failure("pinned RED fixture bytes do not match owner authorization", "authority")
    for rel, expected in (auth.get("candidate_production_files") or auth.get("production_files") or {}).items():
        p = pathlib.Path(workspace) / rel
        if p.is_symlink() or not p.is_file() or hashlib.sha256(p.read_bytes()).hexdigest() != expected:
            raise Failure("pinned RED production bytes changed: " + rel, "authority")
    argv = list(auth["argv"])
    # The owner may refer to the pinned fixture by a literal token only; no
    # worker payload or shell interpolation is accepted.
    argv = [str(fixture) if x == "{fixture}" else x for x in argv]
    return {"id": "pinned-red-fixture", "argv": argv,
            "required_output": list(auth["required_output"]),
            "timeout": int(auth.get("timeout", 900))}


def _prepare_pinned_red_workspace(task, revision, base, candidate_workspace, baseline_workspace, cfg):
    """Materialize the sole owner-authorized cfg(test) probe in a -red tree."""
    auth = (cfg or {}).get("pinned_red_fixture_authorization")
    if not auth:
        return None
    probe = _pinned_red_fixture(task, revision, base, candidate_workspace, cfg)
    source = pathlib.Path(candidate_workspace).resolve()
    target_root = pathlib.Path(baseline_workspace).resolve()
    target = target_root / auth["target_path"]
    fixture = target_root / auth["fixture_path"]
    if target.is_symlink() or fixture.is_symlink() or not target.is_file():
        raise Failure("pinned RED injection target is missing or symlinked", "authority")
    candidate_hash = (auth.get("candidate_production_files") or auth.get("production_files") or {}).get(auth["target_path"])
    base_hash = (auth.get("base_production_files") or auth.get("production_files") or {}).get(auth["target_path"])
    if not candidate_hash or not base_hash or hashlib.sha256((source / auth["target_path"]).read_bytes()).hexdigest() != candidate_hash:
        raise Failure("pinned RED injection source production bytes are not owner-pinned", "authority")
    original = target.read_bytes()
    if hashlib.sha256(original).hexdigest() != base_hash:
        raise Failure("pinned RED baseline production bytes changed", "authority")
    fixture_source = pathlib.Path(auth.get("fixture_source", source / auth["fixture_path"]))
    declaration = auth["declaration"]
    expected_declaration = '#[cfg(test)] #[path="' + pathlib.PurePosixPath(auth["fixture_path"]).name + '"] mod ' + auth["module_name"] + ';'
    if declaration.strip() != expected_declaration:
        raise Failure("pinned RED declaration is not the fixed cfg(test) module declaration", "authority")
    if (fixture_source.resolve() == source / auth["target_path"] or
            (target_root / auth["fixture_path"]).exists()):
        raise Failure("pinned RED fixture path aliases an existing source path", "authority")
    if declaration.encode() in original:
        raise Failure("pinned RED declaration already exists in baseline", "authority")
    if hashlib.sha256(fixture_source.read_bytes()).hexdigest() != auth["fixture_sha256"]:
        raise Failure("pinned RED fixture source hash changed", "authority")
    if auth.get("fixture_source_sha256") and auth["fixture_source_sha256"] != auth["fixture_sha256"]:
        raise Failure("pinned RED fixture source manifest hash mismatch", "authority")
    fixture.parent.mkdir(parents=True, exist_ok=True)
    fixture.write_bytes(fixture_source.read_bytes())
    probe["argv"] = [str(fixture) if x == str(fixture_source) or x == "{fixture}" else x for x in probe["argv"]]
    if not declaration.endswith("\n"):
        declaration += "\n"
    if declaration.encode() in original:
        raise Failure("pinned RED declaration already exists in baseline", "authority")
    target.write_bytes(original + declaration.encode())
    if not target.read_bytes().startswith(original) or target.read_bytes() == original:
        raise Failure("pinned RED injection did not preserve production prefix", "authority")
    return probe, {auth["target_path"]: hashlib.sha256(target.read_bytes()).hexdigest(),
                   auth["fixture_path"]: hashlib.sha256(fixture.read_bytes()).hexdigest()}


def _verify_pinned_red_workspace(task, revision, base, candidate_workspace, baseline_workspace, cfg):
    auth = (cfg or {}).get("pinned_red_fixture_authorization")
    if not auth:
        return
    source = pathlib.Path(candidate_workspace).resolve()
    target_root = pathlib.Path(baseline_workspace).resolve()
    target = target_root / auth["target_path"]
    original = source / auth["target_path"]
    if hashlib.sha256(original.read_bytes()).hexdigest() != (auth.get("candidate_production_files") or auth.get("production_files") or {}).get(auth["target_path"]):
        raise Failure("pinned RED source production bytes changed after test", "authority")
    declaration = auth["declaration"].encode().rstrip() + b"\n"
    baseline_bytes = target.read_bytes()
    if not baseline_bytes.rstrip().endswith(declaration.rstrip()):
        raise Failure("pinned RED baseline declaration changed after test", "authority")
    base_bytes = baseline_bytes[:baseline_bytes.rfind(declaration.rstrip())]
    expected_base = (auth.get("base_production_files") or auth.get("production_files") or {}).get(auth["target_path"])
    if hashlib.sha256(base_bytes).hexdigest() != expected_base:
        raise Failure("pinned RED baseline production prefix changed after test", "authority")
    fixture = pathlib.Path(auth.get("fixture_source", target_root / auth["fixture_path"]))
    if hashlib.sha256(fixture.read_bytes()).hexdigest() != auth["fixture_sha256"]:
        raise Failure("pinned RED fixture changed after test", "authority")


def _owner_meta_path(path):
    return pathlib.Path(path) / ".delivery-owner.json"


def _load_owner(path):
    p = _owner_meta_path(path)
    if not p.is_file():
        return None
    try:
        return json.loads(p.read_text())
    except Exception:
        return None


def _revision_authorized(repo, revision, ws, stage, task=None, workspace=None):
    allowed = set(ws.get("allowed_refs", []))
    # base/current revisions must be reachable from allowed refs; candidate revisions may be task-owned commits
    for ref in allowed:
        tip = Git(repo).try_run("rev-parse", "--verify", ref)
        if tip["returncode"] == 0 and tip["output"].strip() == revision:
            return True
    # allow commits reachable from any allowed ref tip
    for ref in allowed:
        if Git(repo).try_run("merge-base", "--is-ancestor", revision, ref)["returncode"] == 0:
            return True
    # allow current HEAD of configured author/feature worktree branch when registered for same task
    if workspace and pathlib.Path(workspace).exists():
        try:
            head = Git(workspace).head("HEAD")
            if head == revision:
                entry = _registered_worktree_entry(repo, workspace)
                branch = entry.get("branch") if entry else None
                if branch and branch in allowed:
                    return True
                owner = _load_owner(workspace) or {}
                if task and owner.get("task") == task:
                    return True
        except Exception:
            pass
    return False


def _select_feature_ref(store, task_id, revision, grant=None, valid=lambda: True):
    ws = _ws_cfg(store)
    refs = list(ws.get("feature_refs") or [])
    if not refs and ws.get("author_ref"):
        refs = [ws["author_ref"]]
    if not refs:
        raise Failure("explicit enumerated feature refs are required for author workspaces", "authority")
    rows = {r["id"]: r for r in store.tasks()}
    root = pathlib.Path(ws["root"]).resolve()
    repo = pathlib.Path(store.meta("repo")).resolve()
    protected = list((store.meta("config") or {}).get("protected_paths") or [])

    def protected_delta(ref, target):
        if ref == target:
            return False
        changed = Git(repo).try_run("diff", "--name-only", target, ref)["output"].splitlines()
        return any(overlaps(path, pinned) for path in changed for pinned in protected)

    # 1) reuse existing owned workspace/ref for same task (exact registration required)
    for ref in refs:
        safe = re.sub(r"[^A-Za-z0-9_.-]", "_", ref.split("/")[-1])
        for path in (root / ("author-" + safe), root / ("author-" + safe + "-" + re.sub(r"[^A-Za-z0-9_.-]", "_", task_id))):
            if not path.exists():
                continue
            owner = _load_owner(path) or {}
            if owner.get("task") == task_id and owner.get("feature_ref") == ref:
                if not _registered_worktree_entry(repo, path):
                    raise Failure("owned author workspace is not an exact registered worktree", "authority")
                # A failed attempt may leave out-of-grant staged/index bytes.
                # Preserve that worktree untouched and allocate a fresh ref for
                # the retry instead of refusing the whole task forever.
                if _dirty_state(path, valid, grant):
                    continue
                if protected_delta(ref, revision):
                    continue
                return ref, path
    # 2) allocate the first entirely unused enumerated slot.
    #
    # An existing author-<ref> path always carries another task's checked-out
    # bytes (its owner metadata names that task). `_ensure_author_worktree`
    # only ever adopts a path whose owner is this exact task, so returning a
    # different-task path here would fail closed with an ownership mismatch and
    # leave the slot unusable. Never silently adopt or reset preserved work:
    # skip every existing path and provision a fresh enumerated ref instead.
    for ref in refs:
        safe = re.sub(r"[^A-Za-z0-9_.-]", "_", ref.split("/")[-1])
        path = root / ("author-" + safe)
        if path.exists():
            if not _registered_worktree_entry(repo, path):
                raise Failure("workspace slot is occupied by an unregistered/ambiguous path", "authority")
            owner = _load_owner(path) or {}
            if owner.get("task") == task_id:
                if _dirty_state(path, valid, grant):
                    continue
                return ref, path
            continue
        # A missing directory does not make an existing branch ref unused. The
        # branch may preserve a rejected candidate (including protected evaluator
        # bytes) that cannot be rewound safely. Reusing it made every retry
        # inherit the old candidate and index; allocate a genuinely new ref.
        if Git(repo).try_run("rev-parse", "--verify", "-q", ref)["returncode"] == 0:
            # Reuse a slot whose branch is already at or behind the requested
            # base, or whose reset cannot touch protected evaluator blobs.
            # Divergent protected history stays preserved and is skipped.
            if (Git(repo).try_run("merge-base", "--is-ancestor", ref, revision)["returncode"] == 0
                    or not protected_delta(ref, revision)):
                return ref, path
            continue
        return ref, path
    raise Failure("no available enumerated feature ref/workspace slot", "wait")


def _require_store_reservation(store, job, feature_ref, path, base, lease_seconds):
    try:
        return store.reserve_workspace(job["task"], job["id"], job["token"], int(job.get("generation") or 0),
                                       lease_seconds, base, feature_ref, str(path))
    except WorkspaceSlotContention:
        raise Failure("workspace slot already reserved", "slot_contention")
    except ValueError as exc:
        if str(exc) == "workspace slot already reserved":
            raise Failure(str(exc), "slot_contention")
        raise Failure("workspace reservation refused: " + str(exc), "authority")
    except Exception as exc:
        raise Failure("workspace reservation refused: " + str(exc), "authority")


def _recheck_store_reservation(store, job, feature_ref, path, base):
    try:
        store.require_workspace_reservation(job["task"], job["id"], job["token"],
                                            feature_ref=feature_ref, path=str(path), base=base)
    except Exception as exc:
        raise Failure("workspace reservation/lease fencing refused mutation: " + str(exc), "authority")


def _ensure_author_worktree(store, job, revision, valid):
    ws = _validate_managed_admission(store, "implement")
    repo = pathlib.Path(store.meta("repo")).resolve()
    recovery_publication_guard(store, store.meta("config"))
    task_id = job["task"]
    payload_task = json.loads(job.get("payload", "{}")).get("task", {}) or {}
    grant = list(payload_task.get("paths") or [])
    retained = payload_task.get("retained_package")
    scoped = (store.meta("config") or {}).get("retained_package_authorization")
    if retained and scoped and scoped.get("task_id") == task_id and scoped.get("workspace") == retained.get("workspace"):
        feature_ref, path = retained["feature_ref"], pathlib.Path(retained["workspace"]).resolve()
        if (feature_ref not in set(ws.get("allowed_refs", [])) or retained.get("base") != revision or
                retained.get("source_head") != scoped.get("source_head") or retained.get("files") != scoped.get("files") or
                str(path) != scoped.get("workspace")):
            raise Failure("retained package base/ref is not authorized", "authority")
        if not _repo_identity_matches(repo, path) or not _registered_worktree_entry(repo, path):
            raise Failure("retained package is not a registered worktree", "authority")
    else:
        if retained is not None:
            raise Failure("retained package does not match its exact owner authorization", "authority")
        feature_ref, path = _select_feature_ref(store, task_id, revision, grant, valid)
    if not _revision_authorized(repo, revision, ws, "implement", task=task_id, workspace=str(path)):
        raise Failure("requested revision/base is not authorized by allowed refs", "authority")
    path.parent.mkdir(parents=True, exist_ok=True)
    retained_mode = bool(retained and scoped and scoped.get("task_id") == task_id and scoped.get("workspace") == str(path.resolve()))
    reservation_base = revision if retained_mode else revision
    lease_seconds = int(ws.get("lease_seconds") or (store.meta("config") or {}).get("lease_seconds") or 900)
    try:
        _require_store_reservation(store, job, feature_ref, path, reservation_base, lease_seconds)
    except Failure as first:
        if first.kind != "slot_contention":
            raise
        # Concurrent implement jobs can observe the same empty slot before
        # either transaction commits.  Retry allocation against the remaining
        # explicitly enumerated refs; Store.reserve_workspace remains the
        # atomic authority and active owners are never stolen.
        refs = list(ws.get("feature_refs") or [])
        for alternate in refs:
            if alternate == feature_ref:
                continue
            safe = re.sub(r"[^A-Za-z0-9_.-]", "_", alternate.split("/")[-1])
            alternate_path = pathlib.Path(ws["root"]).resolve() / ("author-" + safe)
            if alternate_path.exists():
                owner = _load_owner(alternate_path) or {}
                # An existing path without *this* task's owner metadata is
                # either another task's preserved work or an orphaned slot.
                # _ensure_author_worktree refuses both, so never reserve it.
                if owner.get("task") != task_id:
                    continue
            try:
                _require_store_reservation(store, job, alternate, alternate_path, revision, lease_seconds)
                feature_ref, path = alternate, alternate_path
                break
            except Failure as exc:
                if exc.kind != "slot_contention":
                    raise
                continue
        else:
            raise first
    with store.workspace_mutation(task_id, job["id"], job["token"], feature_ref=feature_ref, path=str(path), base=reservation_base):
        entry = _registered_worktree_entry(repo, path)
        branch_name = feature_ref[len("refs/heads/"):] if feature_ref.startswith("refs/heads/") else feature_ref
        _recheck_store_reservation(store, job, feature_ref, path, revision)
        if path.exists():
            if not entry or not _repo_identity_matches(repo, path):
                raise Failure("existing path is not an exact registered worktree of this repository", "authority")
            owner = _load_owner(path)
            retained = json.loads(job.get("payload", "{}")).get("task", {}).get("retained_package")
            scoped = (store.meta("config") or {}).get("retained_package_authorization")
            if not owner:
                if (not retained or not scoped or scoped.get("task_id") != task_id or
                        scoped.get("workspace") != str(path.resolve()) or
                        scoped.get("common_git_dir") != str(common_dir(repo)) or
                        scoped.get("feature_ref") != feature_ref or scoped.get("files") != retained.get("files") or
                        scoped.get("custody") != retained.get("custody")):
                    raise Failure("retained workspace lacks exact controller adoption authority", "authority")
            elif owner.get("task") != task_id:
                raise Failure("author workspace ownership/identity mismatch", "authority")
            elif retained and owner.get("feature_ref") not in {None, feature_ref}:
                raise Failure("existing retained workspace owner ref mismatch", "authority")
            # on-disk token may be from previous valid job for same task; Store reservation is authoritative.
            # refuse if on-disk owner claims a different task (handled above) - stale/replaced job for same task
            # is fenced by reservation rechecks and job validity, not by stale metadata alone.
            if retained:
                if not _retained_manifest_matches(path, retained):
                    raise Failure("retained package exact path/hash manifest does not match workspace", "authority")
            elif _dirty_state(path, valid, grant):
                raise Failure("retained author workspace has staged/unstaged/untracked/ignored bytes; preserve and refuse ambiguous adoption", "authority")
            git = Git(path, valid)
            current_branch = git.try_run("symbolic-ref", "-q", "HEAD")
            expected = feature_ref
            if current_branch["returncode"] != 0 or current_branch["output"].strip() not in {expected, "refs/heads/" + branch_name}:
                raise Failure("author workspace is not on its owned branch/ref", "authority")
            head = git.head("HEAD")
            if head != revision:
                _recheck_store_reservation(store, job, feature_ref, path, revision)
                # byte-preserving: only move when clean and relationship proven; prefer ff-only
                if Git(repo).try_run("merge-base", "--is-ancestor", head, revision)["returncode"] == 0:
                    git.run("merge", "--ff-only", revision)
                elif Git(repo).try_run("merge-base", "--is-ancestor", revision, head)["returncode"] == 0:
                    pass
                elif not _dirty_state(path, valid):
                    # HEAD is this task's previous candidate, which is not an
                    # ancestor of the new canonical base.  Re-aim at canonical
                    # so the author is not forced to re-diverge from a stale
                    # tree (that is what made every prepare conflict recur).
                    # Nothing is lost: the previous candidate commit stays
                    # reachable through the task row and shared object storage,
                    # and its diff is passed to the optimizer as evidence.
                    # Only a strictly clean worktree may move; any uncommitted
                    # byte still refuses adoption below.
                    git.run("reset", "--hard", revision)
                else:
                    raise Failure("cannot move retained author workspace to requested revision without preserving bytes/history", "authority")
        else:
            _recheck_store_reservation(store, job, feature_ref, path, revision)
            try:
                if Git(repo).try_run("rev-parse", "--verify", "-q", feature_ref)["returncode"] == 0:
                    Git(repo, valid).run("worktree", "add", str(path), branch_name)
                else:
                    # Enumerated feature refs may not exist yet; create the owned
                    # branch at the authorized base rather than failing closed.
                    Git(repo, valid).run("worktree", "add", "-b", branch_name, str(path), revision)
            except Failure as exc:
                raise Failure("cannot provision owned feature worktree: " + str(exc), "authority")
            entry = _registered_worktree_entry(repo, path)
            if not entry or not _repo_identity_matches(repo, path):
                raise Failure("provisioned worktree is not exactly registered", "authority")
            git = Git(path, valid)
            # Linked worktrees share the repository config.  Never redirect
            # core.hooksPath here: doing so disables the canonical Recovery
            # Mode reference-transaction hook for every worktree.  Identity and
            # gc suppression come from the environment for the same reason:
            # concurrent `git config` writes collide on `.git/config.lock`.
            # Some Git versions materialize a linked worktree index from the
            # canonical checkout rather than the new branch's tree. Reset the
            # private branch, index and worktree to the authorized base. Slot
            # selection above already proved this cannot change protected
            # evaluator blobs; a new branch at the base makes this a no-op.
            git.run("reset", "--hard", revision)
            head = git.head("HEAD")
            if head != revision:
                _recheck_store_reservation(store, job, feature_ref, path, revision)
                if _dirty_state(path, valid):
                    raise Failure("newly provisioned author workspace is unexpectedly dirty", "authority")
                if Git(repo).try_run("merge-base", "--is-ancestor", head, revision)["returncode"] == 0:
                    git.run("merge", "--ff-only", revision)
                elif Git(repo).try_run("merge-base", "--is-ancestor", revision, head)["returncode"] == 0:
                    pass
                else:
                    # move owned branch to authorized base only when clean (fresh provisioned state)
                    git.run("reset", "--hard", revision)
        _recheck_store_reservation(store, job, feature_ref, path, revision)
        # write/update fenced owner metadata
        meta = {
            "job": job["id"], "token": job["token"], "task": task_id,
            "stage": job.get("stage"), "feature_ref": feature_ref,
            "base": revision, "workspace": str(path.resolve()),
            "leased_at": time.time(),
        }
        _owner_meta_path(path).write_text(json.dumps(meta, sort_keys=True))
        return path


def _ensure_isolated_worktree(store, job, remote, revision, valid, suffix=""):
    ws = _validate_managed_admission(store, job.get("stage", ""))
    repo = pathlib.Path(store.meta("repo")).resolve()
    recovery_publication_guard(store, store.meta("config"))
    if not ws:
        return None
    if not _revision_authorized(repo, revision, ws, job.get("stage", ""), task=job.get("task"), workspace=None):
        # allow revisions present in source repo object db (candidates/commits) when reachable from task workspace/ref context
        if Git(repo).try_run("cat-file", "-e", revision + "^{commit}")["returncode"] != 0:
            raise Failure("requested revision is not available/authorized", "authority")
    root = pathlib.Path(ws["root"]).resolve()
    safe = re.sub(r"[^A-Za-z0-9_.-]", "_", job["id"] + suffix)
    path = root / ("job-" + safe)
    if path.exists():
        raise Failure("isolated workspace path already exists", "authority")
    # immutable isolated workspace: detached worktree from source repo (shared git db), no per-job independent repo
    try:
        Git(repo, valid).run("worktree", "add", "--detach", str(path), revision)
    except Failure:
        # Candidates are committed into the shared object database, so the
        # revision is normally already present.  Only a genuinely absent object
        # needs a fetch, and only from a path that still exists: the recorded
        # author workspace is routinely reclaimed after publication, and trying
        # to fetch from the deleted path turned a missing-object error into a
        # FileNotFoundError that was reported as a route failure.
        if Git(repo, valid).try_run("cat-file", "-e", revision + "^{commit}")["returncode"] != 0:
            if str(remote) != str(repo) and pathlib.Path(str(remote)).exists():
                try:
                    Git(repo, valid).run("fetch", "--quiet", "--no-tags", "--no-write-fetch-head", str(remote), revision)
                except Failure:
                    # `upload-pack` only serves advertised refs, so a bare sha is
                    # rejected even when the object exists. Ask for every head.
                    Git(repo, valid).run("fetch", "--quiet", "--no-tags", "--no-write-fetch-head", str(remote),
                                         "+refs/heads/*:refs/delivery-fetch/*")
            if Git(repo, valid).try_run("cat-file", "-e", revision + "^{commit}")["returncode"] != 0:
                raise Failure("candidate revision is neither present locally nor fetchable", "authority")
        Git(repo, valid).run("worktree", "add", "--detach", str(path), revision)
    entry = _registered_worktree_entry(repo, path)
    if not entry or not _repo_identity_matches(repo, path):
        raise Failure("isolated worktree is not exactly registered", "authority")
    git = Git(path, valid)
    isolated_gitdir = pathlib.Path(git.run("rev-parse", "--absolute-git-dir"))
    if not isolated_gitdir.joinpath("index").exists():
        git.run("read-tree", revision)
    _owner_meta_path(path).write_text(json.dumps({
        "job": job["id"], "token": job["token"], "task": job.get("task"),
        "stage": job.get("stage"), "readonly": True, "workspace": str(path.resolve()),
    }, sort_keys=True))
    return path


def _fetch_source(recorded_workspace, repo):
    """Pick a fetch source that still exists.

    The recorded author workspace is temporary and is routinely reclaimed once
    it is no longer needed. Publication used it unconditionally as its fetch
    source, so a reclaimed workspace surfaced as FileNotFoundError and was
    recorded as a provider route failure that stopped dispatch. A candidate
    authored in a managed worktree already lives in the source repository's
    shared object database, so that is the safe fallback.
    """
    if recorded_workspace and pathlib.Path(recorded_workspace).is_dir():
        return recorded_workspace
    return repo

def create_workspace(store, job, remote, revision, valid, suffix=""):
    if shutil.disk_usage(store.root).free < store.meta("config")["min_free_bytes"]:
        raise Failure("insufficient free disk for another isolated workspace", "wait")
    ws = _ws_cfg(store)
    stage = job.get("stage", "")
    if ws:
        if stage == "implement" and not suffix:
            return _ensure_author_worktree(store, job, revision, valid)
        path = _ensure_isolated_worktree(store, job, remote, revision, valid, suffix)
        if path is not None:
            return path
    # Legacy detached-repo behavior only when managed workspaces not configured and managed mode not required
    cfg = store.meta("config") or {}
    if _ws_cfg(store):
        raise Failure("managed workspace provisioning failed", "authority")
    if stage == "implement" and cfg.get("require_managed_workspaces"):
        raise Failure("managed durable workspace configuration required for implementation dispatch", "authority")
    root = store.root / "workspaces" / (job["id"]+suffix)
    root.parent.mkdir(exist_ok=True)
    root.mkdir(exist_ok=False)
    git = Git(root, valid)
    git.run("init", "--quiet", "--template=")
    git.run("config", "core.hooksPath", str(store.root / "empty-hooks"))
    git.run("fetch", "--quiet", "--no-tags", "--no-write-fetch-head", str(remote), revision)
    git.run("checkout", "--quiet", "--detach", revision)
    (root / ".git" / "delivery-owner.json").write_text(json.dumps({"job": job["id"], "token": job["token"]}))
    return root

def changed(git, base):
    # Include new files when checking the scope; no broad staging before this check.
    tracked = git.run("diff", "--name-only", "--no-renames", base, "--").splitlines()
    new = git.run("ls-files", "--others", "--exclude-standard").splitlines()
    return sorted(set(tracked+new))


def check_scope(git, base, task, cfg):
    # Inspect both the worktree and the index.  A worker can restore a
    # forbidden tracked file in the worktree while leaving its malicious
    # version staged; checking only `diff base` would silently commit that
    # hidden index payload.
    names = sorted(set(
        changed(git, base)
        + git.run("diff", "--cached", "--name-only", "--no-renames", base).splitlines()
    ))
    # Controller ownership metadata is written inside linked author
    # worktrees; it is not candidate content and must never be staged.
    # The runtime also writes its own project permission overlay at
    # .opencode/opencode.json (runtime.overlay); a job killed before its
    # finally-block cleanup leaves it behind, and it must not be mistaken
    # for an out-of-grant agent edit.
    names = [name for name in names
             if name != ".delivery-owner.json"
             and name not in {".opencode/opencode.json", ".opencode/opencode.jsonc"}]
    if not names:
        raise Failure("implementation produced no code or tests", "candidate_empty")
    for name in names:
        if any(x.startswith(".env") or x in {".ssh", ".aws", ".netrc"} for x in pathlib.PurePosixPath(name).parts):
            raise Failure("credential-like path is outside ordinary task authority", "authority")
        if not any(within(name, grant) for grant in task["paths"]):
            raise Failure("change outside task grant: " + name, "authority")
        if any(within(name, grant) for grant in cfg["protected_paths"]):
            scoped = cfg.get("retained_package_authorization")
            retained = task.get("retained_package")
            if (not scoped or not retained or task.get("id") != scoped.get("task_id") or
                    retained.get("workspace") != scoped.get("workspace") or retained.get("files") != scoped.get("files") or retained.get("custody") != scoped.get("custody") or
                    retained.get("base") != scoped.get("base") or retained.get("source_head") != scoped.get("source_head") or
                    retained.get("common_git_dir") != scoped.get("common_git_dir") or retained.get("feature_ref") != scoped.get("feature_ref") or
                    name not in scoped.get("paths", []) or retained.get("files", {}).get(name) is None):
                raise Failure("change to protected policy/evaluator: " + name, "authority")
            if hashlib.sha256((git.root / name).read_bytes()).hexdigest() != retained["files"][name]:
                raise Failure("retained protected bytes changed from authorized manifest", "authority")
        current = git.root / name
        # Reject writes through symlink parents and symlink/submodule additions.
        for p in [current, *list(current.parents)]:
            if p == git.root.parent:
                break
            if p.is_symlink():
                raise Failure("symlink change or traversal: " + name, "authority")
        if any(within(name, grant) for grant in cfg["frozen_tests"]):
            exists = git.run("cat-file", "-e", base+":"+name, check=False)
            if exists["returncode"] == 0:
                raise Failure("existing acceptance test is frozen: " + name, "authority")
    return names


def commit(git, base, task, cfg):
    names = check_scope(git, base, task, cfg)
    # argv values, never shell interpolation; '--' protects names beginning '-'.
    git.run("add", "--all", "--", *names)
    modes = git.run("diff", "--cached", "--raw", "--no-renames", base)
    if any(line.split()[1] in {"120000", "160000"} for line in modes.splitlines() if len(line.split()) > 1):
        raise Failure("symlink or submodule candidate is not permitted", "authority")
    git.run("diff", "--cached", "--check")
    git.run("commit", "--quiet", "-m", "delivery("+task["id"]+"): "+task["goal"][:100])
    return git.head(), names


def _checkpoint_reserved(store, job, workspace, base, task, cfg, feature_ref):
    """Create and bind a candidate commit while the reservation is held.

    This is deliberately controller-owned: the worker supplies bytes only;
    staging, tree/commit creation, and the expected-old ref update are fenced
    by the same Store transaction and the exact reservation identity.
    """
    path = pathlib.Path(workspace).resolve()
    with store.workspace_mutation(job["task"], job["id"], job["token"],
                                  feature_ref=feature_ref, path=str(path), base=base) as reservation:
        def live():
            return (reservation["job_lease_until"] > time.time() and
                    reservation["lease_until"] > time.time())
        if not live():
            raise Failure("workspace reservation expired at checkpoint entry", "authority")
        git = Git(path, lambda: True)
        inherited_index = os.environ.get("GIT_INDEX_FILE")
        if inherited_index:
            raise Failure("managed checkpoint refuses inherited GIT_INDEX_FILE", "authority")
        if os.environ.get("GIT_DIR") or os.environ.get("GIT_WORK_TREE"):
            raise Failure("managed checkpoint refuses inherited repository selector environment", "authority")
        # A linked worktree has its own index.  Make that identity explicit to
        # every trusted checkpoint Git invocation so a caller-inherited
        # GIT_INDEX_FILE cannot redirect staging into the canonical index.
        # Ask Git for the absolute linked-worktree gitdir only after Git.run
        # has installed the trusted selector environment.  This avoids
        # circularly interpreting an inherited GIT_INDEX_FILE or a relative
        # .git target, including macOS /var <-> /private/var aliases.
        gitdir = pathlib.Path(git.run("rev-parse", "--absolute-git-dir")).resolve()
        expected_index = (gitdir / "index").resolve()
        index_path = expected_index
        # A newly provisioned linked worktree may not materialize its private
        # index until the first write.  Identity, not pre-existence, is the
        # safety invariant; `git add` below creates the trusted index.
        canonical_index = (common_dir(path) / "index").resolve()
        if index_path == canonical_index:
            raise Failure("managed checkpoint resolved the canonical index", "authority")
        if git.run("status", "--porcelain=v1", "--untracked-files=all"):
            # Fresh linked worktrees can expose inherited base files as
            # worktree changes when Git's selector is being established; only
            # controller metadata and the task-granted paths are admissible.
            pass
        retained = task.get("retained_package")
        if retained:
            if not _retained_manifest_matches(path, retained):
                raise Failure("retained package changed after adoption", "authority")
            if retained.get("base") != base or retained.get("source_head") != git.head("HEAD"):
                raise Failure("retained source/base identity changed before checkpoint", "authority")
        names = check_scope(git, base, task, cfg)
        if not live():
            raise Failure("workspace reservation expired before staging", "authority")
        git.run("add", "--", *names)
        if not live():
            raise Failure("workspace reservation expired before write-tree", "authority")
        modes = git.run("diff", "--cached", "--raw", "--no-renames", base)
        if any(line.split()[1] in {"120000", "160000"} for line in modes.splitlines() if len(line.split()) > 1):
            raise Failure("symlink or submodule candidate is not permitted", "authority")
        git.run("diff", "--cached", "--check")
        tree = git.run("write-tree")
        if not live():
            raise Failure("workspace reservation expired before commit-tree", "authority")
        message = "delivery(" + task["id"] + "): " + task["goal"][:100]
        candidate = git.run("commit-tree", tree, "-p", base, "-m", message)
        if not live():
            raise Failure("workspace reservation expired before ref update", "authority")
        # The CAS old value must be the CURRENT tip of the task's OWNED ref,
        # not the stage base.  On a retriable re-implement (for example after
        # a review infrastructure failure) the ref legitimately still sits at
        # the task's prior authored checkpoint; anchoring on the moved stage
        # base would reject the ref update or silently skip the checkpoint.
        # The candidate must be a true child of the current tip: this proves
        # provenance from the task's own prior checkpoint and refuses any
        # foreign branch movement instead of adopting an unknown tip.
        current_result = git.try_run("rev-parse", "--verify", feature_ref)
        current = current_result["output"].strip() if current_result["returncode"] == 0 else None
        parent = git.run("rev-parse", "--verify", candidate + "^")
        if current and current != parent.strip():
            # A retry may re-materialize the exact same task-owned checkpoint;
            # accept that identity only when the ref is still the reservation's
            # authored checkpoint.  Never adopt an unrelated tip.
            owned_authored = next((row.get("candidate") for row in store.tasks() if row.get("id") == job["task"]), None)
            # The controller itself writes task-owned commits onto this ref:
            # `_checkpoint_reserved` records a "controller checkpoint" and
            # `prepare` commits the merged tree.  The task row's `candidate`
            # does not track those, so comparing only against it rejects the
            # controller's own bytes and blocks the task permanently.  Accept a
            # tip that is provably this task's own: the exact authored
            # candidate, a commit the controller authored for this task id, or
            # a descendant of the authored candidate.  A foreign tip still
            # fails closed, and the ref update below stays an atomic CAS on
            # `current`, so no other task's bytes are ever clobbered.
            subject = git.run("log", "-1", "--format=%s", current)
            task_owned = subject.startswith("delivery(" + task["id"] + "):")
            descends = bool(owned_authored) and git.try_run(
                "merge-base", "--is-ancestor", owned_authored, current)["returncode"] == 0
            if current != owned_authored and not task_owned and not descends:
                raise Failure("checkpoint ref CAS refused: candidate does not descend from current owned ref tip", "authority")
        git.run("update-ref", feature_ref, candidate, current or base)
        return candidate, names


def test_environment(home, cache, cwd, valid, needs_rust=False):
    """Private test homes/cache; resolve trusted toolchain binaries, not user config."""
    env = runtime.environment()
    home.mkdir(parents=True, exist_ok=True)
    cargo = pathlib.Path(cache)/"cargo"
    cargo.mkdir(parents=True, exist_ok=True)
    env.update({"HOME": str(home), "XDG_DATA_HOME": str(home/"data"),
                "XDG_STATE_HOME": str(home/"state"), "XDG_CONFIG_HOME": str(home/"config"),
                "XDG_CACHE_HOME": str(home/"cache"), "CARGO_HOME": str(cargo),
                "RUSTUP_HOME": str(home/"rustup"), "RUSTUP_AUTO_INSTALL": "0"})
    rustup = shutil.which("rustup") if needs_rust else None
    if rustup:
        # Only this trusted resolver sees the installation location. Test code
        # gets direct toolchain binaries plus a separate, credential-free HOME.
        host = runtime.environment()
        host["RUSTUP_AUTO_INSTALL"] = "0"
        if os.environ.get("RUSTUP_HOME"):
            host["RUSTUP_HOME"] = os.environ["RUSTUP_HOME"]
        found = runtime.run([rustup, "which", "cargo"], cwd, valid=valid,
                            timeout=15, max_bytes=65536, env=host)
        if found["returncode"] != 0:
            listed = runtime.run([rustup, "toolchain", "list"], cwd, valid=valid,
                                 timeout=15, max_bytes=65536, env=host)
            toolchain = next((line.split()[0] for line in listed["output"].splitlines()
                              if line.split() and "(default)" in line), None)
            if toolchain:
                host["RUSTUP_TOOLCHAIN"] = toolchain
                found = runtime.run([rustup, "which", "cargo"], cwd, valid=valid,
                                    timeout=15, max_bytes=65536, env=host)
        if found["returncode"] == 0:
            binary = pathlib.Path(found["output"].strip())
            if binary.is_absolute() and binary.is_file() and (binary.parent/"rustc").is_file():
                env["PATH"] = str(binary.parent)+os.pathsep+env.get("PATH", "")
                env["RUSTC"] = str(binary.parent/"rustc")
    return env


def checks(git, commands, cfg, valid, logdir, expect_failure=False, cache_root=None, allow_mutation=None):
    result = []
    initial = git.head()
    owner_meta = git.root / ".delivery-owner.json"
    owner_snapshot = None
    if os.path.lexists(owner_meta):
        if owner_meta.is_symlink() or not owner_meta.is_file():
            raise Failure("controller owner metadata is not a regular file", "authority")
        owner_snapshot = hashlib.sha256(owner_meta.read_bytes()).hexdigest()
    for check in commands:
        home = logdir / "home"
        required = [check, *cfg.get("full_checks", []), *cfg.get("post_merge_checks", [])]
        needs_rust = any(pathlib.Path(x["argv"][0]).name in {"cargo", "rustc", "rustdoc", "rustfmt"} for x in required)
        env = test_environment(home, cache_root or logdir/"build-cache", git.root, valid, needs_rust)
        raw = runtime.run(check["argv"], git.root, valid=valid, timeout=check["timeout"],
                          max_bytes=cfg["max_output_bytes"], env=env, log=logdir/(check["id"]+".log"))
        missing = [p for p in check["required_output"] if not re.search(p, raw["output"], re.MULTILINE)]
        evidence = {"id": check["id"], "argv": check["argv"], "sha": initial,
                    "returncode": raw["returncode"], "output_sha256": raw["output_sha256"],
                    "duration": raw["duration"], "missing_assertions": missing,
                    "log": str(logdir/(check["id"]+".log"))}
        result.append(evidence)
        argv = check["argv"]
        known_runner = ((pathlib.Path(argv[0]).name == "cargo" and "test" in argv)
                        or (pathlib.Path(argv[0]).name == "rustc" and "--test" in argv)
                        or "unittest" in argv or "pytest" in argv or pathlib.Path(argv[0]).name == "pytest")
        if known_runner and not expect_failure:
            counts = [int(n) for group in re.findall(r"(?:Ran (\d+) tests?|test result: ok\. (\d+) passed|(\d+) passed)", raw["output"]) for n in group if n]
            if not any(n > 0 for n in counts):
                missing.append("positive executed test count")
        if known_runner and expect_failure:
            if re.search(r"error\[E\d+\]|could not compile|aborting due to", raw["output"], re.IGNORECASE):
                missing.append("compiler failure is not RED evidence")
            if not re.search(r"test result: FAILED", raw["output"]):
                missing.append("executed failing test evidence")
        exit_bad = (raw["returncode"] == 0) if expect_failure else (raw["returncode"] != 0)
        if exit_bad or missing or raw["truncated"]:
            raise Failure("check failed: "+check["id"]+"\n"+raw["output"][-10000:], "test")
        changed = set(git.run("diff", "--name-only", "HEAD", "--").splitlines())
        staged = set(git.run("diff", "--cached", "--name-only", "--").splitlines())
        # This exact controller-owned metadata file is created before a managed
        # worker runs. It is not candidate content, but it must remain a regular
        # file with the exact pre-command bytes; staged metadata is still denied.
        if owner_snapshot is not None:
            changed.discard(".delivery-owner.json")
            if (owner_meta.is_symlink() or not owner_meta.is_file() or
                    hashlib.sha256(owner_meta.read_bytes()).hexdigest() != owner_snapshot):
                raise Failure("controller owner metadata changed during check", "authority")
        allowed = allow_mutation or {}
        allowed_paths = set(allowed) if isinstance(allowed, dict) else set(allowed)
        if git.head() != initial or staged or changed - allowed_paths:
            raise Failure("check mutated the tested revision", "authority")
        if allow_mutation is not None:
            untracked = set(git.run("ls-files", "--others", "--exclude-standard").splitlines())
            if owner_snapshot is not None:
                untracked.discard(".delivery-owner.json")
            if untracked - allowed_paths:
                raise Failure("check mutated the tested revision", "authority")
        if isinstance(allowed, dict):
            for rel, expected in allowed.items():
                path = git.root / rel
                if path.is_symlink() or not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != expected:
                    raise Failure("allowed probe bytes changed: " + rel, "authority")
    return result


@contextlib.contextmanager
def heavy_lock(store, valid):
    # Shared with the old Phase-1 judge if this is the canonical repo's state.
    cfg = store.meta("config")
    root = common_dir(store.meta("repo")) / "recovery-state"
    root.mkdir(exist_ok=True)
    path = root / "heavy-validation.lock"
    with path.open("a+") as f:
        while True:
            if not valid():
                raise Failure("lease revoked while waiting for validation", "cancelled")
            try:
                fcntl.flock(f.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except BlockingIOError:
                time.sleep(0.1)
        try:
            yield
        finally:
            fcntl.flock(f.fileno(), fcntl.LOCK_UN)


def _build_cache_root(store):
    ws = _ws_cfg(store)
    if ws and ws.get("build_root"):
        return pathlib.Path(ws["build_root"])
    return store.root / "build-cache"


def stage(store, job):
    cfg = store.meta("config")
    repo = pathlib.Path(store.meta("repo"))
    p = json.loads(job["payload"])
    task = p["task"]
    if digest(task) != p["spec_hash"]:
        raise Failure("task contract hash mismatch", "authority")
    if p.get("config_hash") and p["config_hash"] != store.meta("config_hash"):
        raise Failure("job was issued under another workflow configuration", "authority")
    valid = lambda: store.valid(job["id"], job["token"])
    ws = _ws_cfg(store)
    if ws and ws.get("evidence_root"):
        logs = pathlib.Path(ws["evidence_root"]) / job["id"]
    else:
        logs = store.root / "evidence" / job["id"]
    logs.mkdir(parents=True, exist_ok=True)
    build_cache = _build_cache_root(store)
    role = p["stage"]
    if role == "implement":
        # Author against current canonical, never the base frozen at the first
        # reservation.  `prepare` merges a candidate onto canonical with
        # `merge-tree` and can only fail on a real content conflict, so a base
        # that merely fell behind canonical would make every retry re-author
        # onto the same stale tree and conflict forever.  The prior candidate
        # diff below is handed to the optimizer as evidence, and the previous
        # candidate commit stays in shared object storage, so nothing is lost.
        base = Git(repo, valid).head(cfg["branch"])
        workspace = create_workspace(store, job, repo, base, valid)
        p["base"] = base
        # Prior diffs are evidence for the optimizer, never blindly applied over a
        # changed canonical tree. The owning task and its goal remain the same.
        if (p.get("workspace") and p.get("candidate") and p.get("base_previous")
                and pathlib.Path(p["workspace"]).is_dir()):
            # The prior candidate diff is optional optimizer evidence. A stale
            # or reclaimed workspace path must never crash the job; skip it.
            old = Git(p["workspace"], valid)
            old_patch = old.run("diff", "--binary", p["base_previous"], p["candidate"])
            # Only expose prior intent when a repair/rebase actually needs it.
            if len(old_patch) < 200000:
                p["report"] = (p.get("report", "") + "\nPrevious candidate diff:\n" + old_patch)[-60000:]
        output = runtime.agent(p, cfg, workspace, job["route"], valid, logs/"agent.log")
        ws_cfg = _ws_cfg(store)
        if ws_cfg and _registered_worktree_entry(repo, workspace):
            owner = _load_owner(workspace) or {}
            fref = owner.get("feature_ref")
            if fref:
                _recheck_store_reservation(store, job, fref, workspace, base)
                candidate, names = _checkpoint_reserved(store, job, workspace, base, task, cfg, fref)
            else:
                candidate, names = commit(Git(workspace, valid), base, task, cfg)
        else:
            candidate, names = commit(Git(workspace, valid), base, task, cfg)
        return {"kind": "candidate", "base": base, "candidate": candidate,
                "workspace": str(workspace), "author": job["id"], "changed_paths": names,
                "summary": output.get("summary", "")}
    if role in {"review", "test", "diagnose"}:
        if p.get("candidate") and p.get("workspace"):
            revision, remote = p["candidate"], p["workspace"]
        else:
            revision, remote = Git(repo, valid).head(cfg["branch"]), repo
        workspace = create_workspace(store, job, remote, revision, valid)
        git = Git(workspace, valid)
        if role == "test":
            with heavy_lock(store, valid):
                red = []
                pinned = _pinned_red_fixture(task, revision, p.get("base"), workspace, cfg)
                if pinned:
                    baseline = create_workspace(store, job, p["workspace"], p["base"], valid, "-pinned-red")
                    pinned, allowed_mutation = _prepare_pinned_red_workspace(task, revision, p.get("base"), workspace, baseline, cfg)
                    try:
                        red = checks(Git(baseline, valid), [pinned], cfg, valid, logs/"pinned-red",
                                     expect_failure=True, cache_root=build_cache, allow_mutation=allowed_mutation)
                    finally:
                        _verify_pinned_red_workspace(task, revision, p.get("base"), workspace, baseline, cfg)
                if task.get("red_checks"):
                    if any(not c["required_output"] for c in task["red_checks"]):
                        raise Failure("RED checks require a precise expected failure pattern", "authority")
                    baseline = create_workspace(store, job, p["workspace"], p["base"], valid, "-red")
                    # Explicit baseline probes verify the observed behavior failure;
                    # compiler/import errors do not pass without an owner-specified
                    # assertion pattern. New test authoring belongs to the candidate.
                    red = checks(Git(baseline, valid), task["red_checks"], cfg, valid, logs/"red", expect_failure=True,
                                 cache_root=build_cache)
                evidence = checks(git, task["checks"], cfg, valid, logs, cache_root=build_cache)
            return {"kind": "tested", "candidate": revision, "checks": evidence, "red_checks": red}
        if role == "review":
            if p.get("author") == job["id"]:
                raise Failure("author cannot review own work", "authority")
            diff = git.run("diff", "--binary", p["base"], revision)
            review_file = workspace / "DELIVERY_REVIEW.diff"
            if review_file.exists():
                raise Failure("reserved review fixture already exists", "authority")
            owner_file = _owner_meta_path(workspace)
            owner_before = owner_file.read_bytes() if owner_file.is_file() else None
            owner_identity = _load_owner(workspace) if owner_before is not None else None
            if owner_identity is not None and owner_identity.get("workspace") != str(pathlib.Path(workspace).resolve()):
                raise Failure("review workspace owner metadata is not controller-bound", "authority")
            review_file.write_text(diff)
            before = hashlib.sha256(diff.encode()).hexdigest()
            before_untracked = set(git.run("ls-files", "--others", "--exclude-standard").splitlines())
            before_ignored = set(git.run("ls-files", "--others", "--ignored", "--exclude-standard").splitlines())
            managed_review = bool(_ws_cfg(store))
            if managed_review and (owner_file.is_symlink() or not owner_file.is_file()):
                raise Failure("review workspace controller metadata is not a regular file", "authority")
            response = runtime.agent(p, cfg, workspace, job["route"], valid, logs/"agent.log")
            post_head = git.head()
            if post_head != revision:
                raise Failure("reviewer edited candidate: head=" + post_head + " expected=" + revision, "authority")
            if hashlib.sha256(review_file.read_bytes()).hexdigest() != before:
                raise Failure("reviewer changed supplied diff", "authority")
            if owner_before is not None and (not owner_file.is_file() or owner_file.read_bytes() != owner_before):
                raise Failure("reviewer changed controller-owned metadata", "authority")
            if managed_review and (owner_file.is_symlink() or not owner_file.is_file()):
                raise Failure("reviewer changed controller metadata file identity", "authority")
            untracked = set(git.run("ls-files", "--others", "--exclude-standard").splitlines())
            ignored = set(git.run("ls-files", "--others", "--ignored", "--exclude-standard").splitlines())
            unexpected = (untracked | ignored) - before_ignored - {"DELIVERY_REVIEW.diff", ".delivery-owner.json"}
            # In a linked worktree the candidate's tracked files are visible
            # in the worktree but are not reviewer-created files.  Only reject
            # untracked paths outside the controller review fixture/metadata.
            if unexpected:
                raise Failure("reviewer created files: " + ",".join(sorted(unexpected)), "authority")
            return {"kind": "review", "candidate": revision, "reviewer": job["id"],
                    "security": p.get("security_review", False), "verdict": response["verdict"],
                    "findings": response["findings"], "diff_sha256": before}
        response = runtime.agent(p, cfg, workspace, job["route"], valid, logs/"agent.log")
        if git.head() != revision:
            raise Failure("diagnostician edited candidate", "authority")
        return {"kind": "diagnosis", "diagnosis": response["diagnosis"], "approach": response["approach"]}
    if role == "prepare":
        current = Git(repo, valid).head(cfg["branch"])
        if current == p["base"]:
            return {"kind": "unchanged", "base": current, "candidate": p["candidate"], "workspace": p["workspace"]}
        workspace = create_workspace(store, job, repo, current, valid)
        git = Git(workspace, valid)
        # shared-Git worktrees: candidate commit should already exist in common object db.
        # Never fetch from the workspace path as a self-remote dependency when local.
        have = git.try_run("cat-file", "-e", p["candidate"] + "^{commit}")
        if have["returncode"] != 0:
            # try fetching from source repo/common db path first
            fetched = git.run("fetch", "--quiet", "--no-tags", "--no-write-fetch-head", str(repo), p["candidate"], check=False)
            if fetched["returncode"] != 0 and p.get("workspace") and str(pathlib.Path(p["workspace"]).resolve()) != str(git.root.resolve()):
                git.run("fetch", "--quiet", "--no-tags", "--no-write-fetch-head", p["workspace"], p["candidate"])
        have = git.try_run("cat-file", "-e", p["candidate"] + "^{commit}")
        if have["returncode"] != 0:
            raise Failure("candidate commit is not available in shared Git storage", "git")
        # Recovery Mode's reference-transaction hook denies any real AUTO_MERGE
        # ref write, which `cherry-pick --no-commit` always performs. Compute the
        # merged tree in the object database instead (`merge-tree --write-tree`):
        # no ref is touched, exit 0 yields the merged tree, exit 1 yields the
        # conflict stages the optimizer needs.
        merged = git.run("merge-tree", "--write-tree", current, p["candidate"], check=False)
        if merged["returncode"]:
            return {"kind": "conflict", "error": "candidate conflicts with current main; optimizer must repair\n"+merged["output"][-5000:]}
        tree = merged["output"].splitlines()[0].strip() if merged["output"].strip() else ""
        if not re.fullmatch(r"[0-9a-f]{40,64}", tree):
            raise Failure("merge-tree did not report a merged tree", "git")
        git.run("read-tree", "--reset", "-u", tree)
        candidate, names = commit(git, current, task, cfg)
        return {"kind": "prepared", "base": current, "candidate": candidate,
                "workspace": str(workspace), "changed_paths": names}
    if role == "postcheck":
        previous_id = p.get("report", "")
        journal = store.publication(previous_id)
        if not journal or journal["candidate_sha"] != p["candidate"]:
            raise Failure("postcheck lacks an exact publication journal", "authority")
        if Git(repo, valid).head(cfg["branch"]) != p["candidate"]:
            raise Failure("canonical changed since interrupted publication", "ambiguous")
        workspace = create_workspace(store, job, repo, p["candidate"], valid)
        with publication_lock(repo, job, valid):
            with heavy_lock(store, valid):
                try:
                    evidence = checks(Git(workspace, valid), cfg["post_merge_checks"], cfg, valid, logs,
                                      cache_root=build_cache)
                except Failure as exc:
                    store.publication(previous_id, "postcheck_failed")
                    store.meta("stop_new_publications", {"task": job["task"], "sha": p["candidate"], "error": str(exc)})
                    return {"kind": "regression", "candidate": p["candidate"], "error": str(exc)}
                if Git(repo, valid).head(cfg["branch"]) != p["candidate"]:
                    raise Failure("canonical moved during recovered postchecks", "ambiguous")
                store.publication(previous_id, "verified")
                store.meta("last_verified_sha", p["candidate"])
                return {"kind": "accepted", "candidate": p["candidate"], "post": evidence, "reconciled": True}
    if role == "publish":
        return publish(store, job, p, cfg, valid, logs)
    raise Failure("unknown stage", "authority")


@contextlib.contextmanager
def publication_lock(repo, job, valid):
    root = common_dir(repo) / "recovery-state"
    root.mkdir(exist_ok=True)
    p = root / "integration.lock"
    try:
        fd = os.open(str(p), os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except FileExistsError:
        raise Failure("an integration lock already exists; reconcile rather than stealing it", "wait")
    data = {"owner": "delivery:"+job["id"], "failure": job["task"], "job": job["id"], "acquiredAt": time.time()}
    with os.fdopen(fd, "w") as f:
        json.dump(data, f)
        f.flush(); os.fsync(f.fileno())
    try:
        if not valid():
            raise Failure("lease revoked before publication", "cancelled")
        yield p
    finally:
        # Remove only this job's lock, never a replacement owner's lock.
        with contextlib.suppress(OSError, ValueError):
            if json.loads(p.read_text()).get("job") == job["id"]:
                p.unlink()


def publish(store, job, p, cfg, valid, logs):
    repo = pathlib.Path(store.meta("repo"))
    source = Git(repo, valid)
    build_cache = _build_cache_root(store)
    build_cache = _build_cache_root(store)
    before = source.head(cfg["branch"])
    if before != p["base"]:
        return {"kind": "stale", "error": "canonical moved before integration"}
    if source.run("symbolic-ref", "--quiet", "--short", "HEAD") != cfg["branch"]:
        raise Failure("canonical working tree is not on configured branch", "wait")
    if source.run("status", "--porcelain=v1", "--untracked-files=normal"):
        raise Failure("canonical working tree is dirty; no stash/reset/auto-commit performed", "wait")
    if not p.get("publish_enabled"):
        raise Failure("publication was not explicitly enabled", "wait")
    reviews = p.get("reviews", [])
    required = p["task"]["reviews"]
    if len({r.get("reviewer") for r in reviews if r.get("candidate") == p["candidate"] and r.get("verdict") == "APPROVE"}) < required:
        raise Failure("exact-candidate independent approvals are missing", "authority")
    if any(r.get("reviewer") == p.get("author") or r.get("verdict") != "APPROVE" for r in reviews):
        raise Failure("self/negative review blocks publication", "authority")
    if p["task"]["security_review"] and not any(r.get("security") for r in reviews):
        raise Failure("independent security review missing", "authority")
    try:
        guards = checks(source, cfg["guard_checks"], cfg, valid, logs/"guards", cache_root=build_cache)
    except Failure as exc:
        raise Failure("canonical guard blocks publication: " + str(exc), "wait") from exc
    workspace = create_workspace(store, job, _fetch_source(p.get("workspace"), repo), p["candidate"], valid)
    git = Git(workspace, valid)
    check_scope(git, p["base"], p["task"], cfg)
    with publication_lock(repo, job, valid):
        recovery_publication_guard(store, cfg)
        problems = source_gate(store, valid)
        if problems:
            raise Failure("; ".join(problems), "wait")
        if source.head(cfg["branch"]) != before:
            return {"kind": "stale", "error": "canonical moved before the publication lock"}
        with heavy_lock(store, valid):
            # Policies are run from canonical, never from worker-modifiable bytes.
            focused = checks(git, p["task"]["checks"], cfg, valid, logs/"focused", cache_root=build_cache)
            full = checks(git, cfg["full_checks"], cfg, valid, logs/"full", cache_root=build_cache)
            if source.head(cfg["branch"]) != before:
                return {"kind": "stale", "error": "canonical moved during combined validation"}
            if source.run("status", "--porcelain=v1", "--untracked-files=normal"):
                raise Failure("canonical became dirty during validation", "wait")
            if source.run("symbolic-ref", "--quiet", "--short", "HEAD") != cfg["branch"]:
                raise Failure("canonical branch changed during validation", "wait")
            if not valid():
                raise Failure("publication lease expired", "cancelled")
            evidence = {"spec_hash": p["spec_hash"], "candidate": p["candidate"],
                        "reviews": reviews, "focused": focused, "full": full, "guards": guards}
            store.publish_intent(job["id"], job["task"], before, p["candidate"], evidence)
            source.run("fetch", "--quiet", "--no-tags", "--no-write-fetch-head", str(workspace), p["candidate"])
            env = runtime.environment()
            # Optional existing Recovery Mode capability: only the trusted git
            # writer receives it. No hook is disabled and no registry is faked.
            token = common_dir(repo) / "recovery-state" / "canonical-write.token"
            if token.is_file() and cfg.get("use_recovery_capability", False):
                env["OC2_RECOVERY_CANONICAL_TOKEN"] = token.read_text().strip()
            source.run("merge", "--ff-only", "--no-edit", p["candidate"], env=env)
            if source.head(cfg["branch"]) != p["candidate"]:
                raise Failure("publication ended at an unexpected SHA; reconciliation required", "ambiguous")
            store.publication(job["id"], "landed")
            try:
                post_workspace = create_workspace(store, job, repo, p["candidate"], valid, "-post")
                # `full` and `post` run the same `cargo test --workspace` against
                # the shared build_root.  A cargo run that finishes while the next
                # one starts can replace a test binary between spawn and exec,
                # which surfaces as `could not execute process ... (never
                # executed)` on an otherwise passing tree (seen three times).
                # Give postchecks their own target dir so the two phases cannot
                # race on the same artifacts.
                post = checks(Git(post_workspace, valid), cfg["post_merge_checks"], cfg, valid, logs/"post",
                              cache_root=build_cache/"post")
            except Failure as exc:
                store.publication(job["id"], "postcheck_failed")
                store.meta("stop_new_publications", {"task": job["task"], "sha": p["candidate"], "error": str(exc)})
                return {"kind": "regression", "candidate": p["candidate"], "error": str(exc)}
            if source.head(cfg["branch"]) != p["candidate"]:
                raise Failure("canonical moved during postchecks; reconcile journal", "ambiguous")
            if not valid():
                raise Failure("lease expired after publication; reconcile journal", "ambiguous")
            store.publication(job["id"], "verified")
            store.meta("last_verified_sha", p["candidate"])
            return {"kind": "accepted", "candidate": p["candidate"], "post": post,
                    "evidence": str(logs), "before": before}


def reconcile_publication(store, previous, valid):
    """Re-run missing postchecks after a crash; never blindly merge again."""
    journal = store.publication(previous["id"])
    if not journal:
        return {"kind": "retry", "error": "no publication intent existed"}
    cfg = store.meta("config")
    repo = store.meta("repo")
    current = Git(repo, valid).head(cfg["branch"])
    if current == journal["before_sha"]:
        if journal["state"] != "intent":
            return {"kind": "blocked", "error": "journal proves publication already occurred but canonical was rewound; do not replay"}
        return {"kind": "retry", "error": "publication did not move canonical"}
    if current != journal["candidate_sha"]:
        return {"kind": "blocked", "error": "canonical is neither journal before nor candidate; manual reconciliation required"}
    if journal["state"] == "verified":
        return {"kind": "accepted", "candidate": current, "reconciled": True}
    return {"kind": "recheck", "candidate": current, "error": "candidate landed; postchecks must run without republishing"}
