"""Trusted stage executors; agents propose code, never acceptance or publication."""
from __future__ import annotations

import contextlib
import fcntl
import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import time

from . import runtime
from .model import digest, within
from .runtime import Failure


class Git:
    def __init__(self, root, valid=lambda: True):
        self.root = pathlib.Path(root).resolve()
        self.valid = valid

    def run(self, *args, check=True, env=None, timeout=120):
        result = runtime.run(["git", *args], self.root, valid=self.valid, timeout=timeout,
                             max_bytes=8388608, env=env)
        if result["truncated"]:
            raise Failure("git output exceeded bound", "authority")
        if check and result["returncode"]:
            raise Failure("git " + args[0] + ": " + result["output"][-4000:], "git")
        return result["output"].strip() if check else result

    def head(self, branch="HEAD"):
        return self.run("rev-parse", branch)


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


def create_workspace(store, job, remote, revision, valid, suffix=""):
    import shutil
    if shutil.disk_usage(store.root).free < store.meta("config")["min_free_bytes"]:
        raise Failure("insufficient free disk for another isolated workspace", "wait")
    root = store.root / "workspaces" / (job["id"]+suffix)
    root.parent.mkdir(exist_ok=True)
    # Always a new workspace per job/lease. A stale worker can never touch a
    # replacement's files. These are separate repositories, not source worktrees.
    root.mkdir(exist_ok=False)
    git = Git(root, valid)
    git.run("init", "--quiet", "--template=")
    git.run("config", "user.name", "OpenCode-RK Delivery")
    git.run("config", "user.email", "delivery@localhost.invalid")
    git.run("config", "core.hooksPath", str(store.root / "empty-hooks"))
    git.run("fetch", "--quiet", "--no-tags", "--no-write-fetch-head", str(remote), revision)
    git.run("checkout", "--quiet", "--detach", revision)
    git.run("config", "gc.auto", "0")
    (root / ".git" / "delivery-owner.json").write_text(json.dumps({"job": job["id"], "token": job["token"]}))
    return root


def changed(git, base):
    # Include new files when checking the scope; no broad staging before this check.
    tracked = git.run("diff", "--name-only", "--no-renames", base, "--").splitlines()
    new = git.run("ls-files", "--others", "--exclude-standard").splitlines()
    return sorted(set(tracked+new))


def check_scope(git, base, task, cfg):
    names = changed(git, base)
    if not names:
        raise Failure("implementation produced no code or tests", "candidate")
    for name in names:
        if any(x.startswith(".env") or x in {".ssh", ".aws", ".netrc"} for x in pathlib.PurePosixPath(name).parts):
            raise Failure("credential-like path is outside ordinary task authority", "authority")
        if not any(within(name, grant) for grant in task["paths"]):
            raise Failure("change outside task grant: " + name, "authority")
        if any(within(name, grant) for grant in cfg["protected_paths"]):
            raise Failure("change to protected policy/evaluator: " + name, "authority")
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
        if found["returncode"] == 0:
            binary = pathlib.Path(found["output"].strip())
            if binary.is_absolute() and binary.is_file() and (binary.parent/"rustc").is_file():
                env["PATH"] = str(binary.parent)+os.pathsep+env.get("PATH", "")
                env["RUSTC"] = str(binary.parent/"rustc")
    return env


def checks(git, commands, cfg, valid, logdir, expect_failure=False, cache_root=None):
    result = []
    initial = git.head()
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
                        or "unittest" in argv or "pytest" in argv or pathlib.Path(argv[0]).name == "pytest")
        if known_runner and not expect_failure:
            counts = re.findall(r"(?:Ran (\d+) tests?|test result: ok\. (\d+) passed|(\d+) passed)", raw["output"])
            if not any(int(n) > 0 for group in counts for n in group if n):
                missing.append("positive executed test count")
        exit_bad = (raw["returncode"] == 0) if expect_failure else (raw["returncode"] != 0)
        if exit_bad or missing or raw["truncated"]:
            raise Failure("check failed: "+check["id"]+"\n"+raw["output"][-10000:], "test")
        if git.head() != initial or git.run("diff", "--name-only", "HEAD", "--"):
            raise Failure("check mutated the tested revision", "authority")
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
    logs = store.root / "evidence" / job["id"]
    logs.mkdir(parents=True, exist_ok=True)
    role = p["stage"]
    if role == "implement":
        base = Git(repo, valid).head(cfg["branch"])
        workspace = create_workspace(store, job, repo, base, valid)
        p["base"] = base
        # Prior diffs are evidence for the optimizer, never blindly applied over a
        # changed canonical tree. The owning task and its goal remain the same.
        if p.get("workspace") and p.get("candidate") and p.get("base_previous"):
            old = Git(p["workspace"], valid)
            old_patch = old.run("diff", "--binary", p["base_previous"], p["candidate"])
            # Only expose prior intent when a repair/rebase actually needs it.
            if len(old_patch) < 200000:
                p["report"] = (p.get("report", "") + "\nPrevious candidate diff:\n" + old_patch)[-60000:]
        output = runtime.agent(p, cfg, workspace, job["route"], valid, logs/"agent.log")
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
                if task.get("red_checks"):
                    if any(not c["required_output"] for c in task["red_checks"]):
                        raise Failure("RED checks require a precise expected failure pattern", "authority")
                    baseline = create_workspace(store, job, p["workspace"], p["base"], valid, "-red")
                    # Explicit baseline probes verify the observed behavior failure;
                    # compiler/import errors do not pass without an owner-specified
                    # assertion pattern. New test authoring belongs to the candidate.
                    red = checks(Git(baseline, valid), task["red_checks"], cfg, valid, logs/"red", expect_failure=True,
                                 cache_root=store.root/"build-cache")
                evidence = checks(git, task["checks"], cfg, valid, logs, cache_root=store.root/"build-cache")
            return {"kind": "tested", "candidate": revision, "checks": evidence, "red_checks": red}
        if role == "review":
            if p.get("author") == job["id"]:
                raise Failure("author cannot review own work", "authority")
            diff = git.run("diff", "--binary", p["base"], revision)
            review_file = workspace / "DELIVERY_REVIEW.diff"
            if review_file.exists():
                raise Failure("reserved review fixture already exists", "authority")
            review_file.write_text(diff)
            before = hashlib.sha256(diff.encode()).hexdigest()
            response = runtime.agent(p, cfg, workspace, job["route"], valid, logs/"agent.log")
            if git.head() != revision or git.run("diff", "--name-only", "HEAD", "--"):
                raise Failure("reviewer edited candidate", "authority")
            if hashlib.sha256(review_file.read_bytes()).hexdigest() != before:
                raise Failure("reviewer changed supplied diff", "authority")
            untracked = git.run("ls-files", "--others", "--exclude-standard").splitlines()
            if set(untracked) - {"DELIVERY_REVIEW.diff"}:
                raise Failure("reviewer created files", "authority")
            return {"kind": "review", "candidate": revision, "reviewer": job["id"],
                    "security": p.get("security_review", False), "verdict": response["verdict"],
                    "findings": response["findings"], "diff_sha256": before}
        response = runtime.agent(p, cfg, workspace, job["route"], valid, logs/"agent.log")
        if git.head() != revision or git.run("diff", "--name-only", "HEAD", "--"):
            raise Failure("diagnostician edited candidate", "authority")
        return {"kind": "diagnosis", "diagnosis": response["diagnosis"], "approach": response["approach"]}
    if role == "prepare":
        current = Git(repo, valid).head(cfg["branch"])
        if current == p["base"]:
            return {"kind": "unchanged", "base": current, "candidate": p["candidate"], "workspace": p["workspace"]}
        workspace = create_workspace(store, job, repo, current, valid)
        git = Git(workspace, valid)
        git.run("fetch", "--quiet", "--no-tags", "--no-write-fetch-head", p["workspace"], p["candidate"])
        attempt = git.run("cherry-pick", "--no-commit", p["candidate"], check=False)
        if attempt["returncode"]:
            return {"kind": "conflict", "error": "candidate conflicts with current main; optimizer must repair\n"+attempt["output"][-5000:]}
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
                                      cache_root=store.root/"build-cache")
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
        guards = checks(source, cfg["guard_checks"], cfg, valid, logs/"guards", cache_root=store.root/"build-cache")
    except Failure as exc:
        raise Failure("canonical guard blocks publication: " + str(exc), "wait") from exc
    workspace = create_workspace(store, job, p["workspace"], p["candidate"], valid)
    git = Git(workspace, valid)
    check_scope(git, p["base"], p["task"], cfg)
    with publication_lock(repo, job, valid):
        problems = source_gate(store, valid)
        if problems:
            raise Failure("; ".join(problems), "wait")
        if source.head(cfg["branch"]) != before:
            return {"kind": "stale", "error": "canonical moved before the publication lock"}
        with heavy_lock(store, valid):
            # Policies are run from canonical, never from worker-modifiable bytes.
            focused = checks(git, p["task"]["checks"], cfg, valid, logs/"focused", cache_root=store.root/"build-cache")
            full = checks(git, cfg["full_checks"], cfg, valid, logs/"full", cache_root=store.root/"build-cache")
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
                post = checks(Git(post_workspace, valid), cfg["post_merge_checks"], cfg, valid, logs/"post",
                              cache_root=store.root/"build-cache")
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
