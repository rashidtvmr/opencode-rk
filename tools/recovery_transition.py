#!/usr/bin/env python3
"""One-use, blob-bound controller transition for the UI phase authority delta.

PROPOSED NEW MODULE (not present on canonical main-v2 today).

This artifact has NOT been approved or activated. It requires fresh independent
security and test review of these exact blobs at base
the canonical SHA resolved by ``prepare`` before any human operator runs it
against the live repository.  (Historical note: the preserved v0 proposal at
base ``3eac63f97bc43ab93e7d185b19d738a4f268f8e6`` is superseded by this
current-base restatement; no historical receipt metadata was modified.)

Two distinct phases:

1. **Human-operated bootstrap** (one time): no existing Recovery primitive can
   amend ``config/recovery-mode.json`` or ``tools/recovery_ref_hook.py``
   (``bootstrapMutable=false``; claim/review/integrate never grant that). The
   operator must (a) obtain two independent review receipts — one ``security``,
   one ``test``, neither authored by the transition author, each bound to this
   capability's digest — produced and persisted by those reviewers' own
   sessions, then (b) run ``prepare`` / ``record-receipts`` / ``verify`` /
   ``activate`` by hand while the runtime queue is idle. The module can check
   receipt structure; reviewer human provenance CANNOT be machine-verified here
   and is an explicit human verification boundary (see DESIGN.md).
2. **Controlled phase transition** (the machine-checked part): a capability
   bound to the old canonical SHA + exact proposed blob ids + exact allowed
   path set; a one-use ARMED window during which the amended reference hook
   admits exactly one transaction whose diff and per-path blobs match the
   capability, then APPLIED with protected-blob re-pinning and return to
   ordinary guard enforcement.

Power model (fail closed end to end):

* ``prepare`` writes a PENDING capability. It grants nothing.
* ``record-receipts`` ingests reviewer-authored receipt files; structural
  checks (digest binding, author exclusion, distinct reviewers, security+test
  roles) reject what can be rejected. Identity provenance stays human.
* ``activate`` re-validates base SHA, approvals, config delta, dirty worktree,
  runtime revision consistency, active claims and integration lock, then arms,
  applies, commits with ``OC2_RECOVERY_TRANSITION_CAP``, re-pins protected
  blobs and records APPLIED. Failure after ARMED stays fail-closed and is
  handled only by ``recover``.

This module never advances a lane, never touches queue/gate/legacy refs, and
never bypasses the ordinary claim/candidate/review/integrate path afterwards.
"""

from __future__ import annotations

import copy
import datetime as dt
import fcntl
import hashlib
import json
import os
import pathlib
import re
import secrets
import subprocess
import stat
import errno
import sys
from contextlib import contextmanager


def _read_bounded(fd: int, size: int, limit: int = 1024 * 1024) -> bytes:
    """Read exactly the recorded bounded size, tolerating legal short reads."""
    if size < 0 or size > limit:
        raise TransitionError("bounded record size is invalid")
    chunks = []
    remaining = size
    while remaining:
        chunk = os.read(fd, remaining)
        if not chunk:
            raise TransitionError("record ended before its validated size")
        if len(chunk) > remaining:
            raise TransitionError("record exceeded its validated size")
        chunks.append(chunk)
        remaining -= len(chunk)
    if os.read(fd, 1):
        raise TransitionError("record grew after its validated size")
    return b"".join(chunks)

ZERO = "0" * 40
SHA40 = re.compile(r"^[0-9a-f]{40}$")
ACTIVE_STATES = {"CLAIMED", "CANDIDATE", "PREVERIFIED", "INTEGRATION_READY", "INTEGRATING"}
REQUIRED_APPROVALS = 2
REQUIRED_ROLES = ("security", "test")
OBSERVER_PATHS = {
    "tests/e2e/native_provider_setup.py",
    "tests/e2e/native_stream_tool.py",
    "tests/e2e/native_utf8_input.py",
    "tests/e2e/native_escape_input.py",
    "tests/e2e/native_resize.py",
    "tests/e2e/native_bracketed_paste.py",
}
ALLOWED_PATHS = [
    "config/recovery-mode.json",
    "tools/recovery_ref_hook.py",
    "tools/recovery_transition.py",
]
PIN_ALLOWED_PATHS = frozenset({"tools/recovery_ref_hook.py"})
BOOTSTRAP_REPAIR_PATHS = [
    "config/recovery-mode.json",
    "tools/recovery_ref_hook.py",
    "tools/recovery_transition.py",
    "tools/recovery_state.py",
    "tools/recovery_guard.py",
]
TRANSITION_POLICY_EXPECTED = {
    "mechanism": "tools/recovery_transition.py",
    "requiredApprovals": 2,
    "requiredRoles": ["security", "test"],
    "authorMayNotApprove": True,
    "oneUse": True,
    "boundToCanonicalSha": True,
    "boundToExactBlobs": True,
    "capabilityLocation": "git-common-dir/recovery-state/transition-capability.json",
    "failClosedOnActiveClaims": True,
    "failClosedOnIntegrationLock": True,
    "capabilityExpirySeconds": 900,
    "authorizedObserverBlobsAreExact": True,
}
HUMAN_BOUNDARY = (
    "HUMAN-VERIFICATION-BOUNDARY: receipt structure checked; reviewer identity/"
    "provenance is NOT machine verifiable and must be confirmed by the operator"
)


class TransitionError(RuntimeError):
    pass


def now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat()


def git(root: pathlib.Path, *args: str, env: dict | None = None) -> str:
    return subprocess.check_output(
        ["git", *args], cwd=root, text=True, stderr=subprocess.STDOUT, env=env
    ).strip()


def git_ok(root: pathlib.Path, *args: str) -> bool:
    return subprocess.run(["git", *args], cwd=root, capture_output=True).returncode == 0


def common_git_dir(root: pathlib.Path) -> pathlib.Path:
    raw = pathlib.Path(git(root, "rev-parse", "--git-common-dir"))
    if not raw.is_absolute():
        raw = (root / raw).resolve()
    return raw


def state_dir(root: pathlib.Path) -> pathlib.Path:
    return common_git_dir(root) / "recovery-state"


def cap_path(root: pathlib.Path) -> pathlib.Path:
    return state_dir(root) / "transition-capability.json"


@contextmanager
def locked(path: pathlib.Path):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a+") as handle:
        fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
        try:
            yield
        finally:
            fcntl.flock(handle.fileno(), fcntl.LOCK_UN)


def write_cap(path: pathlib.Path, record: dict) -> None:
    tmp = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    tmp.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    os.chmod(tmp, 0o600)
    os.replace(tmp, path)


CAPABILITY_KINDS = {"bootstrap-controller", "observer-correction", "lane-sync-controller", "bootstrap-repair", "driver-policy-reconciliation"}
CAPABILITY_STATUSES = {"PENDING", "ARMED", "APPLIED", "PINNED", "REJECTED"}
CAPABILITY_ID = re.compile(r"^(?:ui-phase-transition-v2|observer-correction|lane-sync-controller|bootstrap-repair|driver-policy-reconciliation)-[0-9a-f]{16}$")
KIND_PATHS = {"bootstrap-controller": frozenset(ALLOWED_PATHS), "observer-correction": frozenset(OBSERVER_PATHS),
               "lane-sync-controller": frozenset({"tools/recovery_state.py"}), "bootstrap-repair": frozenset(BOOTSTRAP_REPAIR_PATHS),
               "driver-policy-reconciliation": frozenset({"policy.json"})}

def validate_capability_schema(record: dict) -> None:
    if not isinstance(record, dict):
        raise TransitionError("capability must be a JSON object")
    kind = record.get("kind")
    if kind not in CAPABILITY_KINDS:
        raise TransitionError("unknown or missing capability kind")
    if record.get("status") not in CAPABILITY_STATUSES:
        raise TransitionError("unknown capability status")
    if not CAPABILITY_ID.fullmatch(str(record.get("capabilityId", ""))):
        raise TransitionError("malformed capability id")
    allowed = record.get("allowedPaths")
    proposed = record.get("proposedBlobs")
    if not isinstance(allowed, list) or len(allowed) != len(set(allowed)):
        raise TransitionError("capability path set does not match its closed kind")
    if not isinstance(proposed, dict) or any(
        not (kind == "driver-policy-reconciliation" and str(k) == "policy.json" and re.fullmatch(r"[0-9a-f]{64}", str(v)))
        and not SHA40.fullmatch(str(v)) for k, v in proposed.items()
    ):
        raise TransitionError("capability blob map does not match its closed kind")
    if kind == "observer-correction":
        # A correction may name any non-empty subset of the closed observer set;
        # the blob map must cover exactly the named paths.
        if (not allowed or not set(map(str, allowed)) <= set(KIND_PATHS[kind])
                or set(map(str, allowed)) != set(map(str, proposed))):
            raise TransitionError("observer correction must name a non-empty subset of the closed observer set")
    elif set(map(str, allowed)) != set(KIND_PATHS[kind]) or set(map(str, proposed)) != set(KIND_PATHS[kind]):
        raise TransitionError("capability path set does not match its closed kind")
    if kind == "lane-sync-controller" and record.get("authorizedObserverBlobs") != {}:
        raise TransitionError("lane-sync controller capability may not authorize observer blobs")
    if kind == "bootstrap-repair" and record.get("authorizedObserverBlobs") not in (None, {}):
        raise TransitionError("bootstrap-repair capability may not authorize observer blobs")
    if kind == "driver-policy-reconciliation" and record.get("authorizedObserverBlobs") not in (None, {}):
        raise TransitionError("driver reconciliation may not authorize observer blobs")
    if not SHA40.fullmatch(str(record.get("baseSha"))) or not record.get("canonicalHead") or not record.get("capabilityId"):
        raise TransitionError("capability identity/base is malformed")
    if not isinstance(record.get("expiresAt"), str):
        raise TransitionError("capability expiry is malformed")


def load_cap(root: pathlib.Path) -> dict:
    path = cap_path(root)
    if not path.is_file():
        raise TransitionError("no transition capability exists; run prepare")
    try:
        record = json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:
        raise TransitionError(f"capability unreadable: {exc}") from exc
    validate_capability_schema(record)
    return record


def capability_digest(record: dict) -> str:
    fields = {
            "kind": record.get("kind", "bootstrap-controller"),
            "baseSha": record.get("baseSha"),
            "allowedPaths": record.get("allowedPaths"),
            "proposedBlobs": record.get("proposedBlobs"),
            "authorizedObserverBlobs": record.get("authorizedObserverBlobs"),
            "futureControllerBlob": record.get("futureControllerBlob"),
            "originalProtectedEvaluatorBlobs": record.get("originalProtectedEvaluatorBlobs"),
            "expiresAt": record.get("expiresAt"),
        }
    # Historical capabilities did not contain a pin plan.  Do not silently
    # invalidate receipts for those records merely because this amendment is
    # installed; only the newly supported producer binds a plan.
    if "pinPlan" in record:
        fields["pinPlan"] = record["pinPlan"]
    if record.get("kind") == "driver-policy-reconciliation":
        plan = record.get("reconciliationPlan")
        if not isinstance(plan, dict):
            raise TransitionError("reconciliation capability lacks immutable authorization plan")
        required = ("oldDriverSha256", "newDriverSha256", "canonicalDriverBlob", "wrapperSha256", "policySha256", "protectedMap", "originalPolicyBytes", "authorizedHistory")
        if any(k not in plan for k in required):
            raise TransitionError("reconciliation authorization plan is incomplete")
        fields["reconciliationAuthorization"] = {k: plan[k] for k in required}
    material = json.dumps(fields, sort_keys=True).encode()
    return hashlib.sha256(material).hexdigest()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _load_policy_checked(root: pathlib.Path) -> dict:
    policy_path = state_dir(root) / "policy.json"
    if not policy_path.is_file():
        raise TransitionError("runtime policy is missing")
    return json.loads(policy_path.read_text(encoding="utf-8"))


def _capture_pin_plan(root: pathlib.Path, policy: dict, base: str) -> dict | None:
    """Capture the trusted producer state while preparing a capability.

    This is deliberately performed by ``prepare`` rather than accepted from a
    caller.  A caller can propose source files, but cannot nominate a foreign
    hook, wrapper or driver as the original producer.
    """
    hooks_dir = common_git_dir(root) / "hooks"
    if policy.get("originalHookPinned") is True:
        # The original producer was already durably pinned.  Later stage
        # preparation must not reinterpret the pinned wrapper as a new
        # original-driver authorization.
        return None
    hook = hooks_dir / "reference-transaction"
    if not hook.is_file() or hook.is_symlink():
        return None
    configured = git(root, "config", "--path", "--get", "core.hooksPath")
    if pathlib.Path(configured).resolve() != hooks_dir.resolve():
        raise TransitionError("effective hooksPath does not match the transition hook directory")
    wrapper = hook.read_bytes()
    driver = _parse_wrapper_driver(hook, policy)
    expected_driver = (root / "tools" / "recovery_ref_hook.py").resolve()
    if driver != expected_driver:
        raise TransitionError("installed wrapper driver is not the approved canonical driver")
    driver_bytes = driver.read_bytes()
    expected_driver_sha = policy.get("referenceHookDriverSha256")
    if not isinstance(expected_driver_sha, str) or sha256_bytes(driver_bytes) != expected_driver_sha:
        raise TransitionError("trusted original driver hash is missing or mismatched")
    canonical_blob = git(root, "rev-parse", f"{base}:tools/recovery_ref_hook.py")
    if git_blob_id(root, driver_bytes) != canonical_blob:
        raise TransitionError("installed driver differs from trusted canonical producer blob")
    immutable = hooks_dir / "pinned-drivers" / "original_recovery_ref_hook.py"
    new_wrapper = ("#!/bin/sh\nexec /usr/bin/env python3 " + str(immutable) + ' "$@"\n').encode()
    return {
        "baseSha": base,
        "hookPath": "reference-transaction",
        "wrapperSha256": sha256_bytes(wrapper),
        "driverPath": "tools/recovery_ref_hook.py",
        "driverSha256": expected_driver_sha,
        "driverGitBlob": canonical_blob,
        "pinnedPath": str(immutable),
        "newWrapperSha256": sha256_bytes(new_wrapper),
        "newDriverSha256": expected_driver_sha,
    }


def _preparation_snapshot(root: pathlib.Path, policy_path: pathlib.Path, policy: dict, base: str) -> dict:
    """Capture trust inputs used by preparation and revalidate them under lock."""
    return {
        "baseSha": base,
        "policyBytes": policy_path.read_bytes(),
        "policySha256": sha256_bytes(policy_path.read_bytes()),
        "protectedEvaluatorBlobs": copy.deepcopy(policy.get("protectedEvaluatorBlobs")),
        "pinPlan": _capture_pin_plan(root, policy, base),
    }


def _assert_preparation_snapshot(root: pathlib.Path, policy_path: pathlib.Path, initial: dict) -> None:
    """Fail closed if any producer input changed before capability publication."""
    current_base = git(root, "rev-parse", "main-v2")
    if current_base != initial["baseSha"]:
        raise TransitionError("canonical base changed during capability preparation")
    current_bytes = policy_path.read_bytes()
    if current_bytes != initial["policyBytes"]:
        raise TransitionError("runtime policy changed during capability preparation")
    current_policy = json.loads(current_bytes.decode("utf-8"))
    if current_policy.get("protectedEvaluatorBlobs") != initial["protectedEvaluatorBlobs"]:
        raise TransitionError("runtime protected evaluator map changed during capability preparation")
    current_plan = _capture_pin_plan(root, current_policy, current_base)
    if current_plan != initial["pinPlan"]:
        raise TransitionError("hook producer inputs changed during capability preparation")


def _parse_wrapper_driver(hook_path: pathlib.Path, policy: dict) -> pathlib.Path:
    """Extract and validate the driver path from the wrapper script."""
    wrapper = hook_path.read_bytes()
    expected_wrapper = policy.get("referenceHookSha256")
    if expected_wrapper and sha256_bytes(wrapper) != expected_wrapper:
        raise TransitionError("installed wrapper does not match trusted policy")
    text = wrapper.decode("utf-8", "strict")
    marker = "exec /usr/bin/env python3 "
    if marker not in text:
        raise TransitionError("installed wrapper is not a Recovery-owned driver wrapper")
    raw = text.split(marker, 1)[1].split(' "$@"', 1)[0].strip()
    driver = pathlib.Path(raw)
    if not driver.is_absolute():
        raise TransitionError("driver path must be absolute")
    # lstat before resolution is intentional: a symlink to a regular file is
    # not an acceptable producer path, even if resolve() would hide it.
    try:
        driver_stat = driver.lstat()
    except OSError as exc:
        raise TransitionError("installed original driver is missing or unsafe") from exc
    if stat.S_ISLNK(driver_stat.st_mode) or not stat.S_ISREG(driver_stat.st_mode):
        raise TransitionError("installed original driver is missing or unsafe")
    resolved = driver.resolve(strict=True)
    if not resolved.is_file():
        raise TransitionError("installed original driver is missing or unsafe")
    for part in resolved.parts:
        if part == "..":
            raise TransitionError("driver path traverses forbidden component")
    return resolved


def _pin_driver_bytes(root: pathlib.Path, driver_bytes: bytes, hooks_dir: pathlib.Path) -> pathlib.Path:
    """Write driver bytes to the immutable pinned copy (idempotent if identical)."""
    immutable = hooks_dir / "pinned-drivers" / "original_recovery_ref_hook.py"
    if not immutable.parent.exists():
        _validate_destination_parent(hooks_dir, hooks_dir)
        immutable.parent.mkdir(mode=0o700)
        _fsync_dir(hooks_dir)
    _validate_destination_parent(hooks_dir, immutable.parent)
    if immutable.exists():
        if immutable.is_symlink() or not immutable.is_file():
            raise TransitionError("pinned driver path is unsafe")
        if immutable.read_bytes() != driver_bytes:
            raise TransitionError("pinned original driver already differs")
        return immutable
    tmp = _exclusive_temp(immutable.parent, immutable.name)
    try:
        os.write(tmp, driver_bytes)
        os.fsync(tmp)
        os.fchmod(tmp, 0o600)
        os.close(tmp); tmp = -1
        os.replace(immutable.parent / (immutable.name + ".pin.tmp"), immutable)
    finally:
        if tmp >= 0: os.close(tmp)
    dfd = os.open(str(immutable.parent), os.O_RDONLY)
    try:
        os.fsync(dfd)
    finally:
        os.close(dfd)
    return immutable


def _rewrite_wrapper(hook_path: pathlib.Path, driver_path: pathlib.Path) -> str:
    new_wrapper = ("#!/bin/sh\nexec /usr/bin/env python3 " + str(driver_path) + ' "$@"\n').encode()
    parent = hook_path.parent
    _validate_destination_parent(parent, parent)
    fd = _exclusive_temp(parent, hook_path.name)
    try:
        os.write(fd, new_wrapper); os.fsync(fd); os.fchmod(fd, 0o755)
        os.close(fd); fd = -1
        os.replace(parent / (hook_path.name + ".pin.tmp"), hook_path)
        _fsync_dir(parent)
    finally:
        if fd >= 0: os.close(fd)
    return sha256_bytes(new_wrapper)


def _write_policy_atomic(policy_path: pathlib.Path, policy: dict) -> None:
    data = (json.dumps(policy, indent=2, sort_keys=True) + "\n").encode()
    fd = _exclusive_temp(policy_path.parent, policy_path.name)
    try:
        os.write(fd, data); os.fsync(fd); os.fchmod(fd, 0o600)
        os.close(fd); fd = -1
        os.replace(policy_path.parent / (policy_path.name + ".pin.tmp"), policy_path)
        _fsync_dir(policy_path.parent)
    finally:
        if fd >= 0: os.close(fd)


def _fsync_dir(path: pathlib.Path) -> None:
    fd = os.open(str(path), os.O_RDONLY)
    try: os.fsync(fd)
    finally: os.close(fd)


def _validate_destination_parent(base: pathlib.Path, target: pathlib.Path) -> None:
    """Reject symlink/non-directory/world-writable path components."""
    base = base.absolute(); target = target.absolute()
    try: rel = target.relative_to(base)
    except ValueError as exc: raise TransitionError("pin destination escapes hooks directory") from exc
    current = base
    for component in (base.name,) + rel.parts:
        if component != base.name: current = current / component
        st = current.lstat()
        if not stat.S_ISDIR(st.st_mode) or stat.S_ISLNK(st.st_mode):
            raise TransitionError("pin destination parent is unsafe")
        if st.st_uid != os.getuid() or st.st_mode & 0o022:
            raise TransitionError("pin destination parent ownership or mode is unsafe")


def _exclusive_temp(parent: pathlib.Path, name: str) -> int:
    path = parent / (name + ".pin.tmp")
    try:
        return os.open(str(path), os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    except FileExistsError as exc:
        raise TransitionError("pin temporary path already exists") from exc


def pin_original_hook(root: pathlib.Path, operator: str) -> str:
    """Capability-bound pin of the currently installed original driver.

    Requires a PENDING capability with structural receipts.  Validates the
    driver against the canonical Git blob at the capability baseSha.  Writes
    the immutable pinned copy, rewrites the wrapper, and updates the policy.
    Preserves token, protectedEvaluatorBlobs, and ref-policy.json.
    """
    if not operator:
        raise TransitionError("pin-original-hook requires operator identity (audit field)")
    # The lock covers every read which authorizes the subsequent replacements.
    # In particular, do not load a capability before taking it: that creates a
    # capability/policy TOCTOU window.
    with locked(state_dir(root) / "state.lock"):
        cap = load_cap(root)
        if cap.get("status") != "PENDING":
            raise TransitionError(f"capability is {cap.get('status')}; pin requires PENDING")
        if capability_expired(cap):
            raise TransitionError("capability expired; prepare a fresh capability")
        plan = cap.get("pinPlan")
        required_plan = {"baseSha", "hookPath", "wrapperSha256", "driverPath", "driverSha256", "driverGitBlob", "pinnedPath", "newWrapperSha256", "newDriverSha256"}
        if not isinstance(plan, dict) or not required_plan <= set(plan):
            raise TransitionError("pin capability has no exact immutable pin plan")
        if plan["baseSha"] != cap.get("baseSha") or plan["hookPath"] != "reference-transaction":
            raise TransitionError("pin plan is not bound to this canonical base/hook")
        if not SHA40.fullmatch(str(plan["driverGitBlob"])) or not re.fullmatch(r"[0-9a-f]{64}", str(plan["driverSha256"])) or not re.fullmatch(r"[0-9a-f]{64}", str(plan["wrapperSha256"])):
            raise TransitionError("pin plan contains malformed hashes")
        errors = active_blockers(root) + revision_blockers(root) + validate_approvals(cap)
        if git(root, "rev-parse", "main-v2") != cap["baseSha"]:
            errors.append("canonical base changed since pin capability preparation")
        policy = _load_policy_checked(root)
        if policy.get("protectedEvaluatorBlobs") != cap.get("originalProtectedEvaluatorBlobs"):
            errors.append("runtime protected evaluator blob map changed since prepare")
        for rel, expected in policy.get("protectedEvaluatorBlobs", {}).items():
            try:
                if git(root, "rev-parse", f"{cap['baseSha']}:{rel}") != expected:
                    errors.append(f"protected evaluator blob changed: {rel}")
            except (subprocess.CalledProcessError, TransitionError):
                errors.append(f"protected evaluator blob unavailable: {rel}")
        if errors:
            raise TransitionError("pin authorization rejected: " + "; ".join(errors))
        hooks_dir = common_git_dir(root) / "hooks"
        hook = hooks_dir / "reference-transaction"
        if not hook.is_file() or hook.is_symlink():
            raise TransitionError("installed reference hook is missing or unsafe")
        configured = git(root, "config", "--path", "--get", "core.hooksPath")
        if pathlib.Path(configured).resolve() != hooks_dir.resolve():
            raise TransitionError("effective hooksPath does not match the transition hook directory")
        if sha256_bytes(hook.read_bytes()) != plan["wrapperSha256"] or policy.get("referenceHookSha256") != plan["wrapperSha256"]:
            raise TransitionError("installed wrapper is not the capability-bound wrapper")
        driver_path = _parse_wrapper_driver(hook, policy)
        expected_driver = (root / "tools" / "recovery_ref_hook.py").resolve()
        if driver_path != expected_driver:
            raise TransitionError("wrapper driver is not the approved canonical driver")
        driver_bytes = driver_path.read_bytes()
        actual_blob = git_blob_id(root, driver_bytes)
        canonical_blob = git(root, "rev-parse", f"{cap['baseSha']}:tools/recovery_ref_hook.py")
        if canonical_blob != plan["driverGitBlob"] or actual_blob != canonical_blob:
            raise TransitionError("driver does not match the capability-bound canonical Git blob")
        if sha256_bytes(driver_bytes) != plan["driverSha256"] or policy.get("referenceHookDriverSha256") != plan["driverSha256"]:
            raise TransitionError("driver does not match the trusted producer hash")
        record = dict(cap)
        record["pinJournal"] = {"phase": "BEFORE_FILES", "oldWrapperSha256": plan["wrapperSha256"],
                                "oldWrapperText": hook.read_text(encoding="utf-8"),
                                "newWrapperSha256": plan["newWrapperSha256"],
                                "oldPolicyWrapperSha256": policy.get("referenceHookSha256"),
                                "oldPolicyText": (state_dir(root) / "policy.json").read_text(encoding="utf-8"),
                                "newPolicyWrapperSha256": plan["newWrapperSha256"],
                                "oldDriverSha256": plan["driverSha256"],
                                "newDriverSha256": plan["newDriverSha256"],
                                "baseSha": cap["baseSha"],
                                "protectedMap": cap.get("originalProtectedEvaluatorBlobs")}
        write_cap(cap_path(root), record)
        immutable = _pin_driver_bytes(root, driver_bytes, hooks_dir)
        record["pinJournal"]["phase"] = "DRIVER_INSTALLED"
        write_cap(cap_path(root), record)
        new_wrapper_sha = _rewrite_wrapper(hook, immutable)
        if new_wrapper_sha != plan["newWrapperSha256"]:
            raise TransitionError("installed wrapper differs from prepared pin plan")
        record["pinJournal"]["phase"] = "WRAPPER_INSTALLED"
        write_cap(cap_path(root), record)
        policy["referenceHookSha256"] = new_wrapper_sha
        policy["referenceHookDriverSha256"] = sha256_bytes(driver_bytes)
        policy["originalHookPinnedBy"] = operator
        policy["originalHookPinned"] = True
        policy["originalDriverGitBlob"] = canonical_blob
        policy_bytes = (json.dumps(policy, indent=2, sort_keys=True) + "\n").encode()
        record["pinJournal"]["newPolicySha256"] = sha256_bytes(policy_bytes)
        write_cap(cap_path(root), record)
        _write_policy_atomic(state_dir(root) / "policy.json", policy)
        record["pinJournal"]["phase"] = "POLICY_INSTALLED"
        record["pinReceipt"] = {"operator": operator, "driverSha256": sha256_bytes(driver_bytes),
                                "driverGitBlob": canonical_blob, "wrapperSha256": new_wrapper_sha, "at": now()}
        record["pinJournal"]["phase"] = "COMPLETED"
        record["status"] = "PINNED"
        record["pinJournalCompleted"] = record.pop("pinJournal")
        write_cap(cap_path(root), record)
        return sha256_bytes(driver_bytes)


def interstage_hook_refresh(root: pathlib.Path, operator: str) -> str:
    """Refresh the wrapper to the stage-1-landed driver after stage-1 activation.

    Requires an APPLIED (archived) bootstrap-controller capability whose
    proposedBlobs contain the new driver bytes.  Validates the canonical
    worktree driver against the APPLIED blob before installing.
    """
    if not operator:
        raise TransitionError("interstage-hook-refresh requires operator identity")
    with locked(state_dir(root) / "state.lock"):
        cap = load_cap(root)
        if cap.get("status") != "APPLIED" or cap.get("kind") != "bootstrap-controller":
            raise TransitionError("interstage refresh requires an APPLIED bootstrap-controller capability")
        applied = cap.get("appliedSha")
        if not SHA40.fullmatch(str(applied)) or git(root, "rev-parse", "main-v2") != applied:
            raise TransitionError("stage-1 canonical head is stale or malformed")
        if validate_approvals(cap):
            raise TransitionError("stage-1 approval receipts are invalid")
        expected_blob = cap.get("proposedBlobs", {}).get("tools/recovery_ref_hook.py")
        if not SHA40.fullmatch(str(expected_blob)):
            raise TransitionError("stage-1 proposed driver blob is missing")
        policy = _load_policy_checked(root)
        expected_map = _expected_protected_map(cap)
        if policy.get("protectedEvaluatorBlobs") != expected_map:
            raise TransitionError("protected evaluator map changed since stage-1")
        landed_bytes = subprocess.check_output(["git", "show", f"{applied}:tools/recovery_ref_hook.py"], cwd=root)
        if git_blob_id(root, landed_bytes) != expected_blob:
            raise TransitionError("stage-1 landed driver blob mismatch")
        hooks_dir = common_git_dir(root) / "hooks"
        hook = hooks_dir / "reference-transaction"
        if not hook.is_file() or hook.is_symlink():
            raise TransitionError("installed reference hook is missing or unsafe")
        pin = cap.get("pinReceipt")
        if not isinstance(pin, dict) or policy.get("originalDriverGitBlob") != pin.get("driverGitBlob"):
            raise TransitionError("initial pin receipt is missing or altered")
        new_pinned = hooks_dir / "pinned-drivers" / "interstage_recovery_ref_hook.py"
        if not new_pinned.parent.exists():
            new_pinned.parent.mkdir(mode=0o700)
        _validate_destination_parent(hooks_dir, new_pinned.parent)
        if new_pinned.exists():
            if new_pinned.is_symlink() or new_pinned.read_bytes() != landed_bytes:
                raise TransitionError("foreign interstage pinned driver exists")
        else:
            fd = _exclusive_temp(new_pinned.parent, new_pinned.name)
            try:
                os.write(fd, landed_bytes); os.fsync(fd); os.fchmod(fd, 0o600)
                os.close(fd); fd = -1
                os.replace(new_pinned.parent / (new_pinned.name + ".pin.tmp"), new_pinned)
                _fsync_dir(new_pinned.parent)
            finally:
                if fd >= 0: os.close(fd)
        new_wrapper = ("#!/bin/sh\nexec /usr/bin/env python3 " + str(new_pinned) + ' "$@"\n').encode()
        old_wrapper = hook.read_bytes()
        record = dict(cap)
        record["interstageJournal"] = {"oldWrapperSha256": sha256_bytes(old_wrapper),
                                        "oldWrapperText": old_wrapper.decode("utf-8"),
                                        "newWrapperSha256": sha256_bytes(new_wrapper),
                                        "oldPolicySha256": sha256_bytes((state_dir(root) / "policy.json").read_bytes()),
                                        "expectedDriverBlob": expected_blob, "phase": "BEFORE_FILES"}
        write_cap(cap_path(root), record)
        _rewrite_wrapper(hook, new_pinned)
        record["interstageJournal"]["phase"] = "WRAPPER_INSTALLED"
        write_cap(cap_path(root), record)
        policy["referenceHookSha256"] = sha256_bytes(new_wrapper)
        policy["referenceHookDriverSha256"] = sha256_bytes(landed_bytes)
        _write_policy_atomic(state_dir(root) / "policy.json", policy)
        record["interstageJournal"]["phase"] = "POLICY_INSTALLED"
        record["interstageReceipt"] = {"driverGitBlob": expected_blob, "wrapperSha256": sha256_bytes(new_wrapper), "at": now()}
        record.pop("interstageJournal", None)
        write_cap(cap_path(root), record)
        return sha256_bytes(landed_bytes)


def git_blob_id(root: pathlib.Path, data: bytes) -> str:
    """Actual Git object ID of ``data`` as a blob (no object written).

    This is what ``git rev-parse <rev>:<path>`` returns and therefore what the
    reference hook compares against.  It is NOT the sha256 content-hash
    manifest used for evidence files.
    """
    proc = subprocess.run(
        ["git", "hash-object", "--stdin"],
        cwd=root, input=data, capture_output=True,
    )
    oid = proc.stdout.decode().strip()
    if proc.returncode == 0 and SHA40.fullmatch(oid):
        return oid
    # ponytail: fallback assumes sha1 object format (this repository's format);
    # sha256-object repos must go through git hash-object instead.
    return hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()


def _expected_protected_map(record: dict) -> dict:
    """Expected policy pins at the record's current lifecycle boundary.

    Before activation, the policy must equal the captured original map.  Once
    a stage-1 bootstrap commit is APPLIED, _finish_policy has legitimately
    advanced only protected entries named by the capability's proposed blobs.
    The key set remains closed to the original map.
    """
    original = record.get("originalProtectedEvaluatorBlobs")
    if not isinstance(original, dict):
        return {}
    expected = {str(k): str(v) for k, v in original.items()}
    proposed = record.get("proposedBlobs")
    allowed = record.get("allowedPaths")
    if record.get("status") == "APPLIED" and (not isinstance(proposed, dict) or not isinstance(allowed, list)):
        return {}
    if record.get("status") == "APPLIED":
        for rel in expected:
            if rel in allowed:
                blob = proposed.get(rel)
                if not isinstance(blob, str) or not SHA40.fullmatch(blob):
                    return {}
                expected[rel] = blob
    return expected


def validate_config_delta(old: dict, new: dict) -> list[str]:
    """Exactly the authorized delta; anything else is an error."""
    errors: list[str] = []
    old_n = copy.deepcopy(old)
    new_n = copy.deepcopy(new)
    added_policy = new_n.pop("transitionPolicy", None)
    if "transitionPolicy" in old_n:
        errors.append("transitionPolicy already exists in base config")
    if not isinstance(added_policy, dict):
        errors.append("transitionPolicy missing in proposed config")
        added_policy = {}
    for key, expected in TRANSITION_POLICY_EXPECTED.items():
        if added_policy.get(key) != expected:
            errors.append(
                f"transitionPolicy.{key} mismatch: {added_policy.get(key)!r} != {expected!r}"
            )
    lane_pin = added_policy.get("laneSyncController")
    if not isinstance(lane_pin, dict) or set(lane_pin) != {"kind", "path", "blob", "maxLifetimeSeconds"}:
        errors.append("transitionPolicy.laneSyncController must be the exact hard-pin object")
    elif (lane_pin.get("kind") != "lane-sync-controller" or lane_pin.get("path") != "tools/recovery_state.py"
          or not SHA40.fullmatch(str(lane_pin.get("blob", ""))) or lane_pin.get("maxLifetimeSeconds") != 60):
        errors.append("transitionPolicy.laneSyncController is not the exact single-controller pin")
    if old_n != new_n:
        errors.append("config changed outside transitionPolicy")
    return errors


def validate_lane_sync_bootstrap_delta(old: dict, new: dict, proposed_blobs: dict) -> list[str]:
    """Stage one adds only the exact state-controller hard pin to existing policy."""
    errors: list[str] = []
    old_n = copy.deepcopy(old)
    new_n = copy.deepcopy(new)
    old_policy = old_n.pop("transitionPolicy", None)
    new_policy = new_n.pop("transitionPolicy", None)
    if old_n != new_n:
        errors.append("stage-one config changed outside transitionPolicy")
    if not isinstance(old_policy, dict) or not isinstance(new_policy, dict):
        return errors + ["stage-one requires the existing transitionPolicy"]
    old_pin = old_policy.pop("laneSyncController", None)
    pin = new_policy.pop("laneSyncController", None)
    if old_policy != new_policy:
        errors.append("stage-one changed existing transitionPolicy fields")
    # The stage-one purpose is to rotate the controller hard-pin from the
    # canonical controller blob to the exact proposed controller blob.  The
    # old pin is an input to this validation, not an immutable field: requiring
    # old_policy == new_policy while retaining old_pin made the supported
    # original-hook stage-one path unpreparable whenever the controller bytes
    # changed.  Keep the old pin structurally valid and bind the replacement
    # to the capability's futureControllerBlob below.
    if not isinstance(old_pin, dict) or old_pin.get("kind") != "lane-sync-controller" or old_pin.get("path") != "tools/recovery_state.py" or old_pin.get("maxLifetimeSeconds") != 60:
        errors.append("stage-one existing laneSyncController pin is malformed")
    expected = {
        "kind": "lane-sync-controller", "path": "tools/recovery_state.py",
        "blob": proposed_blobs.get("tools/recovery_state.py"), "maxLifetimeSeconds": 60,
    }
    if pin != expected or not SHA40.fullmatch(str(expected["blob"])):
        errors.append("stage-one config must hard-pin the exact proposed recovery_state.py blob")
    return errors

def active_blockers(root: pathlib.Path) -> list[str]:
    errors: list[str] = []
    sdir = state_dir(root)
    if (sdir / "integration.lock").exists():
        errors.append("integration lock is held; transition must fail closed")
    failures_path = sdir / "failures.json"
    if failures_path.is_file():
        try:
            rows = json.loads(failures_path.read_text(encoding="utf-8")).get("failures", [])
        except Exception as exc:
            return [f"cannot read runtime queue: {exc}"]
        active = [r.get("id") for r in rows if r.get("status") in ACTIVE_STATES]
        if active:
            errors.append(f"active claims exist; transition must fail closed: {active}")
    else:
        errors.append("runtime queue missing")
    return errors


def revision_blockers(root: pathlib.Path) -> list[str]:
    sdir = state_dir(root)
    try:
        meta = json.loads((sdir / "meta.json").read_text(encoding="utf-8"))
        failures = json.loads((sdir / "failures.json").read_text(encoding="utf-8"))
        gates = json.loads((sdir / "gates.json").read_text(encoding="utf-8"))
        salvage = json.loads((sdir / "salvage-index.json").read_text(encoding="utf-8"))
    except Exception as exc:
        return [f"runtime state unreadable: {exc}"]
    revision = meta.get("stateRevision")
    if not isinstance(revision, int) or revision < 1:
        return ["runtime meta has invalid stateRevision"]
    errors = []
    for name, value in (
        ("failures", failures.get("queueRevision")),
        ("gates", gates.get("stateRevision")),
        ("salvage", salvage.get("stateRevision")),
    ):
        if value != revision:
            errors.append(f"stale/inconsistent runtime revision: {name} {value!r} != {revision!r}")
    return errors


def dirty_blockers(root: pathlib.Path, paths: list[str]) -> list[str]:
    out = subprocess.run(
        ["git", "status", "--porcelain", "--", *paths],
        cwd=root, text=True, capture_output=True,
    )
    if out.returncode != 0:
        return [f"cannot inspect worktree state: {out.stderr.strip()}"]
    dirty = [line for line in out.stdout.splitlines() if line.strip()]
    if dirty:
        return [f"canonical worktree is dirty in transition paths: {dirty[:6]}"]
    return []


def capability_expired(record: dict) -> bool:
    expires = record.get("expiresAt")
    if not isinstance(expires, str):
        return True
    try:
        return dt.datetime.now(dt.timezone.utc) >= dt.datetime.fromisoformat(expires)
    except (ValueError, TypeError, OverflowError):
        return True


def validate_approvals(record: dict) -> list[str]:
    errors: list[str] = []
    digest = capability_digest(record)
    author = str(record.get("author", ""))
    approvals = [a for a in record.get("approvals", []) if isinstance(a, dict)]
    for a in approvals:
        if a.get("verdict") in {"REJECT", "NEEDS_FIX"}:
            errors.append(f"negative verdict {a.get('verdict')} vetoes this capability")
    approving = [
        a for a in approvals
        if a.get("verdict") == "APPROVE" and a.get("capabilityDigest") == digest
    ]
    reviewers = [str(a.get("reviewer", "")) for a in approving]
    if len(set(reviewers)) < REQUIRED_APPROVALS:
        errors.append(
            f"needs {REQUIRED_APPROVALS} distinct independent approvals, has {len(set(reviewers))}"
        )
    for reviewer in reviewers:
        if not reviewer or reviewer == author:
            errors.append(f"author/empty self-approval is not trusted: {reviewer!r}")
    roles = {str(a.get("role", "")) for a in approving}
    for role in REQUIRED_ROLES:
        if role not in roles:
            errors.append(f"missing required independent {role} approval")
    seen = set()
    for reviewer in reviewers:
        if reviewer in seen:
            errors.append(f"duplicate reviewer: {reviewer}")
        seen.add(reviewer)
    for a in approvals:
        if a.get("role") not in set(REQUIRED_ROLES) | {"independent"}:
            errors.append(f"untrusted approval role: {a.get('role')!r}")
        if a.get("verdict") not in {"APPROVE", "REJECT", "NEEDS_FIX"}:
            errors.append(f"untrusted approval verdict: {a.get('verdict')!r}")
        if a.get("capabilityDigest") != digest:
            errors.append("approval receipt is not bound to this capability digest")
    if any(str(a.get("reviewer", "")) == author for a in approvals):
        errors.append("author-as-reviewer receipt present; rejected")
    return errors


def prepare(root: pathlib.Path, proposal_root: pathlib.Path, author: str, observer_manifest: pathlib.Path | None = None) -> dict:
    if not author or author in {"unknown", ""}:
        raise TransitionError("prepare requires a named author")
    base = git(root, "rev-parse", "main-v2")
    if not SHA40.fullmatch(base):
        raise TransitionError("cannot resolve canonical base SHA")
    base_config = json.loads(git(root, "show", f"{base}:config/recovery-mode.json"))
    proposal_config = json.loads(
        (proposal_root / "config" / "recovery-mode.json").read_text(encoding="utf-8")
    )
    if isinstance(proposal_config.get("transitionPolicy", {}).get("laneSyncController"), dict):
        state_source = proposal_root / "tools" / "recovery_state.py"
        state_blob = git_blob_id(root, state_source.read_bytes()) if state_source.is_file() else ""
        delta_errors = validate_lane_sync_bootstrap_delta(
            base_config, proposal_config, {"tools/recovery_state.py": state_blob}
        )
    else:
        delta_errors = validate_config_delta(base_config, proposal_config)
    if delta_errors:
        raise TransitionError("proposal config delta rejected: " + "; ".join(delta_errors))
    content: dict[str, str] = {}
    for rel in ALLOWED_PATHS:
        src = proposal_root / rel
        if not src.is_file():
            raise TransitionError(f"proposal source missing: {src}")
        content[rel] = src.read_text(encoding="utf-8")
    # Git object IDs (what the hook's `git rev-parse new:path` compares),
    # not sha256 evidence-manifest hashes.
    blobs = {rel: git_blob_id(root, text.encode()) for rel, text in content.items()}
    if set(blobs) != set(ALLOWED_PATHS):
        raise TransitionError("proposal blobs must cover exactly the three controller paths")
    observer_blobs: dict[str, str] = {}
    if observer_manifest is None:
        raise TransitionError("bootstrap requires the exact six-path observer baseline manifest")
    if observer_manifest is not None:
        try:
            supplied = json.loads(observer_manifest.read_text(encoding="utf-8"))
        except Exception as exc:
            raise TransitionError(f"observer blob manifest unreadable: {exc}") from exc
        if not isinstance(supplied, dict) or any(not isinstance(k, str) or not SHA40.fullmatch(str(v)) for k, v in supplied.items()):
            raise TransitionError("observer blob manifest must be a path->40-hex Git blob map")
        cfg = json.loads(git(root, "show", f"{base}:config/recovery-mode.json"))
        protected = set(cfg.get("evaluationPolicy", {}).get("protectedEvaluatorPaths", []))
        if set(supplied) != OBSERVER_PATHS:
            raise TransitionError("observer blob manifest must contain exactly the six authorized observer paths")
        if any(k not in protected for k in supplied):
            raise TransitionError("observer blob manifest contains an unprotected path")
        for rel, expected in supplied.items():
            try:
                actual = git(root, "rev-parse", f"{base}:{rel}")
            except subprocess.CalledProcessError as exc:
                raise TransitionError(f"observer manifest path absent at base: {rel}") from exc
            if actual != str(expected):
                raise TransitionError(f"observer manifest blob mismatch at base: {rel}: {actual} != {expected}")
        observer_blobs = {str(k): str(v) for k, v in sorted(supplied.items())}
    record = {
        "schemaVersion": 1,
        "kind": "bootstrap-controller",
        "capabilityId": "ui-phase-transition-v2-" + hashlib.sha256(os.urandom(32)).hexdigest()[:16],
        "status": "PENDING",
        "author": author,
        "baseSha": base,
        "canonicalHead": "main-v2",
        "allowedPaths": list(ALLOWED_PATHS),
        "proposedBlobs": blobs,
        "proposedContent": content,
        "futureControllerBlob": state_blob if isinstance(proposal_config.get("transitionPolicy", {}).get("laneSyncController"), dict) else None,
        "createdAt": now(),
        "expiresAt": (dt.datetime.now(dt.timezone.utc) + dt.timedelta(seconds=900)).isoformat(),
        "authorizedObserverBlobs": observer_blobs,
        "originalProtectedEvaluatorBlobs": {},
        "approvals": [],
    }
    policy_path = state_dir(root) / "policy.json"
    if not policy_path.is_file():
        raise TransitionError("runtime policy missing")
    policy = json.loads(policy_path.read_text(encoding="utf-8"))
    initial_snapshot = _preparation_snapshot(root, policy_path, policy, base)
    protected_map = policy.get("protectedEvaluatorBlobs")
    if not isinstance(protected_map, dict) or not protected_map:
        raise TransitionError("runtime protected evaluator blob map missing")
    record["originalProtectedEvaluatorBlobs"] = {str(k): str(v) for k, v in sorted(protected_map.items())}
    record["pinPlan"] = initial_snapshot["pinPlan"]
    path = cap_path(root)
    with locked(state_dir(root) / "state.lock"):
        if git(root, "rev-parse", "main-v2") != base:
            raise TransitionError("canonical base changed during capability preparation")
        _assert_preparation_snapshot(root, policy_path, initial_snapshot)
        if path.is_file():
            existing = load_cap(root)
            raise TransitionError("capability slot is occupied; archive terminal record before preparing")
        write_cap(path, record)
    return record



def prepare_lane_sync_controller(root: pathlib.Path, proposal: pathlib.Path, author: str) -> dict:
    if not author or author == "unknown":
        raise TransitionError("lane-sync controller preparation requires named author")
    base = git(root, "rev-parse", "main-v2")
    if active_blockers(root) or revision_blockers(root):
        raise TransitionError("lane-sync controller prepare requires idle consistent runtime state")
    policy = json.loads((state_dir(root) / "policy.json").read_text(encoding="utf-8"))
    pinned = policy.get("protectedEvaluatorBlobs")
    if not isinstance(pinned, dict):
        raise TransitionError("protected evaluator pins unavailable")
    current_config = json.loads(git(root, "show", f"{base}:config/recovery-mode.json"))
    pin = current_config.get("transitionPolicy", {}).get("laneSyncController")
    if not isinstance(pin, dict) or set(pin) != {"kind", "path", "blob", "maxLifetimeSeconds"} or pin.get("kind") != "lane-sync-controller" or pin.get("path") != "tools/recovery_state.py" or pin.get("maxLifetimeSeconds") != 60:
        raise TransitionError("stage-1 lane-sync controller pin missing/malformed")
    data = proposal.read_bytes()
    blob = git_blob_id(root, data)
    if blob != pin.get("blob"):
        raise TransitionError("proposed recovery_state.py differs from exact stage-1 hard pin")
    if git(root, "rev-parse", f"{base}:tools/recovery_state.py") != pinned.get("tools/recovery_state.py"):
        raise TransitionError("current protected recovery_state.py pin has drifted")
    record = {
        "schemaVersion": 3, "kind":"lane-sync-controller", "capabilityId":"lane-sync-controller-"+secrets.token_hex(8),
        "status":"PENDING", "author":author, "baseSha":base, "canonicalHead":"main-v2",
        "allowedPaths":["tools/recovery_state.py"], "proposedBlobs":{"tools/recovery_state.py":blob},
        "proposedContent":{"tools/recovery_state.py":data.decode("utf-8")}, "createdAt":now(),
        "expiresAt":(dt.datetime.now(dt.timezone.utc)+dt.timedelta(seconds=900)).isoformat(),
        "authorizedObserverBlobs":{}, "originalProtectedEvaluatorBlobs":{str(k):str(v) for k,v in sorted(pinned.items())}, "approvals":[],
    }
    validate_capability_schema(record)
    with locked(state_dir(root)/"state.lock"):
        if cap_path(root).exists():
            raise TransitionError("capability slot occupied; archive terminal record before preparation")
        errors=active_blockers(root)+revision_blockers(root)
        if errors: raise TransitionError("runtime changed during prepare: "+"; ".join(errors))
        write_cap(cap_path(root),record)
    return record


def validate_bootstrap_repair_delta(old: dict, new: dict) -> list[str]:
    """Narrowly allow topology/guard-integrity bootstrap repair delta."""
    import copy as _copy
    old_n = _copy.deepcopy(old)
    new_n = _copy.deepcopy(new)
    errors = []
    # preserve identity-critical fields
    for key in ("schemaVersion", "status", "mode", "canonicalBranch", "bootstrapBranch",
                "phase1Contract", "trackedFailureSeed", "trackedGateDefinitions",
                "trackedSalvageTemplate", "runtimeStateLocation", "runtimeEvidenceLocation",
                "failureQueue", "phase1GateState", "legacySnapshot", "salvageIndex",
                "branchInventoryIsExecutionQueue", "bulkLegacyMergeAllowed",
                "heritageMergeRequiredForPhase1", "legacyRefsMutable"):
        if old_n.get(key) != new_n.get(key):
            errors.append(f"{key} changed")
    # transition policy may be extended only with bounded bootstrap-repair metadata
    old_tp = old_n.get("transitionPolicy") or {}
    new_tp = new_n.get("transitionPolicy") or {}
    if not isinstance(old_tp, dict) or not isinstance(new_tp, dict):
        errors.append("transitionPolicy must be object")
    else:
        # existing required fields must remain compatible
        for k in ("mechanism", "requiredApprovals", "requiredRoles", "authorMayNotApprove",
                  "oneUse", "boundToCanonicalSha", "boundToExactBlobs", "capabilityLocation",
                  "failClosedOnActiveClaims", "failClosedOnIntegrationLock",
                  "capabilityExpirySeconds", "authorizedObserverBlobsAreExact"):
            if k in old_tp and old_tp.get(k) != new_tp.get(k):
                errors.append(f"transitionPolicy.{k} changed")
        # laneSyncController pin must be preserved if present
        if "laneSyncController" in old_tp and old_tp.get("laneSyncController") != new_tp.get("laneSyncController"):
            errors.append("transitionPolicy.laneSyncController changed")
    # allowed additions/changes for topology bootstrap repair
    allowed_mutable = {"fixedMutableBranches", "maxFixedRecoveryLanes", "featureWorkspacePolicy",
                       "subagentPolicy", "approvedManagedWorkspaceRoots", "transitionPolicy",
                       "executionLanes", "approvedRecoveryWorktrees", "approvedNonExecutionBranches",
                       "lanePathIntent"}
    for k in set(old_n.keys()) | set(new_n.keys()):
        if k in allowed_mutable:
            continue
        if old_n.get(k) != new_n.get(k):
            errors.append(f"unexpected change: {k}")
    # feature refs must be exact enumerated recovery/feature-01..30 only
    fwp = new_n.get("featureWorkspacePolicy") or {}
    refs = fwp.get("refs") or []
    expected = [f"refs/heads/recovery/feature-{i:02d}" for i in range(1, 31)]
    if refs != expected:
        errors.append("featureWorkspacePolicy.refs must be exact refs/heads/recovery/feature-01..30")
    if fwp.get("root") != "/Users/mymac/Projects/opencode-rk-recovery-lanes/workspaces":
        errors.append("featureWorkspacePolicy.root mismatch")
    fixed = new_n.get("fixedMutableBranches") or []
    required_base = ["main-v2", "recovery/core", "recovery/runtime", "recovery/ui", "recovery/platform"]
    if fixed[:5] != required_base:
        errors.append("base fixed mutable branches changed")
    for r in [f"recovery/feature-{i:02d}" for i in range(1, 31)]:
        if r not in fixed:
            errors.append(f"missing feature branch {r}")
    # must not remove existing execution lanes or canonical/bootstrap identity
    for r in required_base:
        if r not in fixed:
            errors.append(f"missing required fixed branch {r}")
    return errors


def prepare_bootstrap_repair(root: pathlib.Path, proposal_root: pathlib.Path, author: str) -> dict:
    if not author or author in {"unknown", ""}:
        raise TransitionError("prepare requires a named author")
    base = git(root, "rev-parse", "main-v2")
    if not SHA40.fullmatch(base):
        raise TransitionError("cannot resolve canonical base SHA")
    base_config = json.loads(git(root, "show", f"{base}:config/recovery-mode.json"))
    proposal_config = json.loads((proposal_root / "config" / "recovery-mode.json").read_text(encoding="utf-8"))
    delta_errors = validate_bootstrap_repair_delta(base_config, proposal_config)
    if delta_errors:
        raise TransitionError("bootstrap repair config delta rejected: " + "; ".join(delta_errors))
    content: dict[str, str] = {}
    for rel in BOOTSTRAP_REPAIR_PATHS:
        src = proposal_root / rel
        if not src.is_file():
            raise TransitionError(f"proposal source missing: {src}")
        content[rel] = src.read_text(encoding="utf-8")
    blobs = {rel: git_blob_id(root, text.encode()) for rel, text in content.items()}
    policy_path = state_dir(root) / "policy.json"
    if not policy_path.is_file():
        raise TransitionError("runtime policy missing")
    policy = json.loads(policy_path.read_text(encoding="utf-8"))
    initial_snapshot = _preparation_snapshot(root, policy_path, policy, base)
    protected_map = policy.get("protectedEvaluatorBlobs")
    if not isinstance(protected_map, dict) or not protected_map:
        raise TransitionError("runtime protected evaluator blob map missing")
    record = {
        "schemaVersion": 2,
        "kind": "bootstrap-repair",
        "capabilityId": "bootstrap-repair-" + hashlib.sha256(os.urandom(32)).hexdigest()[:16],
        "status": "PENDING",
        "author": author,
        "baseSha": base,
        "canonicalHead": "main-v2",
        "allowedPaths": list(BOOTSTRAP_REPAIR_PATHS),
        "proposedBlobs": blobs,
        "proposedContent": content,
        "createdAt": now(),
        "expiresAt": (dt.datetime.now(dt.timezone.utc) + dt.timedelta(seconds=900)).isoformat(),
        "authorizedObserverBlobs": {},
        "originalProtectedEvaluatorBlobs": {str(k): str(v) for k, v in sorted(protected_map.items())},
        "approvals": [],
    }
    record["pinPlan"] = initial_snapshot["pinPlan"]
    validate_capability_schema(record)
    path = cap_path(root)
    with locked(state_dir(root) / "state.lock"):
        if git(root, "rev-parse", "main-v2") != base:
            raise TransitionError("canonical base changed during capability preparation")
        _assert_preparation_snapshot(root, policy_path, initial_snapshot)
        if path.is_file():
            load_cap(root)
            raise TransitionError("capability slot is occupied; archive terminal record before preparing")
        write_cap(path, record)
    return record


def prepare_driver_policy_reconciliation(root: pathlib.Path, author: str) -> dict:
    """Prepare the explicitly closed old-driver -> canonical-driver repin."""
    if not author or author in {"unknown", ""}:
        raise TransitionError("driver reconciliation requires named author")
    base = git(root, "rev-parse", "main-v2")
    policy_path = state_dir(root) / "policy.json"
    policy = _load_policy_checked(root)
    old_sha = "33134ce917b94326d7b38798aa2c4b5a5891899ff01774ae0d28f11893065eea"
    new_sha = "45722cb1a85a9a5396fd462411fd05a9a3ccc46e28ecc0318c8eb21e331f8a03"
    current = (root / "tools" / "recovery_ref_hook.py")
    if policy.get("referenceHookDriverSha256") != old_sha:
        raise TransitionError("reconciliation requires the exact closed old policy driver hash")
    if not current.is_file() or current.is_symlink() or sha256_bytes(current.read_bytes()) != new_sha:
        raise TransitionError("canonical driver does not match the exact authorized replacement")
    canonical_blob = git(root, "rev-parse", f"{base}:tools/recovery_ref_hook.py")
    if sha256_bytes(current.read_bytes()) != new_sha or not SHA40.fullmatch(canonical_blob):
        raise TransitionError("canonical driver identity is invalid")
    hook = common_git_dir(root) / "hooks" / "reference-transaction"
    wrapper = hook.read_bytes()
    capability_id = "driver-policy-reconciliation-" + secrets.token_hex(8)
    created_at = now()
    initial = {"base": base, "policy": policy_path.read_bytes(), "wrapper": wrapper,
               "wrapperSha256": sha256_bytes(wrapper), "driver": current.read_bytes(),
               "driverSha256": sha256_bytes(current.read_bytes()), "driverBlob": canonical_blob,
               "hooksPath": git(root, "config", "--path", "--get", "core.hooksPath")}
    record = {
        "schemaVersion": 1, "kind": "driver-policy-reconciliation", "status": "PENDING",
        "capabilityId": capability_id, "author": author,
        "baseSha": base, "canonicalHead": "main-v2", "allowedPaths": ["policy.json"],
        "proposedBlobs": {"policy.json": sha256_bytes(policy_path.read_bytes())},
        "authorizedObserverBlobs": {}, "originalProtectedEvaluatorBlobs": dict(policy.get("protectedEvaluatorBlobs", {})),
        "approvals": [], "createdAt": created_at,
        "expiresAt": (dt.datetime.now(dt.timezone.utc) + dt.timedelta(seconds=900)).isoformat(),
        "reconciliationPlan": {"oldDriverSha256": old_sha, "newDriverSha256": new_sha,
                                "canonicalDriverBlob": canonical_blob,
                                "wrapperSha256": sha256_bytes(wrapper),
                                "policySha256": sha256_bytes(policy_path.read_bytes()),
                                "protectedMap": dict(policy.get("protectedEvaluatorBlobs", {})),
                                "originalPolicyBytes": policy_path.read_bytes().decode("utf-8"),
                                "authorizedHistory": {"oldDriverSha256": old_sha, "newDriverSha256": new_sha,
                                                       "canonicalDriverBlob": canonical_blob,
                                                       "capabilityId": capability_id, "at": created_at}},
    }
    validate_capability_schema(record)
    policy_bytes = policy_path.read_bytes()
    record["proposedBlobs"]["policy.json"] = sha256_bytes(policy_bytes)
    with locked(state_dir(root) / "state.lock"):
        current_hook_path = git(root, "config", "--path", "--get", "core.hooksPath")
        current_hook = hook.read_bytes() if hook.is_file() and not hook.is_symlink() else b""
        current_driver = current.read_bytes() if current.is_file() and not current.is_symlink() else b""
        if (git(root, "rev-parse", "main-v2") != base or policy_path.read_bytes() != initial["policy"] or
            current_hook_path != initial["hooksPath"] or current_hook != initial["wrapper"] or
            current_driver != initial["driver"] or git(root, "rev-parse", f"{base}:tools/recovery_ref_hook.py") != initial["driverBlob"]):
            raise TransitionError("reconciliation snapshot changed before publication")
        if cap_path(root).exists():
            raise TransitionError("capability slot occupied")
        write_cap(cap_path(root), record)
    return record


def reconcile_driver_policy(root: pathlib.Path, operator: str) -> str:
    if not operator:
        raise TransitionError("driver reconciliation requires operator identity")
    with locked(state_dir(root) / "state.lock"):
        record = load_cap(root)
        if record.get("kind") != "driver-policy-reconciliation" or record.get("status") != "PENDING":
            raise TransitionError("reconciliation requires a pending reconciliation capability")
        if capability_expired(record) or validate_approvals(record):
            raise TransitionError("reconciliation approvals or expiry are invalid")
        plan = record.get("reconciliationPlan", {})
        policy_path = state_dir(root) / "policy.json"
        policy_bytes = policy_path.read_bytes()
        if sha256_bytes(policy_bytes) != plan.get("policySha256"):
            raise TransitionError("policy changed from the authorized old state")
        policy = json.loads(policy_bytes)
        if policy.get("protectedEvaluatorBlobs") != plan.get("protectedMap"):
            raise TransitionError("protected evaluator map changed")
        if policy.get("referenceHookDriverSha256") != plan.get("oldDriverSha256"):
            raise TransitionError("old driver policy identity changed")
        if git(root, "rev-parse", "main-v2") != record.get("baseSha"):
            raise TransitionError("reconciliation base changed")
        hook_dir = common_git_dir(root) / "hooks"
        if pathlib.Path(git(root, "config", "--path", "--get", "core.hooksPath")).resolve() != hook_dir.resolve():
            raise TransitionError("effective hooksPath changed")
        hook = hook_dir / "reference-transaction"
        if hook.is_symlink() or not hook.is_file() or sha256_bytes(hook.read_bytes()) != plan.get("wrapperSha256"):
            raise TransitionError("wrapper identity changed from authorized state")
        driver = root / "tools" / "recovery_ref_hook.py"
        if driver.is_symlink() or not driver.is_file() or sha256_bytes(driver.read_bytes()) != plan.get("newDriverSha256"):
            raise TransitionError("canonical driver identity changed")
        if git_blob_id(root, driver.read_bytes()) != plan.get("canonicalDriverBlob"):
            raise TransitionError("canonical driver Git blob changed")
        if plan.get("oldDriverSha256") != "33134ce917b94326d7b38798aa2c4b5a5891899ff01774ae0d28f11893065eea" or plan.get("newDriverSha256") != "45722cb1a85a9a5396fd462411fd05a9a3ccc46e28ecc0318c8eb21e331f8a03":
            raise TransitionError("closed reconciliation lineage mismatch")
        updated = dict(policy)
        updated["referenceHookDriverSha256"] = plan["newDriverSha256"]
        history = list(updated.get("repinHistory", []))
        history.append(plan["authorizedHistory"])
        updated["repinHistory"] = history[-8:]
        record["reconciliationPlan"]["authorizedHistory"] = history[-1]
        record["reconciliationPlan"]["originalPolicyBytes"] = record["reconciliationPlan"].get("originalPolicyBytes", policy_bytes.decode("utf-8"))
        updated_bytes = (json.dumps(updated, indent=2, sort_keys=True) + "\n").encode()
        record["reconciliationPlan"]["newPolicySha256"] = sha256_bytes(updated_bytes)
        record["reconciliationPlan"]["authorizedHistory"]["at"] = history[-1].get("at", "")
        record["reconciliationJournal"] = {"phase": "POLICY_PREPARED", "oldPolicySha256": plan["policySha256"], "newPolicySha256": record["reconciliationPlan"]["newPolicySha256"]}
        write_cap(cap_path(root), record)
        _write_policy_atomic(policy_path, updated)
        record["status"] = "APPLIED"
        record["reconciliationReceipt"] = {"operator": operator, "oldDriverSha256": plan["oldDriverSha256"], "newDriverSha256": plan["newDriverSha256"], "at": now()}
        record["reconciliationJournal"]["phase"] = "COMPLETED"
        record.pop("reconciliationJournal", None)
        write_cap(cap_path(root), record)
        return record["capabilityId"]


def _recover_reconciliation(root: pathlib.Path, record: dict) -> str:
    journal = record.get("reconciliationJournal")
    if not isinstance(journal, dict):
        return "NO-ACTION status=APPLIED"
    policy_path = state_dir(root) / "policy.json"
    current = policy_path.read_bytes()
    digest = sha256_bytes(current)
    plan = record.get("reconciliationPlan", {})
    if sha256_bytes(str(plan.get("originalPolicyBytes", "")).encode()) != plan.get("policySha256"):
        raise TransitionError("reconciliation original policy custody is invalid")
    if journal.get("phase") != "POLICY_PREPARED":
        raise TransitionError("reconciliation journal phase is invalid")
    if journal.get("oldPolicySha256") != plan.get("policySha256"):
        raise TransitionError("reconciliation journal old policy hash is unauthorized")
    policy = json.loads(str(plan["originalPolicyBytes"]))
    expected = dict(policy)
    expected["referenceHookDriverSha256"] = plan["newDriverSha256"]
    history = list(expected.get("repinHistory", []))
    history.append(plan.get("authorizedHistory", {"oldDriverSha256": plan["oldDriverSha256"], "newDriverSha256": plan["newDriverSha256"], "canonicalDriverBlob": plan["canonicalDriverBlob"], "capabilityId": record["capabilityId"], "at": ""}))
    expected["repinHistory"] = history[-8:]
    expected_new = (json.dumps(expected, indent=2, sort_keys=True) + "\n").encode()
    if journal.get("newPolicySha256") != sha256_bytes(expected_new):
        raise TransitionError("reconciliation journal new policy hash is unauthorized")
    if digest == journal.get("newPolicySha256"):
        policy = json.loads(current)
        if policy.get("referenceHookDriverSha256") != plan.get("newDriverSha256"):
            raise TransitionError("reconciliation new policy driver identity diverged")
        record["status"] = "APPLIED"
        record["reconciliationReceipt"] = {"operator": "recovered", "oldDriverSha256": plan["oldDriverSha256"], "newDriverSha256": plan["newDriverSha256"], "at": now()}
        record["reconciliationJournalCompleted"] = dict(journal, phase="COMPLETED", outcome="POLICY_APPLIED")
        record.pop("reconciliationJournal", None)
        write_cap(cap_path(root), record)
        return "RECONCILIATION-RECOVERED-TO-APPLIED"
    if digest == journal.get("oldPolicySha256"):
        record["status"] = "PENDING"
        record["reconciliationJournalCompleted"] = dict(journal, phase="COMPLETED", outcome="POLICY_OLD_STATE")
        record.pop("reconciliationJournal", None)
        write_cap(cap_path(root), record)
        return "RECONCILIATION-RECOVERED-TO-PENDING"
    raise TransitionError("reconciliation policy is outside recorded old/new states")


def cancel_driver_policy_reconciliation(root: pathlib.Path, capability_id: str, expected_digest: str, operator: str, reason: str) -> str:
    if not capability_id or not expected_digest or not operator or not reason:
        raise TransitionError("reconciliation cancellation requires id, digest, operator, and reason")
    with locked(state_dir(root) / "state.lock"):
        record = load_cap(root)
        if record.get("kind") != "driver-policy-reconciliation" or record.get("status") != "PENDING":
            raise TransitionError("only pending driver reconciliation may be cancelled")
        if record.get("capabilityId") != capability_id or capability_digest(record) != expected_digest:
            raise TransitionError("reconciliation cancellation identity mismatch")
        if record.get("reconciliationJournal") is not None:
            raise TransitionError("active reconciliation journal cannot be cancelled")
        if git(root, "rev-parse", "main-v2") != record.get("baseSha"):
            raise TransitionError("reconciliation cancellation base changed")
        policy_path = state_dir(root) / "policy.json"
        plan = record.get("reconciliationPlan", {})
        if sha256_bytes(policy_path.read_bytes()) != plan.get("policySha256"):
            raise TransitionError("reconciliation policy changed before cancellation")
        record["status"] = "REJECTED"
        record["rejectedAt"] = now(); record["rejectedBy"] = operator; record["rejectionReason"] = reason
        write_cap(cap_path(root), record)
        return capability_id

def prepare_observer_correction(
    root: pathlib.Path, proposal_root: pathlib.Path, baseline_manifest: pathlib.Path, author: str
) -> dict:
    """Prepare a fresh, separately reviewed observer correction capability.

    The bootstrap capability remains limited to ALLOWED_PATHS. This capability
    authorizes a non-empty subset of the closed six-path observer set: only the
    paths whose proposed bytes differ from the recorded baseline are named, and
    the resulting commit must change exactly those paths.
    """
    if not author or author in {"unknown", ""}:
        raise TransitionError("observer correction requires a named author")
    base = git(root, "rev-parse", "main-v2")
    try:
        baseline = json.loads(baseline_manifest.read_text(encoding="utf-8"))
    except Exception as exc:
        raise TransitionError(f"observer baseline manifest unreadable: {exc}") from exc
    if set(baseline) != OBSERVER_PATHS or any(not SHA40.fullmatch(str(v)) for v in baseline.values()):
        raise TransitionError("observer baseline must contain exactly six valid blobs")
    policy = json.loads((state_dir(root) / "policy.json").read_text(encoding="utf-8"))
    protected = policy.get("protectedEvaluatorBlobs")
    if not isinstance(protected, dict) or any(protected.get(k) != v for k, v in baseline.items()):
        raise TransitionError("observer baseline does not match current protected pins")
    for rel, expected in baseline.items():
        if git(root, "rev-parse", f"{base}:{rel}") != expected:
            raise TransitionError(f"observer baseline does not match canonical base: {rel}")
    original_map = {str(k): str(v) for k, v in sorted(protected.items())}
    content = {}
    proposed = {}
    for rel in sorted(OBSERVER_PATHS):
        src = proposal_root / rel
        if src.is_file():
            data = src.read_bytes()
            blob = git_blob_id(root, data)
        else:
            # A correction names only the files it changes; unnamed observer
            # files keep their recorded baseline bytes.
            data = subprocess.run(
                ["git", "show", f"{base}:{rel}"], cwd=root, capture_output=True, check=True
            ).stdout
            blob = baseline[rel]
        if blob != baseline[rel]:
            content[rel] = data.decode("utf-8")
            proposed[rel] = blob
    if not proposed:
        raise TransitionError("observer proposal does not change any observer file")
    record = {
        "schemaVersion": 2, "capabilityId": "observer-correction-" + secrets.token_hex(8),
        "kind": "observer-correction", "status": "PENDING", "author": author,
        "baseSha": base, "canonicalHead": "main-v2",
        "allowedPaths": sorted(proposed), "proposedBlobs": proposed,
        "proposedContent": content,
        # The reference hook authorizes observer corrections through
        # ``proposedBlobs``; ``authorizedObserverBlobs`` is a bootstrap-only
        # manifest. ``_activation_blockers`` deliberately rejects a non-empty
        # value for any non-bootstrap kind, so an observer correction must
        # leave it empty or it can never activate.
        "authorizedObserverBlobs": {},
        "originalProtectedEvaluatorBlobs": original_map,
        "createdAt": now(),
        "expiresAt": (dt.datetime.now(dt.timezone.utc) + dt.timedelta(seconds=900)).isoformat(),
        "approvals": [],
    }
    with locked(state_dir(root) / "state.lock"):
        if cap_path(root).is_file():
            old = load_cap(root)
            raise TransitionError("capability slot is occupied; archive terminal record before preparing")
        validate_capability_schema(record)
        write_cap(cap_path(root), record)
    return record


def record_receipts(root: pathlib.Path, receipts_dir: pathlib.Path) -> dict:
    """Ingest reviewer-authored receipt files. Structural checks only.

    Receipts must be written by the independent reviewers' own sessions into
    ``receipts_dir`` as ``*.json`` with keys: reviewer, role, verdict,
    capabilityDigest, evidence.  A caller typing names into this CLI does NOT
    create authority; see HUMAN_BOUNDARY.
    """
    if not receipts_dir.is_dir():
        raise TransitionError(f"receipts directory missing: {receipts_dir}")
    with locked(state_dir(root) / "state.lock"):
        record = load_cap(root)
        if record.get("status") != "PENDING":
            raise TransitionError(
                f"receipts only on PENDING capability, is {record.get('status')}"
            )
        digest = capability_digest(record)
        files = sorted(receipts_dir.glob("*.json"))
        if not files:
            raise TransitionError("no receipt files found")
        for file in files:
            try:
                receipt = json.loads(file.read_text(encoding="utf-8"))
            except Exception as exc:
                raise TransitionError(f"unreadable receipt {file.name}: {exc}") from exc
            missing = [k for k in ("reviewer", "role", "verdict", "capabilityDigest") if not receipt.get(k)]
            if missing:
                raise TransitionError(f"receipt {file.name} missing fields: {missing}")
            if receipt.get("capabilityDigest") != digest:
                raise TransitionError(
                    f"receipt {file.name} is bound to a different capability digest"
                )
            if receipt.get("role") not in set(REQUIRED_ROLES) | {"independent"}:
                raise TransitionError(f"receipt {file.name} has untrusted role {receipt.get('role')!r}")
            reviewer = str(receipt.get("reviewer"))
            if reviewer == str(record.get("author")):
                raise TransitionError(f"receipt {file.name}: author may not review own transition")
            if any(str(a.get("reviewer")) == reviewer for a in record.get("approvals", [])):
                raise TransitionError(f"receipt {file.name}: duplicate reviewer {reviewer}")
            record["approvals"].append(
                {
                    "reviewer": reviewer,
                    "role": receipt["role"],
                    "verdict": receipt["verdict"],
                    "capabilityDigest": receipt["capabilityDigest"],
                    "evidence": str(receipt.get("evidence", "")),
                    "receiptFile": file.name,
                    "at": now(),
                }
            )
        write_cap(cap_path(root), record)
    return record


def _finish_policy(root: pathlib.Path, record: dict, applied_sha: str) -> None:
    """Re-pin protected blobs after a landed transition; unchanged entries must
    still match, so tampering outside the authorized paths is rejected."""
    policy_path = state_dir(root) / "policy.json"
    policy = json.loads(policy_path.read_text(encoding="utf-8"))
    allowed = set(record.get("allowedPaths", []))
    for rel, old_blob in policy.get("protectedEvaluatorBlobs", {}).items():
        new_blob = git(root, "rev-parse", f"{applied_sha}:{rel}")
        if rel not in allowed:
            if new_blob != old_blob:
                raise TransitionError(
                    f"unauthorized change to protected evaluator {rel}: "
                    f"{old_blob} -> {new_blob}"
                )
        else:
            if new_blob != record["proposedBlobs"].get(rel, new_blob):
                raise TransitionError(f"landed blob for {rel} does not match capability")
        policy["protectedEvaluatorBlobs"][rel] = new_blob
    if record.get("kind") in {"bootstrap-controller", "lane-sync-controller", "bootstrap-repair"}:
        if record.get("kind") in {"bootstrap-controller", "bootstrap-repair"} and "tools/recovery_ref_hook.py" in record.get("proposedContent", {}):
            driver_bytes = record["proposedContent"]["tools/recovery_ref_hook.py"].encode()
        else:
            driver_bytes = git(root, "show", f"{applied_sha}:tools/recovery_ref_hook.py").encode()
        policy["referenceHookDriverSha256"] = sha256_bytes(driver_bytes)
    policy["transitionApplied"] = {
        "capabilityId": record["capabilityId"],
        "appliedSha": applied_sha,
        "at": now(),
    }
    tmp = policy_path.with_name(f".policy.json.{os.getpid()}.tmp")
    tmp.write_text(json.dumps(policy, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    os.replace(tmp, policy_path)


def _activation_blockers(root: pathlib.Path, record: dict) -> list[str]:
    errors: list[str] = []
    if record.get("status") == "PINNED":
        plan = record.get("pinPlan")
        receipt = record.get("pinReceipt")
        journal = record.get("pinJournal")
        if not isinstance(plan, dict) or not isinstance(receipt, dict):
            errors.append("PINNED capability lacks durable pin plan/receipt")
        elif journal is not None:
            errors.append("pin journal must be complete before activation")
        elif receipt.get("driverGitBlob") != plan.get("driverGitBlob") or receipt.get("wrapperSha256") != plan.get("newWrapperSha256"):
            errors.append("pin receipt does not match the prepared plan")
    head = git(root, "rev-parse", "main-v2")
    if head != record.get("baseSha"):
        errors.append(f"stale base: canonical {head} != capability {record.get('baseSha')}")
    if capability_expired(record):
        errors.append("capability is expired or has invalid expiry")
    errors.extend(validate_approvals(record))
    observer_blobs = record.get("authorizedObserverBlobs")
    if record.get("kind") == "bootstrap-controller":
        if not isinstance(observer_blobs, dict) or not observer_blobs:
            errors.append("exact authorized observer blob manifest is required")
        elif any(not SHA40.fullmatch(str(v)) for v in observer_blobs.values()):
            errors.append("authorized observer blob manifest contains invalid Git blob")
    if record.get("kind") == "bootstrap-repair":
        base = record.get("baseSha")
        try:
            old_cfg = json.loads(git(root, "show", f"{base}:config/recovery-mode.json"))
            new_cfg = json.loads(record.get("proposedContent", {}).get("config/recovery-mode.json", "{}"))
            errors.extend(validate_bootstrap_repair_delta(old_cfg, new_cfg))
        except Exception as exc:
            errors.append(f"cannot validate bootstrap-repair config: {exc}")
    elif record.get("kind") in {"bootstrap-repair", "lane-sync-controller"} and observer_blobs not in (None, {}):
        errors.append("non-bootstrap controller capability cannot authorize observer blobs")
    errors.extend(active_blockers(root))
    errors.extend(revision_blockers(root))
    errors.extend(dirty_blockers(root, list(record.get("allowedPaths", []))))
    if record.get("kind") == "lane-sync-controller":
        try:
            config = json.loads(git(root, "show", f"{record['baseSha']}:config/recovery-mode.json"))
            pin = config["transitionPolicy"]["laneSyncController"]
            if pin != {"kind":"lane-sync-controller", "path":"tools/recovery_state.py", "blob":record["proposedBlobs"]["tools/recovery_state.py"], "maxLifetimeSeconds":60}:
                errors.append("lane-sync controller blob does not match the exact stage-1 hard pin")
        except Exception as exc:
            errors.append(f"lane-sync controller hard pin unavailable: {exc}")
    if record.get("kind") == "bootstrap-controller":
        try:
            base_config = json.loads(git(root, "show", f"{record['baseSha']}:config/recovery-mode.json"))
            proposed = json.loads(record["proposedContent"]["config/recovery-mode.json"])
            if isinstance(proposed.get("transitionPolicy", {}).get("laneSyncController"), dict):
                future_blob = record.get("futureControllerBlob")
                errors.extend(validate_lane_sync_bootstrap_delta(
                    base_config, proposed, {"tools/recovery_state.py": future_blob}
                ))
            else:
                errors.extend(validate_config_delta(base_config, proposed))
        except Exception as exc:
            errors.append(f"cannot validate config delta: {exc}")
    policy_path = state_dir(root) / "policy.json"
    try:
        policy = json.loads(policy_path.read_text(encoding="utf-8"))
        if policy.get("protectedEvaluatorBlobs") != record.get("originalProtectedEvaluatorBlobs"):
            errors.append("runtime protected evaluator blob map changed since prepare")
        for rel, expected in policy.get("protectedEvaluatorBlobs", {}).items():
            if rel in set(record.get("allowedPaths", [])):
                continue
            actual = git(root, "rev-parse", f"main-v2:{rel}")
            if actual != expected:
                errors.append(
                    f"protected evaluator drift before activation: {rel} {expected} -> {actual}"
                )
    except Exception as exc:
        errors.append(f"cannot verify protected evaluator blobs: {exc}")
    return errors


def _commit_landed(root: pathlib.Path, record: dict, head: str) -> bool:
    """Return true only for the exact one-parent authorized commit."""
    try:
        parents = git(root, "rev-list", "--parents", "-n", "1", head).split()
        if parents != [head, record.get("baseSha")]:
            return False
        changed = changed_paths_for_recovery(root, record["baseSha"], head)
        allowed = sorted(record.get("allowedPaths", []))
        kind = record.get("kind")
        if kind in {"bootstrap-repair", "bootstrap-controller", "lane-sync-controller"}:
            if not changed or not set(changed).issubset(set(allowed)):
                return False
        else:
            if changed != allowed:
                return False
        return all(
            git(root, "rev-parse", f"{head}:{rel}") == blob
            for rel, blob in record.get("proposedBlobs", {}).items()
        )
    except (subprocess.CalledProcessError, OSError, ValueError):
        return False


def changed_paths_for_recovery(root: pathlib.Path, base: str, head: str) -> list[str]:
    out = git(root, "diff", "--name-only", f"{base}..{head}")
    return sorted(line for line in out.splitlines() if line)


def _complete_landed(root: pathlib.Path, record: dict, head: str) -> str:
    if not _commit_landed(root, record, head):
        raise TransitionError("canonical movement is not the exact authorized one-parent commit")
    _finish_policy(root, record, head)
    record["status"] = "APPLIED"
    record["appliedSha"] = head
    record["appliedAt"] = now()
    record.pop("proposedContent", None)
    write_cap(cap_path(root), record)
    return "COMPLETED-TO-APPLIED"


def _authorized_canonical_token(root: pathlib.Path) -> str:
    policy = json.loads((state_dir(root) / "ref-policy.json").read_text(encoding="utf-8"))
    expected = str(policy.get("canonicalWriteTokenSha256", ""))
    token_file = state_dir(root) / "canonical-write.token"
    try:
        fd = os.open(token_file, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK)
        st = os.fstat(fd)
        if not stat.S_ISREG(st.st_mode) or st.st_uid != os.getuid() or stat.S_IMODE(st.st_mode) != 0o600 or st.st_size <= 0 or st.st_size > 4096:
            raise TransitionError("canonical token file must be owned regular mode-0600 bounded file")
        data = os.read(fd, 4097)
        if len(data) > 4096:
            raise TransitionError("canonical token file exceeds bound")
    except OSError as exc:
        raise TransitionError("controller canonical-write authority is unavailable") from exc
    finally:
        try: os.close(fd)
        except (UnboundLocalError, OSError): pass
    token = data.decode("utf-8").strip()
    supplied = os.environ.get("OC2_RECOVERY_CANONICAL_TOKEN", "")
    if supplied and hashlib.sha256(supplied.encode()).hexdigest() == expected:
        return supplied
    if token and hashlib.sha256(token.encode()).hexdigest() == expected:
        return token
    raise TransitionError("controller canonical-write authority is unavailable or mismatched")


def archive_capability(root: pathlib.Path) -> pathlib.Path:
    with locked(state_dir(root) / "state.lock"):
        cap = cap_path(root)
        state = state_dir(root)
        validated = load_cap(root)
        capid = str(validated.get("capabilityId", ""))
        if validated.get("status") not in {"APPLIED", "REJECTED"}:
            raise TransitionError("only APPLIED or REJECTED capabilities may be archived")
        if not CAPABILITY_ID.fullmatch(capid):
            raise TransitionError("malformed capability id")
        source_fd = statefd = dirfd = fd = -1
        archive_parent_created = False
        try:
            # Keep the parent directory of the active slot open.  The final
            # unlink and its durability barrier must apply to this directory,
            # not to the archive directory.
            statefd = os.open(state, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
            state_st = os.fstat(statefd)
            if not stat.S_ISDIR(state_st.st_mode) or state_st.st_uid != os.getuid():
                raise TransitionError("recovery state directory is not an owned directory")
            source_fd = os.open(cap, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
            source_st = os.fstat(source_fd)
            if not stat.S_ISREG(source_st.st_mode) or source_st.st_uid != os.getuid() or stat.S_IMODE(source_st.st_mode) != 0o600 or source_st.st_size > 1024 * 1024:
                raise TransitionError("capability record must be owned regular mode-0600 bounded file")
            raw = b""
            while len(raw) < source_st.st_size:
                chunk = os.read(source_fd, min(65536, source_st.st_size - len(raw)))
                if not chunk: raise TransitionError("capability record truncated")
                raw += chunk
            parsed = json.loads(raw.decode("utf-8"))
            validate_capability_schema(parsed)
            if parsed != validated or capability_digest(parsed) != capability_digest(validated):
                raise TransitionError("active capability changed during validation")
            audit = state / "capability-archive"
            try:
                try:
                    dirfd = os.open(audit, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
                except FileNotFoundError:
                    audit.mkdir(mode=0o700)
                    dirfd = os.open(audit, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
                    archive_parent_created = True
                st = os.fstat(dirfd)
                if not stat.S_ISDIR(st.st_mode) or st.st_uid != os.getuid() or stat.S_IMODE(st.st_mode) != 0o700:
                    raise TransitionError("capability archive directory must be owned mode-0700 directory")
                name = f"{capid}.json"
                stage_name = f".{name}.{hashlib.sha256(raw).hexdigest()[:16]}.stage"
                retry_existing = False
                staged = False
                # Prefer reconciliation of an already-present canonical
                # entry before creating any staging bytes. This keeps an
                # exact interrupted archive retry a pure verify/fsync path.
                try:
                    fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=dirfd)
                    retry_existing = True
                    existing_st = os.fstat(fd)
                    if (not stat.S_ISREG(existing_st.st_mode) or existing_st.st_uid != os.getuid() or
                        stat.S_IMODE(existing_st.st_mode) != 0o600 or existing_st.st_nlink not in (1, 2) or
                        existing_st.st_size != len(raw) or existing_st.st_size > 1024 * 1024):
                        raise TransitionError("capability archive collision is unsafe")
                    existing = b""
                    while len(existing) < existing_st.st_size:
                        chunk = os.read(fd, min(65536, existing_st.st_size - len(existing)))
                        if not chunk: raise TransitionError("capability archive is truncated")
                        existing += chunk
                    existing_record = json.loads(existing.decode("utf-8"))
                    validate_capability_schema(existing_record)
                    if (existing != raw or existing_record != validated or str(existing_record.get("capabilityId", "")) != capid or
                        capability_digest(existing_record) != capability_digest(validated)):
                        raise TransitionError("capability archive collision does not match active record")
                    # A supported interruption after publication leaves the
                    # private stage and final names linked to the same inode.
                    # Recover it only when the operation-specific stage name
                    # is discoverable from this process's publication token,
                    # and account for the complete link set before deleting
                    # that one private entry.
                    if existing_st.st_nlink == 2:
                        stage_entry_name = f".{name}.{hashlib.sha256(raw).hexdigest()[:16]}.stage"
                        try:
                            stage_fd = os.open(stage_entry_name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=dirfd)
                        except FileNotFoundError as exc:
                            raise TransitionError("capability archive staging identity is unavailable") from exc
                        try:
                            stage_st = os.fstat(stage_fd)
                            if ((stage_st.st_dev, stage_st.st_ino) != (existing_st.st_dev, existing_st.st_ino) or
                                stage_st.st_nlink != 2 or stage_st.st_uid != os.getuid() or
                                stat.S_IMODE(stage_st.st_mode) != 0o600 or stage_st.st_size != len(raw) or
                                _read_bounded(stage_fd, stage_st.st_size) != raw):
                                raise TransitionError("capability archive staging identity is unsafe")
                        finally:
                            os.close(stage_fd)
                        os.unlink(stage_entry_name, dir_fd=dirfd)
                        os.fsync(dirfd)
                    elif existing_st.st_nlink != 1:
                        raise TransitionError("capability archive collision is unsafe")
                except FileNotFoundError:
                    if fd >= 0:
                        os.close(fd); fd = -1
                if not retry_existing:
                    # Reuse only the deterministic operation-private stage.
                    # A safe incomplete prefix is completed in place; no PID
                    # fallback or directory sweep can create a new identity.
                    try:
                        fd = os.open(stage_name, os.O_RDWR | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=dirfd)
                        stg = os.fstat(fd)
                        if (not stat.S_ISREG(stg.st_mode) or stg.st_uid != os.getuid() or
                            stat.S_IMODE(stg.st_mode) != 0o600 or stg.st_nlink != 1 or
                            stg.st_size > len(raw)):
                            raise TransitionError("capability archive staging collision is unsafe")
                        existing_stage = _read_bounded(fd, stg.st_size)
                        if existing_stage and raw[:len(existing_stage)] != existing_stage:
                            raise TransitionError("capability archive staging bytes do not match active record")
                        os.ftruncate(fd, 0)
                        os.lseek(fd, 0, os.SEEK_SET)
                        written = 0
                        while written < len(raw):
                            n = os.write(fd, raw[written:])
                            if n <= 0: raise OSError(errno.EIO, "short archive write")
                            written += n
                    except FileNotFoundError:
                        fd = os.open(stage_name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600, dir_fd=dirfd)
                        staged = True
                        written = 0
                        while written < len(raw):
                            n = os.write(fd, raw[written:])
                            if n <= 0: raise OSError(errno.EIO, "short archive write")
                            written += n
                try:
                    if retry_existing:
                        pass
                except FileExistsError:
                    raise TransitionError("capability archive collision is unsafe")
                if not retry_existing:
                    os.fsync(fd); os.close(fd); fd = -1
                    verify_fd = os.open(stage_name, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=dirfd)
                    try:
                        if _read_bounded(verify_fd, len(raw)) != raw: raise TransitionError("archived capability bytes failed verification")
                    finally: os.close(verify_fd)
                else:
                    os.fsync(fd)
                if not retry_existing:
                    try:
                        os.link(stage_name, name, src_dir_fd=dirfd, dst_dir_fd=dirfd, follow_symlinks=False)
                    except FileExistsError:
                        # Publication collision: preserve the staged file and
                        # existing audit until the existing entry is proven to
                        # be the exact validated record. Never replace it.
                        os.unlink(stage_name, dir_fd=dirfd)
                        retry_existing = True
                        fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=dirfd)
                        existing_st = os.fstat(fd)
                        if (not stat.S_ISREG(existing_st.st_mode) or existing_st.st_uid != os.getuid() or
                            stat.S_IMODE(existing_st.st_mode) != 0o600 or existing_st.st_nlink != 1 or
                            existing_st.st_size != len(raw) or existing_st.st_size > 1024 * 1024):
                            raise TransitionError("capability archive collision is unsafe")
                        existing = b""
                        while len(existing) < existing_st.st_size:
                            chunk = os.read(fd, min(65536, existing_st.st_size - len(existing)))
                            if not chunk: raise TransitionError("capability archive is truncated")
                            existing += chunk
                        existing_record = json.loads(existing.decode("utf-8"))
                        validate_capability_schema(existing_record)
                        if (existing != raw or existing_record != validated or
                            str(existing_record.get("capabilityId", "")) != capid or
                            capability_digest(existing_record) != capability_digest(validated)):
                            raise TransitionError("capability archive collision does not match active record")
                        os.fsync(fd)
                    if not retry_existing:
                        os.unlink(stage_name, dir_fd=dirfd)
                os.fsync(dirfd)
                if archive_parent_created or retry_existing:
                    # Persist the newly-created archive directory entry only
                    # after the complete archive file and its directory have
                    # reached their durability barriers.  The later
                    # state-directory fsync remains a separate barrier for
                    # active-entry removal itself.
                    os.fsync(statefd)
                current_st = os.fstat(source_fd)
                if (current_st.st_dev, current_st.st_ino, current_st.st_size) != (source_st.st_dev, source_st.st_ino, source_st.st_size) or cap.read_bytes() != raw:
                    raise TransitionError("active capability replaced during archive")
                # The archive is durable before removing the active entry.
                # Unlink relative to the already validated state directory FD,
                # then fsync that same directory.  If this final barrier
                # fails, the archive is still an exact durable recovery
                # artifact, while active-entry durability is explicitly
                # uncertain; do not recreate or mutate the active record.
                os.unlink(cap.name, dir_fd=statefd)
                try:
                    os.fsync(statefd)
                except OSError as exc:
                    raise TransitionError(
                        f"active capability removal durability is uncertain; recover archive {capid}"
                    ) from exc
                return audit / name
            except FileExistsError as exc:
                raise TransitionError("capability archive already exists") from exc
            except (OSError, NotADirectoryError, IsADirectoryError) as exc:
                raise TransitionError("capability archive filesystem operation failed") from exc
            finally:
                if fd >= 0:
                    try: os.close(fd)
                    except OSError: pass
                if dirfd >= 0:
                    try: os.close(dirfd)
                    except OSError: pass
        except (OSError, UnicodeError, json.JSONDecodeError) as exc:
            raise TransitionError("capability record could not be safely archived") from exc
        finally:
            if source_fd >= 0:
                try: os.close(source_fd)
                except OSError: pass
            if statefd >= 0:
                try: os.close(statefd)
                except OSError: pass


def activate(root: pathlib.Path, operator: str) -> str:
    if not operator:
        raise TransitionError("activate requires an explicit operator identity (audit only)")
    lock_path = state_dir(root) / "state.lock"
    with locked(lock_path):
        record = load_cap(root)
        status = record.get("status")
        if capability_expired(record):
            raise TransitionError("capability expired; prepare a fresh capability")
        if status == "APPLIED":
            raise TransitionError("capability already APPLIED; replay forbidden")
        if status == "ARMED":
            raise TransitionError("capability is ARMED but not APPLIED; run recover")
        if status not in {"PENDING", "PINNED"}:
            raise TransitionError(f"capability is {status}; cannot activate")
        errors = _activation_blockers(root, record)
        if errors:
            raise TransitionError("activation refused (fail closed): " + "; ".join(errors))

        record["status"] = "ARMED"
        record["armedAt"] = now()
        record["armedBy"] = operator
        write_cap(cap_path(root), record)

        for rel, text in record["proposedContent"].items():
            target = root / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(text, encoding="utf-8")
        git(root, "add", "--", *record["allowedPaths"])
        env = dict(os.environ)
        env["OC2_RECOVERY_TRANSITION_CAP"] = str(record["capabilityId"])
        env["OC2_RECOVERY_CANONICAL_TOKEN"] = _authorized_canonical_token(root)
        try:
            git(
                root,
                "-c", "user.name=recovery-transition",
                "-c", "user.email=recovery-transition@local",
                "commit", "-m",
                f"recovery: authorized UI phase transition {record['capabilityId']}",
                env=env,
            )
        except subprocess.CalledProcessError as exc:
            # Git may reject an auxiliary AUTO_MERGE ref after moving the
            # canonical head. Inspect the actual ref and commit; never infer
            # success from the exception or whitelist the pseudo-ref.
            actual = git(root, "rev-parse", "main-v2")
            if actual != record.get("baseSha"):
                try:
                    return _complete_landed(root, record, actual)
                except TransitionError:
                    record["status"] = "REJECTED"
                    write_cap(cap_path(root), record)
                    raise TransitionError("commit failed after ambiguous canonical movement; capability rejected") from exc
            raise TransitionError(
                "transition commit was rejected; canonical remains at base; capability remains ARMED for recover. "
                f"Details: {exc.output}"
            ) from exc
        applied = git(root, "rev-parse", "main-v2")
        for rel, blob in record["proposedBlobs"].items():
            actual = git(root, "rev-parse", f"{applied}:{rel}")
            if actual != blob:
                record["status"] = "REJECTED"
                write_cap(cap_path(root), record)
                raise TransitionError(f"post-commit blob mismatch on {rel}: {actual} != {blob}")
        _finish_policy(root, record, applied)
        record["status"] = "APPLIED"
        record["appliedSha"] = applied
        record["appliedAt"] = now()
        record.pop("proposedContent", None)
        write_cap(cap_path(root), record)
    return applied


def recover(root: pathlib.Path) -> str:
    with locked(state_dir(root) / "state.lock"):
        record = load_cap(root)
        status = record.get("status")
        if record.get("kind") == "driver-policy-reconciliation" and isinstance(record.get("reconciliationJournal"), dict):
            if git(root, "rev-parse", "main-v2") != record.get("baseSha"):
                raise TransitionError("reconciliation recovery base changed")
            if capability_expired(record) or validate_approvals(record):
                raise TransitionError("reconciliation recovery approvals or expiry are invalid")
            return _recover_reconciliation(root, record)
        interstage = record.get("interstageJournal")
        if status == "APPLIED" and isinstance(interstage, dict):
            if git(root, "rev-parse", "main-v2") != record.get("appliedSha"):
                raise TransitionError("interstage recovery canonical head is stale")
            if validate_approvals(record):
                raise TransitionError("interstage recovery approvals are invalid")
            expected_blob = interstage.get("expectedDriverBlob")
            if not SHA40.fullmatch(str(expected_blob)):
                raise TransitionError("interstage journal driver blob is invalid")
            landed = subprocess.check_output(["git", "show", f"{record['appliedSha']}:tools/recovery_ref_hook.py"], cwd=root)
            if git_blob_id(root, landed) != expected_blob:
                raise TransitionError("interstage recovery landed driver differs from journal")
            policy_path = state_dir(root) / "policy.json"
            policy_bytes = policy_path.read_bytes()
            policy = json.loads(policy_bytes)
            expected_map = _expected_protected_map(record)
            if policy.get("protectedEvaluatorBlobs") != expected_map:
                raise TransitionError("interstage recovery protected map diverged")
            hook = common_git_dir(root) / "hooks" / "reference-transaction"
            if hook.is_symlink() or not hook.is_file():
                raise TransitionError("interstage recovery wrapper is unsafe")
            current_wrapper = hook.read_bytes()
            old_wrapper = str(interstage.get("oldWrapperText", "")).encode()
            if not old_wrapper or sha256_bytes(old_wrapper) != interstage.get("oldWrapperSha256"):
                raise TransitionError("interstage recovery old wrapper custody is invalid")
            current_hash = sha256_bytes(current_wrapper)
            if current_hash not in {interstage.get("oldWrapperSha256"), interstage.get("newWrapperSha256")}:
                raise TransitionError("interstage recovery wrapper diverged")
            if sha256_bytes(policy_bytes) != interstage.get("oldPolicySha256"):
                raise TransitionError("interstage recovery policy is outside recorded old state")
            if current_hash == interstage.get("newWrapperSha256"):
                tmp = hook.with_name(".reference-transaction.interstage-recover.tmp")
                tmp.write_bytes(old_wrapper); tmp.chmod(0o755); os.replace(tmp, hook)
                _fsync_dir(hook.parent)
            record["interstageJournalCompleted"] = dict(interstage, phase="COMPLETED", outcome="RESTORED_OLD")
            record["interstageReceipt"] = {"driverGitBlob": record.get("pinReceipt", {}).get("driverGitBlob"),
                                            "wrapperSha256": interstage.get("oldWrapperSha256"),
                                            "outcome": "RESTORED_OLD", "at": now()}
            record.pop("interstageJournal", None)
            write_cap(cap_path(root), record)
            return "INTERSTAGE-RECOVERED-TO-OLD"
        journal = record.get("pinJournal")
        if status == "PENDING" and isinstance(journal, dict):
            if git(root, "rev-parse", "main-v2") != journal.get("baseSha"):
                raise TransitionError("pin recovery base changed; refusing filesystem mutation")
            blockers = active_blockers(root) + revision_blockers(root) + validate_approvals(record)
            policy_path = state_dir(root) / "policy.json"
            policy_now = policy_path.read_bytes() if policy_path.is_file() else b""
            if not policy_now or sha256_bytes(policy_now) not in {
                sha256_bytes(str(journal.get("oldPolicyText", "")).encode()),
                str(journal.get("newPolicySha256", "")),
            }:
                blockers.append("policy bytes are outside the recorded pin journal states")
            try:
                policy_obj = json.loads(policy_now)
                if policy_obj.get("protectedEvaluatorBlobs") != record.get("originalProtectedEvaluatorBlobs"):
                    blockers.append("protected evaluator map changed during pin recovery")
            except (ValueError, TypeError):
                blockers.append("policy is not valid JSON during pin recovery")
            if blockers:
                raise TransitionError("pin recovery refused: " + "; ".join(blockers))
            phase = journal.get("phase")
            if phase not in {"BEFORE_FILES", "DRIVER_INSTALLED", "WRAPPER_INSTALLED"}:
                raise TransitionError("pin journal has an unknown phase; recovery refused")
            hooks_dir = common_git_dir(root) / "hooks"
            hook = hooks_dir / "reference-transaction"
            if phase == "WRAPPER_INSTALLED":
                current = hook.read_bytes() if hook.is_file() and not hook.is_symlink() else b""
                if sha256_bytes(current) not in {journal.get("oldWrapperSha256"), journal.get("newWrapperSha256")}:
                    raise TransitionError("pin journal wrapper state diverged; recovery refused")
                old = str(journal.get("oldWrapperText", "")).encode()
                if sha256_bytes(old) != journal.get("oldWrapperSha256"):
                    raise TransitionError("pin journal old wrapper custody is invalid")
                if sha256_bytes(policy_now) == journal.get("newPolicySha256") and sha256_bytes(current) == journal.get("newWrapperSha256"):
                    record["pinJournal"]["phase"] = "POLICY_INSTALLED"
                    record["pinReceipt"] = {"operator": "recovered", "driverSha256": journal["newDriverSha256"],
                                            "driverGitBlob": record["pinPlan"]["driverGitBlob"],
                                            "wrapperSha256": journal["newWrapperSha256"], "at": now()}
                    record["status"] = "PINNED"
                    record.pop("pinJournal", None)
                    write_cap(cap_path(root), record)
                    return "PIN-RECOVERED-TO-PINNED"
                tmp = hook.with_name(".reference-transaction.recover.tmp")
                tmp.write_bytes(old); tmp.chmod(0o755); os.replace(tmp, hook)
                _fsync_dir(hook.parent)
            elif phase in {"BEFORE_FILES", "DRIVER_INSTALLED"}:
                current = hook.read_bytes() if hook.is_file() and not hook.is_symlink() else b""
                old = str(journal.get("oldWrapperText", "")).encode()
                if sha256_bytes(current) not in {journal.get("oldWrapperSha256"), journal.get("newWrapperSha256")}:
                    raise TransitionError("pin journal wrapper state diverged; recovery refused")
                if sha256_bytes(current) == journal.get("newWrapperSha256"):
                    tmp = hook.with_name(".reference-transaction.recover.tmp")
                    tmp.write_bytes(old); tmp.chmod(0o755); os.replace(tmp, hook)
                    _fsync_dir(hook.parent)
            immutable = pathlib.Path(record["pinPlan"]["pinnedPath"])
            if immutable.exists() and immutable.is_file() and not immutable.is_symlink():
                immutable.unlink()
            record.pop("pinJournal", None)
            record["recoveredAt"] = now()
            write_cap(cap_path(root), record)
            return "PIN-RECOVERED-TO-PENDING"
        if status == "ARMED":
            head = git(root, "rev-parse", "main-v2")
            if head == record.get("baseSha"):
                # Interrupted before canonical moved: roll the worktree back to
                # the recorded base contents, then reopen PENDING. Approvals
                # remain bound to the same digest; nothing landed.
                git_ok(root, "reset", "-q", "HEAD", "--", *record.get("allowedPaths", []))
                for rel in record.get("allowedPaths", []):
                    file = root / rel
                    show = subprocess.run(
                        ["git", "show", f"{record['baseSha']}:{rel}"],
                        cwd=root, capture_output=True,
                    )
                    if show.returncode == 0:
                        file.write_bytes(show.stdout)
                    elif file.exists() and rel == "tools/recovery_transition.py":
                        file.unlink()
                record["status"] = "PENDING"
                record["recoveredAt"] = now()
                write_cap(cap_path(root), record)
                return "RECOVERED-TO-PENDING"
            if _commit_landed(root, record, head):
                return _complete_landed(root, record, head)
            record["status"] = "REJECTED"
            write_cap(cap_path(root), record)
            raise TransitionError(
                "ARMED capability diverged from canonical; REJECTED, new capability required"
            )
        return f"NO-ACTION status={status}"


def verify(root: pathlib.Path) -> int:
    """Operator pre-flight: machine-checked facts + explicit human boundary."""
    record = load_cap(root)
    checks = [
        ("capability-status", record.get("status") in {"PENDING", "ARMED", "APPLIED", "PINNED"}),
        ("base-sha-matches-canonical",
         git(root, "rev-parse", "main-v2") == record.get("baseSha")),
        ("approval-structure", not validate_approvals(record)),
        ("queue-idle", not active_blockers(root)),
        ("runtime-revision-consistent", not revision_blockers(root)),
        ("transition-paths-clean", not dirty_blockers(root, list(record.get("allowedPaths", [])))),
        ("protected-map-stable", not _activation_blockers(root, record)),
    ]
    ok = True
    for name, passed in checks:
        print(f"{'PASS' if passed else 'FAIL'} {name}")
        ok = ok and passed
    print(HUMAN_BOUNDARY)
    print(f"capability={record.get('capabilityId')} base={record.get('baseSha')}")
    print(f"digest={capability_digest(record)}")
    return 0 if ok else 1


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__)
        return 2
    try:
        cmd = argv[1]
        if cmd == "approve":
            raise TransitionError(
                "approve is disabled: caller-typed reviewer names are not authority; "
                "use record-receipts <dir> with reviewer-authored receipt files"
            )
        root = pathlib.Path(argv[2]).resolve() if len(argv) > 2 else pathlib.Path.cwd().resolve()
        if cmd == "prepare":
            proposal_root = pathlib.Path(argv[3]).resolve() if len(argv) > 3 else root
            manifest = pathlib.Path(argv[4]).resolve() if len(argv) > 4 else None
            record = prepare(root, proposal_root, os.environ.get("USER", "unknown"), manifest)
        elif cmd == "prepare-bootstrap-repair":
            if len(argv) < 4:
                raise TransitionError("prepare-bootstrap-repair requires <root> <proposal-root>")
            record = prepare_bootstrap_repair(root, pathlib.Path(argv[3]).resolve(), os.environ.get("USER", "unknown"))
            print(f"CAPABILITY-PENDING {record['capabilityId']} base={record['baseSha']}")
        elif cmd == "prepare-lane-sync-controller":
            if len(argv) < 4:
                raise TransitionError("prepare-lane-sync-controller requires <root> <exact-script-path>")
            record = prepare_lane_sync_controller(root, pathlib.Path(argv[3]).resolve(), os.environ.get("USER", "unknown"))
            print(f"CAPABILITY-PENDING {record['capabilityId']} base={record['baseSha']}")
        elif cmd == "prepare-observer":
            if len(argv) < 5:
                raise TransitionError("prepare-observer requires <root> <proposal-root> <baseline-manifest>")
            record = prepare_observer_correction(root, pathlib.Path(argv[3]).resolve(), pathlib.Path(argv[4]).resolve(), os.environ.get("USER", "unknown"))
            print(f"CAPABILITY-PENDING {record['capabilityId']} base={record['baseSha']}")
        elif cmd == "prepare-driver-policy-reconciliation":
            record = prepare_driver_policy_reconciliation(root, os.environ.get("USER", "unknown"))
            print(f"CAPABILITY-PENDING {record['capabilityId']} base={record['baseSha']}")
        elif cmd == "archive":
            archived = archive_capability(root)
            print(f"CAPABILITY-ARCHIVED {archived}")
        elif cmd == "record-receipts":
            record = record_receipts(root, pathlib.Path(argv[3]))
            print(f"RECEIPTS-RECORDED count={len(record['approvals'])}")
            print(HUMAN_BOUNDARY)
        elif cmd == "activate":
            if len(argv) < 4:
                raise TransitionError("activate requires: activate <root> <operator-id>")
            print(f"TRANSITION-APPLIED {activate(root, argv[3])}")
        elif cmd == "recover":
            print(f"TRANSITION-RECOVER {recover(root)}")
        elif cmd == "verify":
            return verify(root)
        elif cmd == "status":
            record = load_cap(root)
            record.pop("proposedContent", None)
            print(json.dumps(record, indent=2, sort_keys=True))
        elif cmd == "pin-original-hook":
            print(f"ORIGINAL-HOOK-PINNED {pin_original_hook(root, argv[3] if len(argv) > 3 else '')}")
        elif cmd == "interstage-hook-refresh":
            print(f"INTERSTAGE-HOOK-REFRESHED {interstage_hook_refresh(root, argv[3] if len(argv) > 3 else '')}")
        elif cmd == "reconcile-driver-policy":
            print(f"DRIVER-POLICY-RECONCILED {reconcile_driver_policy(root, argv[3] if len(argv) > 3 else '')}")
        elif cmd == "cancel-driver-policy-reconciliation":
            if len(argv) < 6:
                raise TransitionError("cancel-driver-policy-reconciliation requires <root> <capability-id> <digest> <operator> <reason>")
            print(f"DRIVER-POLICY-RECONCILIATION-CANCELLED {cancel_driver_policy_reconciliation(root, argv[3], argv[4], argv[5], argv[6] if len(argv) > 6 else '')}")
        else:
            print(f"unknown command: {cmd}", file=sys.stderr)
            return 2
    except TransitionError as exc:
        print(f"TRANSITION REFUSED: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
