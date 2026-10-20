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


CAPABILITY_KINDS = {"bootstrap-controller", "observer-correction"}
CAPABILITY_STATUSES = {"PENDING", "ARMED", "APPLIED", "REJECTED"}
CAPABILITY_ID = re.compile(r"^(?:ui-phase-transition-v2|observer-correction)-[0-9a-f]{16}$")
KIND_PATHS = {"bootstrap-controller": frozenset(ALLOWED_PATHS), "observer-correction": frozenset(OBSERVER_PATHS)}

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
    if not isinstance(allowed, list) or set(allowed) != set(KIND_PATHS[kind]) or len(allowed) != len(set(allowed)):
        raise TransitionError("capability path set does not match its closed kind")
    if not isinstance(proposed, dict) or set(proposed) != set(KIND_PATHS[kind]) or any(not SHA40.fullmatch(str(v)) for v in proposed.values()):
        raise TransitionError("capability blob map does not match its closed kind")
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
    material = json.dumps(
        {
            "kind": record.get("kind", "bootstrap-controller"),
            "baseSha": record.get("baseSha"),
            "allowedPaths": record.get("allowedPaths"),
            "proposedBlobs": record.get("proposedBlobs"),
            "authorizedObserverBlobs": record.get("authorizedObserverBlobs"),
            "originalProtectedEvaluatorBlobs": record.get("originalProtectedEvaluatorBlobs"),
            "expiresAt": record.get("expiresAt"),
        },
        sort_keys=True,
    ).encode()
    return hashlib.sha256(material).hexdigest()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


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
    if old_n != new_n:
        errors.append("config changed outside transitionPolicy")
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
    protected_map = policy.get("protectedEvaluatorBlobs")
    if not isinstance(protected_map, dict) or not protected_map:
        raise TransitionError("runtime protected evaluator blob map missing")
    record["originalProtectedEvaluatorBlobs"] = {str(k): str(v) for k, v in sorted(protected_map.items())}
    path = cap_path(root)
    with locked(state_dir(root) / "state.lock"):
        if path.is_file():
            existing = load_cap(root)
            raise TransitionError("capability slot is occupied; archive terminal record before preparing")
        write_cap(path, record)
    return record


def prepare_observer_correction(
    root: pathlib.Path, proposal_root: pathlib.Path, baseline_manifest: pathlib.Path, author: str
) -> dict:
    """Prepare a fresh, separately reviewed six-observer correction capability.

    The bootstrap capability remains limited to ALLOWED_PATHS. This capability
    has its own exact six-path transaction and must be reviewed separately.
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
        if not src.is_file():
            raise TransitionError(f"observer proposal source missing: {src}")
        data = src.read_bytes()
        content[rel] = data.decode("utf-8")
        proposed[rel] = git_blob_id(root, data)
    record = {
        "schemaVersion": 2, "capabilityId": "observer-correction-" + secrets.token_hex(8),
        "kind": "observer-correction", "status": "PENDING", "author": author,
        "baseSha": base, "canonicalHead": "main-v2",
        "allowedPaths": sorted(OBSERVER_PATHS), "proposedBlobs": proposed,
        "proposedContent": content, "authorizedObserverBlobs": proposed,
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
    if record.get("kind") != "observer-correction":
        driver_bytes = record["proposedContent"]["tools/recovery_ref_hook.py"].encode()
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
    head = git(root, "rev-parse", "main-v2")
    if head != record.get("baseSha"):
        errors.append(f"stale base: canonical {head} != capability {record.get('baseSha')}")
    if capability_expired(record):
        errors.append("capability is expired or has invalid expiry")
    errors.extend(validate_approvals(record))
    observer_blobs = record.get("authorizedObserverBlobs")
    if not isinstance(observer_blobs, dict) or not observer_blobs:
        errors.append("exact authorized observer blob manifest is required")
    elif any(not SHA40.fullmatch(str(v)) for v in observer_blobs.values()):
        errors.append("authorized observer blob manifest contains invalid Git blob")
    errors.extend(active_blockers(root))
    errors.extend(revision_blockers(root))
    errors.extend(dirty_blockers(root, list(record.get("allowedPaths", []))))
    if record.get("kind") != "observer-correction":
        try:
            base_config = json.loads(git(root, "show", f"{record['baseSha']}:config/recovery-mode.json"))
            proposed = json.loads(record["proposedContent"]["config/recovery-mode.json"])
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
        if changed != sorted(record.get("allowedPaths", [])):
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
        if status != "PENDING":
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
        ("capability-status", record.get("status") in {"PENDING", "ARMED", "APPLIED"}),
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
        elif cmd == "prepare-observer":
            if len(argv) < 5:
                raise TransitionError("prepare-observer requires <root> <proposal-root> <baseline-manifest>")
            record = prepare_observer_correction(root, pathlib.Path(argv[3]).resolve(), pathlib.Path(argv[4]).resolve(), os.environ.get("USER", "unknown"))
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
        else:
            print(f"unknown command: {cmd}", file=sys.stderr)
            return 2
    except TransitionError as exc:
        print(f"TRANSITION REFUSED: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
