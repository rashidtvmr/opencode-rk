"""Operator-authorised grant amendment for the retained browser-login task.

``Store.admit`` treats a task specification as immutable, so the four missing
dependency/module grants cannot be added by re-admitting the task.  This module
is the single, separately reviewed operator exception the owner authorised on
2026-11-14: it unions the four approved paths into the existing
``UI-PROVIDER-LOGIN-DISCOVERY-20261114`` grant, reopens the task to ``ready``
for a fresh claim, and preserves every other byte of the retained task -- its
reviews, candidate, changed paths, accepted evidence, report, the registered
feature worktree and all custody files.

It is deliberately a small command, not another scheduler.  It uses only the
repository's *existing* delivery primitives:

* the real ``tools.delivery.store.Store`` (its real schema and transactions);
* the real per-repository supervisor lock (``delivery-supervisor.lock``) and the
  real global integration lock (``recovery-state/integration.lock``);
* the real feature-worktree owner record written by the workspace provisioner
  (``.delivery-owner.json`` at the worktree root, whose real fields are ``base``,
  ``feature_ref``, ``job``, ``leased_at``, ``stage``, ``task``, ``token`` and
  ``workspace``); and
* the real ``tasks``/``jobs`` rows as the ownership fence.

There is no new operational state schema and no caller-supplied operator
context.  The operator evidence (authorisation, target snapshot, trusted runtime
closure and source custody) is pinned by hash, and the live task, owner job,
worktree and custody are revalidated inside one Store transaction.
"""

from __future__ import annotations

import argparse
import contextlib
import fcntl
import hashlib
import importlib.util
import json
import os
import pathlib
import re
import sqlite3
import subprocess
import sys
import time
import types
import uuid

# The local class is useful only for the hermetic unit tests.  The CLI creates
# the Store class from the verified runtime closure below, before opening the
# production state database.
_TRUSTED_STORE_CLASS: type | None = None
model = None


TARGET = "UI-PROVIDER-LOGIN-DISCOVERY-20261114"
ADDITIONS = frozenset(
    {
        "Cargo.toml",
        "Cargo.lock",
        "crates/providers/Cargo.toml",
        "crates/providers/src/chatgpt_browser.rs",
    }
)

# These values are the operator evidence pins for the approved amendment.
AUTH_SHA = "480e738fd55fda5fb6567e54b9f0b27e738f1a2f2e5d7c1e16fe1c75ca731ae0"
TARGET_SHA = "dbf3d82b9a62d9944581b0702875504ac4e2161b7c844fe65a5d55f4bf14f27c"
TARGET_SPEC_SHA = "bfab7abadaa7d2b01a8b1eed0184189d56d99cb79cbf2a7b7d25bc06fbbd282d"
RUNTIME_SHA = "307133ecf318143c1e947170cf4843e317a5f11b"
RUNTIME_MANIFEST_SHA = "0280a70b1e09f9f08b16527f9240fcd5f5c898d447b1b7a9262a079c2049e94c"
BASE = "331980276d97ab4c17d4d0e95b3250a0c98be2ac"
JOB = "6930b4d2d15e4a649c5e9d168a06fa0c"
CUSTODY_SHA = "8dab1906d276faecc61871563d28a318b67cd273ecac7a0ec8293879d0057cc6"

AUTHORIZATION_PATH = (
    "/Users/mymac/Projects/.oc2-features-20261103/artifacts/"
    "ui-parity-autonomous-20261113/resume-20261114/"
    "provider-grant-amendment-authorization.md"
)
TARGET_SNAPSHOT_PATH = (
    "/Users/mymac/Projects/.oc2-features-20261103/artifacts/"
    "ui-parity-autonomous-20261113/resume-20261114/"
    "provider-amendment-target.json"
)
RUNTIME_ROOT = (
    "/Users/mymac/Projects/.oc2-features-20261103/artifacts/"
    "ui-parity-autonomous-20261113/resume-20261114/install-review/"
    "trusted-runtime-307133ec"
)
RUNTIME_MANIFEST_PATH = (
    "/Users/mymac/Projects/.oc2-features-20261103/artifacts/"
    "ui-parity-autonomous-20261113/resume-20261114/install-review/"
    "trusted-runtime-manifest.json"
)
CUSTODY_PATH = (
    "/Users/mymac/Projects/opencode-rk-recovery-lanes/workspaces/evidence/"
    "provider-login-scope-custody-20261114/custody.json"
)

RESERVED_WORKSPACE = (
    "/Users/mymac/Projects/opencode-rk-recovery-lanes/workspaces/author-feature-05"
)
PRODUCTION_STATE = "/Users/mymac/Projects/opencode-rk/.git/delivery-state"
PRODUCTION_REPO = "/Users/mymac/Projects/opencode-rk-recovery-20261005"

_SHA256 = re.compile(r"[0-9a-f]{64}\Z")
_GIT_OID = re.compile(r"[0-9a-f]{40}\Z")


def sha(path: os.PathLike[str] | str) -> str:
    """Return a file digest without accepting directories or links as files."""

    p = pathlib.Path(path)
    if not p.is_file() or p.is_symlink():
        raise ValueError("pinned artifact is not a regular file")
    return hashlib.sha256(p.read_bytes()).hexdigest()


def _git(path: os.PathLike[str] | str, *args: str, check: bool = True) -> str:
    try:
        result = subprocess.run(
            ("git", "-C", str(path), *args),
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise ValueError("git identity unavailable") from exc
    output = result.stdout.strip()
    if check and result.returncode:
        raise ValueError("git identity unavailable")
    return output


def _git_succeeds(path: os.PathLike[str] | str, *args: str) -> bool:
    """Run a quiet identity predicate without confusing empty output for success."""

    try:
        result = subprocess.run(
            ("git", "-C", str(path), *args),
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise ValueError("git identity unavailable") from exc
    return result.returncode == 0


def _git_blob_hash(path: pathlib.Path) -> str:
    try:
        result = subprocess.run(
            ("git", "hash-object", "--", str(path)),
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise ValueError("trusted runtime hash unavailable") from exc
    if result.returncode:
        raise ValueError("trusted runtime hash unavailable")
    return result.stdout.strip()


@contextlib.contextmanager
def _repository_control(repo: os.PathLike[str] | str):
    """Use the delivery controller's existing per-repository writer lock."""

    raw_common = pathlib.Path(_git(repo, "rev-parse", "--git-common-dir"))
    common = (pathlib.Path(repo) / raw_common if not raw_common.is_absolute() else raw_common).resolve()
    if not common.is_dir() or common.is_symlink():
        raise ValueError("repository common directory unavailable")
    lockfile = common / "delivery-supervisor.lock"
    if lockfile.is_symlink():
        raise ValueError("repository control lock is a symlink")
    try:
        with lockfile.open("a+") as handle:
            try:
                fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError as exc:
                raise ValueError("another supervisor owns this repository") from exc
            try:
                yield
            finally:
                fcntl.flock(handle.fileno(), fcntl.LOCK_UN)
    except OSError as exc:
        raise ValueError("repository control lock unavailable") from exc


def _artifact(path: str, expected: str, label: str) -> pathlib.Path:
    raw = pathlib.Path(path)
    if raw.is_symlink():
        raise ValueError(label + " artifact mismatch")
    for parent in raw.parents:
        if parent.is_symlink():
            raise ValueError(label + " artifact mismatch")
    p = raw.resolve()
    if not p.is_file() or p.is_symlink() or sha(p) != expected:
        raise ValueError(label + " artifact mismatch")
    return p


def _safe_relative(path: object) -> bool:
    if not isinstance(path, str) or not path or path.startswith("/") or "\\" in path:
        return False
    if any(char in path for char in "*?[]\n\r\x00"):
        return False
    return all(part not in {"", ".", ".."} for part in path.split("/"))


def _read_json(path: pathlib.Path, label: str) -> dict:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise ValueError(label + " artifact is invalid") from exc
    if not isinstance(value, dict):
        raise ValueError(label + " artifact must be an object")
    return value


def _verify_runtime() -> dict:
    """Verify the complete pinned runtime closure before using Store."""

    manifest_path = _artifact(RUNTIME_MANIFEST_PATH, RUNTIME_MANIFEST_SHA, "runtime manifest")
    manifest = _read_json(manifest_path, "runtime manifest")
    if manifest.get("candidateSha") != RUNTIME_SHA:
        raise ValueError("trusted runtime identity mismatch")
    if manifest.get("all_files_git_blob_verified") is not True:
        raise ValueError("trusted runtime closure is not verified")

    raw_root = pathlib.Path(RUNTIME_ROOT)
    if raw_root.is_symlink():
        raise ValueError("trusted runtime root is unavailable")
    root = raw_root.resolve()
    if not root.is_dir() or root.is_symlink():
        raise ValueError("trusted runtime root is unavailable")
    files = manifest.get("files")
    if not isinstance(files, dict) or not files:
        raise ValueError("trusted runtime closure is empty")
    for relative, entry in files.items():
        if not isinstance(relative, str) or not isinstance(entry, dict):
            raise ValueError("trusted runtime closure entry is invalid")
        if not _safe_relative(relative):
            raise ValueError("trusted runtime closure path is invalid")
        raw_candidate = root / relative
        for parent in (raw_candidate, *raw_candidate.parents):
            if parent == root:
                break
            if parent.is_symlink():
                raise ValueError("trusted runtime closure traverses a symlink")
        candidate = raw_candidate.resolve()
        if root not in candidate.parents or not candidate.is_file() or candidate.is_symlink():
            raise ValueError("trusted runtime closure escapes its root")
        oid = entry.get("oid")
        if not isinstance(oid, str) or not _GIT_OID.fullmatch(oid):
            raise ValueError("trusted runtime closure oid is invalid")
        actual = _git_blob_hash(candidate)
        if actual != oid:
            raise ValueError("trusted runtime closure byte mismatch: " + relative)
    return manifest


def _runtime_store() -> type:
    """Load Store from the already verified runtime closure.

    The import is isolated under a private package name so a caller's
    ``PYTHONPATH`` cannot substitute delivery modules.  Only ``model.py`` and
    ``store.py`` are needed by this command; repository locking remains the
    existing controller lock imported above.
    """

    global _TRUSTED_STORE_CLASS
    _verify_runtime()
    root = pathlib.Path(RUNTIME_ROOT).resolve()
    alias = "_provider_grant_trusted_runtime_" + hashlib.sha256(str(root).encode()).hexdigest()[:16]
    existing = sys.modules.get(alias + ".delivery.store")
    if existing is not None:
        _TRUSTED_STORE_CLASS = existing.Store
        globals()["model"] = sys.modules[alias + ".delivery.model"]
        return _TRUSTED_STORE_CLASS
    package = types.ModuleType(alias)
    package.__path__ = [str(root / "tools")]
    delivery = types.ModuleType(alias + ".delivery")
    delivery.__path__ = [str(root / "tools" / "delivery")]
    sys.modules[alias] = package
    sys.modules[alias + ".delivery"] = delivery
    for name in ("model", "store"):
        full_name = alias + ".delivery." + name
        source = root / "tools" / "delivery" / (name + ".py")
        if not source.is_file() or source.is_symlink():
            raise ValueError("trusted runtime Store module is missing")
        spec = importlib.util.spec_from_file_location(full_name, source)
        if spec is None or spec.loader is None:
            raise ValueError("trusted runtime Store module cannot be loaded")
        module = importlib.util.module_from_spec(spec)
        sys.modules[full_name] = module
        spec.loader.exec_module(module)
    globals()["model"] = sys.modules[alias + ".delivery.model"]
    _TRUSTED_STORE_CLASS = sys.modules[alias + ".delivery.store"].Store
    return _TRUSTED_STORE_CLASS


def _load_target() -> tuple[dict, dict]:
    """Load the operator's target snapshot: the exact retained task identity.

    The snapshot mirrors the real ``tasks`` row (id, generation, spec hash,
    retained workspace and base) rather than inventing a reservation schema the
    delivery Store does not implement.
    """

    if model is None:
        raise ValueError("trusted runtime model is unavailable")
    target_file = _artifact(TARGET_SNAPSHOT_PATH, TARGET_SHA, "target")
    target = _read_json(target_file, "target")
    if target.get("id") != TARGET or type(target.get("generation")) is not int or target.get("generation") != 1:
        raise ValueError("target snapshot identity mismatch")
    if target.get("spec_hash") != TARGET_SPEC_SHA:
        raise ValueError("target specification pin mismatch")
    try:
        spec = json.loads(target["spec"])
    except (KeyError, TypeError, json.JSONDecodeError) as exc:
        raise ValueError("target specification is invalid") from exc
    if not isinstance(spec, dict) or model.digest(spec) != target["spec_hash"]:
        raise ValueError("target specification hash mismatch")
    workspace = target.get("workspace")
    if not isinstance(workspace, str) or not pathlib.Path(workspace).is_absolute():
        raise ValueError("target workspace is invalid")
    if target.get("base") != BASE:
        raise ValueError("target base mismatch")
    if not isinstance(target.get("config_hash"), str) or not _SHA256.fullmatch(target["config_hash"]):
        raise ValueError("target configuration pin is invalid")
    return target, spec


def _load_custody() -> dict:
    custody_file = _artifact(CUSTODY_PATH, CUSTODY_SHA, "custody")
    custody = _read_json(custody_file, "custody")
    if custody.get("task") != TARGET or custody.get("job") != JOB or custody.get("base") != BASE:
        raise ValueError("custody identity mismatch")
    files = custody.get("files")
    if not isinstance(files, dict) or not files:
        raise ValueError("source custody is empty")
    for relative, entry in files.items():
        if not _safe_relative(relative):
            raise ValueError("source custody path is invalid")
        if not isinstance(entry, dict) or not _SHA256.fullmatch(str(entry.get("sha256", ""))):
            raise ValueError("source custody hash is invalid")
    return custody


def _symbolic_ref(path: pathlib.Path) -> str | None:
    result = _git(path, "symbolic-ref", "-q", "HEAD", check=False)
    # ``git symbolic-ref -q`` returns no output for a detached worktree.
    return result or None


def _registered_worktree(path: pathlib.Path) -> tuple[str | None, str]:
    """Return the registered worktree's ref and HEAD, or fail closed."""

    raw = _git(path, "worktree", "list", "--porcelain")
    current: pathlib.Path | None = None
    current_head: str | None = None
    current_ref: str | None = None
    found: tuple[str | None, str] | None = None

    def finish() -> None:
        nonlocal found
        if current is not None and current == path and current_head is not None:
            found = (current_ref, current_head)

    for line in raw.splitlines() + [""]:
        if line.startswith("worktree "):
            finish()
            current = pathlib.Path(line[9:]).resolve()
            current_head = None
            current_ref = None
        elif line.startswith("HEAD "):
            current_head = line[5:].strip()
        elif line.startswith("branch "):
            current_ref = line[7:].strip()
        elif not line:
            finish()
            current = None
            current_head = None
            current_ref = None
    if found is None:
        raise ValueError("registered worktree required")
    return found


def _owner_record(path: pathlib.Path, owner_job: sqlite3.Row, registered_ref: str) -> dict:
    """Validate the real feature-worktree owner record written by the provisioner.

    The provisioner writes ``.delivery-owner.json`` at the worktree root with
    ``base``, ``feature_ref``, ``job``, ``leased_at``, ``stage``, ``task``,
    ``token`` and ``workspace``.  ``leased_at`` and any other audit fields are
    tolerated; the identity fields must match the live consumed owner job.
    """

    owner_file = path / ".delivery-owner.json"
    if owner_file.is_symlink() or not owner_file.is_file():
        raise ValueError("owner record missing")
    try:
        owner = json.loads(owner_file.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise ValueError("owner record invalid") from exc
    if not isinstance(owner, dict):
        raise ValueError("owner record invalid")
    for key, expected_value in (
        ("task", TARGET),
        ("job", JOB),
        ("base", BASE),
        ("token", owner_job["token"]),
        ("workspace", str(path)),
        ("feature_ref", registered_ref),
        ("stage", owner_job["stage"]),
    ):
        if owner.get(key) != expected_value:
            raise ValueError("owner record mismatch: " + key)
    return owner


def _validate_workspace(request: dict, target: dict, custody: dict, owner_job: sqlite3.Row) -> tuple[pathlib.Path, dict]:
    identity = request.get("workspace_identity")
    if not isinstance(identity, dict) or identity.get("registered") is not True:
        raise ValueError("real registration required")
    path_value = identity.get("physical_path")
    if not isinstance(path_value, str):
        raise ValueError("workspace path is missing")
    path = pathlib.Path(path_value)
    if not path.is_absolute() or not path.is_dir() or path != path.resolve():
        raise ValueError("workspace path is not physical")
    for probe in (path, *path.parents):
        if probe.is_symlink():
            raise ValueError("workspace path traverses a symlink")
    expected = pathlib.Path(target["workspace"]).resolve()
    if path != expected or path != pathlib.Path(RESERVED_WORKSPACE).resolve():
        raise ValueError("workspace reservation mismatch")

    dotgit = path / ".git"
    if dotgit.is_symlink() or not dotgit.is_file():
        raise ValueError("registered worktree required")
    common_raw = pathlib.Path(_git(path, "rev-parse", "--git-common-dir"))
    common = (path / common_raw if not common_raw.is_absolute() else common_raw).resolve()
    if not common.is_dir() or common == path / ".git":
        raise ValueError("registered worktree required")
    registered_ref, registered_head = _registered_worktree(path)
    if registered_ref is None:
        raise ValueError("workspace must be a branch worktree")
    symbolic_ref = _symbolic_ref(path)
    if symbolic_ref != registered_ref:
        raise ValueError("workspace ref registration mismatch")
    if identity.get("ref") != registered_ref or identity.get("head") != registered_head:
        raise ValueError("workspace ref or HEAD mismatch")
    if not _git_succeeds(path, "merge-base", "--is-ancestor", BASE, "HEAD"):
        raise ValueError("workspace base mismatch")

    owner = _owner_record(path, owner_job, registered_ref)

    custody_files = custody["files"]
    supplied_hashes = identity.get("source_hashes")
    expected_hashes = {name: entry["sha256"] for name, entry in custody_files.items()}
    if supplied_hashes != expected_hashes:
        raise ValueError("caller custody substitution")
    for relative, entry in custody_files.items():
        candidate = path / relative
        if not candidate.is_file() or candidate.is_symlink() or sha(candidate) != entry["sha256"]:
            raise ValueError("source custody mismatch")
        for probe in (candidate, *candidate.parents):
            if probe == path:
                break
            if probe.is_symlink():
                raise ValueError("source custody traverses a symlink")

    dirty = _git(path, "status", "--porcelain=v1", "--untracked-files=all")
    allowed = set(custody_files) | set(ADDITIONS) | {".delivery-owner.json"}
    for line in dirty.splitlines():
        if not line:
            continue
        name = line[3:]
        if " -> " in name:
            raise ValueError("workspace rename is not retained custody")
        if name not in allowed:
            raise ValueError("workspace has unexpected dirty bytes")
    return path, owner


def _active_lock_value(value: object) -> bool:
    return value not in (None, "", {}, [], False, 0)


def _release_fence(path: pathlib.Path, record: dict) -> None:
    """Remove only our lock inode after rechecking its complete ownership."""

    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        fd = os.open(path, flags)
    except (FileNotFoundError, OSError):
        return
    owned_fd = fd
    try:
        with os.fdopen(fd, "r", encoding="utf-8") as handle:
            owned_fd = -1
            try:
                current = json.load(handle)
            except (OSError, UnicodeError, json.JSONDecodeError):
                return
            if current != record:
                return
            # Re-read ownership and inode immediately before unlinking.  A
            # replacement lock is never removed merely because it reused the
            # pathname.
            try:
                stat_path = os.stat(path, follow_symlinks=False)
                stat_fd = os.fstat(handle.fileno())
            except OSError:
                return
            if (stat_path.st_dev, stat_path.st_ino) != (stat_fd.st_dev, stat_fd.st_ino):
                return
            try:
                latest = json.loads(path.read_text(encoding="utf-8"))
            except (OSError, UnicodeError, json.JSONDecodeError):
                return
            if latest == record:
                with contextlib.suppress(FileNotFoundError):
                    path.unlink()
    finally:
        # fdopen owns the descriptor on the normal path.  This is harmless for
        # the early-return path and prevents a descriptor leak if parsing fails.
        if owned_fd >= 0:
            with contextlib.suppress(OSError):
                os.close(owned_fd)


@contextlib.contextmanager
def _publication_fence(repo: os.PathLike[str] | str, job: str = JOB, task: str = TARGET):
    """Create a crash-recoverable, owner-bound integration fence.

    This is the same ``recovery-state/integration.lock`` the delivery engine's
    ``publication_lock`` uses, so the amendment can never race a publication.
    """

    common_raw = pathlib.Path(_git(repo, "rev-parse", "--git-common-dir"))
    common = (pathlib.Path(repo) / common_raw if not common_raw.is_absolute() else common_raw).resolve()
    if not common.is_dir() or common.is_symlink():
        raise ValueError("repository common directory unavailable")
    directory = common / "recovery-state"
    if directory.exists() and directory.is_symlink():
        raise ValueError("recovery-state directory is a symlink")
    directory.mkdir(mode=0o700, exist_ok=True)
    path = directory / "integration.lock"
    record = {
        "kind": "provider-grant-amendment",
        "task": task,
        "job": job,
        "pid": os.getpid(),
        "nonce": uuid.uuid4().hex,
    }
    flags = os.O_CREAT | os.O_EXCL | os.O_WRONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        fd = os.open(path, flags, 0o600)
    except FileExistsError as exc:
        raise ValueError("global integration lock active") from exc
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            json.dump(record, handle, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        yield record
    finally:
        _release_fence(path, record)


def _validate_request(request: dict, target: dict, runtime: dict) -> None:
    if not isinstance(request, dict) or request.get("task_id") != TARGET:
        raise ValueError("scope is not authorized")
    additive_paths = request.get("additive_paths")
    if not isinstance(additive_paths, list) or not all(isinstance(path, str) for path in additive_paths):
        raise ValueError("scope is not authorized")
    if len(additive_paths) != len(ADDITIONS) or set(additive_paths) != ADDITIONS:
        raise ValueError("scope is not authorized")
    if request.get("authority_evidence") != str(pathlib.Path(AUTHORIZATION_PATH).resolve()):
        raise ValueError("forged authority evidence")
    if request.get("authority_sha256") != AUTH_SHA:
        raise ValueError("authority pin missing")
    if request.get("target_sha256") != TARGET_SHA:
        raise ValueError("target pin missing")
    if request.get("runtime_candidate") != runtime.get("candidateSha"):
        raise ValueError("runtime pin missing")


def _check_task_collisions(cx: sqlite3.Connection, amended: dict) -> None:
    if model is None:
        raise ValueError("trusted runtime model is unavailable")
    for other in cx.execute("SELECT id,spec,state,workspace FROM tasks WHERE id!=?", (TARGET,)):
        # Match the delivery Store's retained-grant rule: an accepted task with
        # no retained workspace is historical, but every other retained row
        # (including blocked/cancelled rows with bytes) remains a collision
        # fence until its custody is disposed by the controller.
        if other["state"] == "accepted" and not other["workspace"]:
            continue
        try:
            other_spec = model.task(json.loads(other["spec"]))
            paths = other_spec["paths"]
        except (TypeError, KeyError, ValueError, json.JSONDecodeError) as exc:
            raise ValueError("retained task specification is invalid") from exc
        if any(model.overlaps(a, b) for a in amended["paths"] for b in paths):
            raise ValueError("retained grant collision")


def amend(store: object, request: dict, *forbidden_context: object) -> dict:
    """Apply the exact operator amendment through one guarded Store transaction."""

    if forbidden_context:
        raise ValueError("caller-supplied operator context is forbidden")
    trusted_store = _runtime_store()
    if model is None:
        raise ValueError("trusted runtime model is unavailable")
    if not isinstance(store, trusted_store):
        raise ValueError("operator Store is invalid")
    if pathlib.Path(store.root).resolve() != pathlib.Path(PRODUCTION_STATE).resolve():
        raise ValueError("foreign Store rejected")
    repo_value = store.meta("repo")
    if not isinstance(repo_value, str) or pathlib.Path(repo_value).resolve() != pathlib.Path(PRODUCTION_REPO).resolve():
        raise ValueError("foreign repository rejected")

    _artifact(AUTHORIZATION_PATH, AUTH_SHA, "authorization")
    target, target_spec = _load_target()
    runtime = _verify_runtime()
    custody = _load_custody()
    _validate_request(request, target, runtime)
    if type(request.get("expected_generation")) is not int or request.get("expected_generation") != target["generation"]:
        raise ValueError("stale generation")
    if request.get("expected_config_hash") != target["config_hash"]:
        raise ValueError("workflow configuration identity mismatch")
    if request.get("expected_spec_hash") != target["spec_hash"]:
        raise ValueError("stale task")

    cfg = store.meta("config")
    if not isinstance(cfg, dict) or model.digest(cfg) != target["config_hash"]:
        raise ValueError("workflow configuration identity mismatch")
    configured_hash = store.meta("config_hash")
    if configured_hash is not None and configured_hash != target["config_hash"]:
        raise ValueError("workflow configuration identity mismatch")

    repo = pathlib.Path(repo_value).resolve()
    with _repository_control(repo), _publication_fence(repo, JOB, TARGET), store.connection(write=True) as cx:
        row = cx.execute("SELECT * FROM tasks WHERE id=?", (TARGET,)).fetchone()
        if not row:
            raise ValueError("target task is missing")
        if (
            row["spec_hash"] != target["spec_hash"]
            or row["spec_hash"] != request.get("expected_spec_hash")
            or row["generation"] != target["generation"]
            or row["state"] != "blocked"
            or row["stage"] != "implement"
        ):
            raise ValueError("stale or non-blocked task")
        try:
            live_spec = json.loads(row["spec"])
        except (TypeError, json.JSONDecodeError) as exc:
            raise ValueError("live task specification is invalid") from exc
        if live_spec != target_spec or model.digest(live_spec) != row["spec_hash"]:
            raise ValueError("live task specification mismatch")

        owner_job = cx.execute(
            "SELECT * FROM jobs WHERE id=? AND task=?", (JOB, TARGET)
        ).fetchone()
        if not owner_job or owner_job["state"] != "consumed" or owner_job["generation"] != target["generation"]:
            raise ValueError("old owner is not consumed")
        # Store.finish/transition intentionally retain pid and lease metadata for
        # auditability.  A consumed row is the lifecycle's terminal owner state;
        # pid/lease values alone are not evidence of a live worker.
        if cx.execute(
            "SELECT 1 FROM jobs WHERE task=? AND state IN ('running','finished')",
            (TARGET,),
        ).fetchone():
            raise ValueError("unconsumed owner job")

        for key in ("publication_barrier", "stop_new_publications", "integration_turn"):
            value_row = cx.execute("SELECT value FROM meta WHERE key=?", (key,)).fetchone()
            if value_row:
                try:
                    value = json.loads(value_row[0])
                except json.JSONDecodeError as exc:
                    raise ValueError("publication/integration metadata is invalid") from exc
                if _active_lock_value(value):
                    raise ValueError("publication/integration lock active")
        if cx.execute("SELECT 1 FROM publications WHERE task=?", (TARGET,)).fetchone():
            raise ValueError("publication journal exists")

        workspace, owner = _validate_workspace(request, target, custody, owner_job)
        if row["base"] not in (None, BASE) or row["workspace"] not in (None, str(workspace)):
            raise ValueError("task does not retain the reserved worktree")

        amended = dict(live_spec, paths=sorted(set(live_spec["paths"]) | ADDITIONS))
        model.task(amended)
        if set(amended["paths"]) != set(live_spec["paths"]) | ADDITIONS:
            raise ValueError("non-additive amendment")
        for key, value in live_spec.items():
            if key != "paths" and amended.get(key) != value:
                raise ValueError("non-path task field changed")
        _check_task_collisions(cx, amended)

        new_hash = model.digest(amended)
        now = time.time()
        updated = cx.execute(
            """UPDATE tasks
               SET spec=?, spec_hash=?, state='ready', stage='implement',
                   generation=generation+1,
                   base=COALESCE(base, ?), workspace=COALESCE(workspace, ?),
                   eligible_at=0, updated=?
               WHERE id=? AND spec_hash=? AND generation=?
                 AND state='blocked' AND stage='implement'""",
            (
                json.dumps(amended, sort_keys=True),
                new_hash,
                BASE,
                str(workspace),
                now,
                TARGET,
                row["spec_hash"],
                row["generation"],
            ),
        )
        if updated.rowcount != 1:
            raise ValueError("replacement race")

        preserved = {
            "reviews": json.loads(row["reviews"]),
            "candidate": row["candidate"],
            "changed_paths": json.loads(row["changed_paths"]),
            "accepted_sha": row["accepted_sha"],
            "report": row["report"],
        }
        body = {
            "old_spec_hash": row["spec_hash"],
            "new_spec_hash": new_hash,
            "old_generation": row["generation"],
            "new_generation": row["generation"] + 1,
            "added_paths": sorted(ADDITIONS),
            "authority": str(pathlib.Path(AUTHORIZATION_PATH).resolve()),
            "runtime": runtime["candidateSha"],
            "owner_job": owner["job"],
            "preserved": preserved,
        }
        store.event(cx, "task.operator_grant_amended", body, TARGET)
        return body


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--request", required=True)
    args = parser.parse_args(argv)
    request_path = pathlib.Path(args.request).resolve()
    try:
        request = json.loads(request_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise SystemExit("invalid amendment request") from exc
    # Verify the audited closure before opening the persistent Store.  The
    # second check in amend is intentional: callers of amend cannot bypass the
    # same production gate used by this CLI entry point.
    trusted_store = _runtime_store()
    store = trusted_store(PRODUCTION_STATE)
    print(json.dumps(amend(store, request), sort_keys=True))


if __name__ == "__main__":
    main()
