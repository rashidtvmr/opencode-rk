#!/usr/bin/env python3
"""Trusted frozen-test evidence checker (COORD-004, stdlib only).

Validates RED/GREEN/integrated-green file evidence pinned to an exact
revision: required artifact kinds present, sha256 hashes match bytes on
disk (128 MiB cap per file), paths stay inside the evidence root
(traversal and symlink rejected). Fails closed on zero tests, edited
status flags, worker PASS self-reports, changed frozen hashes,
self-verification and ignored/skipped suites.

Read-only: every check stats or reads files; nothing is created,
modified, executed or logged beyond allowlisted metadata. One process,
no threads, no network, no subprocess, no environment or secret access.
Bounded: 64 artifacts, 1 MiB read chunks, 64 KiB evidence record.
Denial helpers let tests assert absent side effects and reclaimed FDs
instead of trusting an error string.
"""
from __future__ import annotations

import hashlib
import json
import os
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
HASH = re.compile(r"[0-9a-f]{64}\Z")
REV = re.compile(r"[0-9a-f]{40}\Z")
MAX_ARTIFACT_BYTES = 128 * 1024 * 1024
MAX_ARTIFACTS = 64
REQUIRED_KINDS = frozenset({"red", "green", "integrated-green"})
MAX_EVIDENCE_BYTES = 64 * 1024
_CHUNK = 1024 * 1024


class InvalidEvidence(ValueError):
    """Frozen-test evidence failed closed; denial performed no writes."""


def safe_path(root: pathlib.Path, relative: str) -> pathlib.Path:
    if not isinstance(relative, str) or not relative or "\\" in relative:
        raise InvalidEvidence("invalid relative path")
    rel = pathlib.PurePosixPath(relative)
    if rel.is_absolute() or any(p in {".", ".."} for p in relative.split("/")):
        raise InvalidEvidence(f"unsafe relative path: {relative}")
    root = pathlib.Path(root).resolve()
    result = (root / relative).resolve()
    if result == root or root not in result.parents:
        raise InvalidEvidence(f"path escapes root: {relative}")
    for part in [root / pathlib.Path(*rel.parts[:i]) for i in range(1, len(rel.parts) + 1)]:
        if part.is_symlink():
            raise InvalidEvidence(f"symlink is not an evidence/include file: {relative}")
    return result


def file_hash(path: pathlib.Path) -> str:
    path = pathlib.Path(path)
    if path.stat().st_size > MAX_ARTIFACT_BYTES:
        raise InvalidEvidence("evidence artifact exceeds 128 MiB")
    digest = hashlib.sha256()
    total = 0
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(_CHUNK), b""):
            digest.update(chunk)
            total += len(chunk)
            if total > MAX_ARTIFACT_BYTES:
                raise InvalidEvidence("evidence artifact exceeds 128 MiB")
    return digest.hexdigest()


def open_fd_count() -> int:
    """Current process open-FD count, or -1 when unobservable."""
    try:
        return len(os.listdir("/proc/self/fd"))
    except OSError:
        return -1


def assert_fds_reclaimed(before: int, after: int | None = None) -> None:
    if after is None:
        after = open_fd_count()
    if before >= 0 and after >= 0 and after > before:
        raise InvalidEvidence(f"file descriptors leaked: {before} -> {after}")


def assert_absent(path: pathlib.Path | str) -> None:
    """A denied run left no file behind; a symlink counts as a side effect."""
    probe = pathlib.Path(path)
    if probe.is_symlink() or probe.exists():
        raise InvalidEvidence(f"denied run left a side effect: {path}")


def assert_no_side_effects(before: set, after: set) -> None:
    extra = set(after) - set(before)
    if extra:
        names = sorted(str(p) for p in list(extra)[:8])
        raise InvalidEvidence(f"denied run left side effects: {names}")


def proof_errors(
    proof: dict,
    root: pathlib.Path,
    revision: str,
    required_tests: list[str],
    *,
    expected_frozen_sha256: str | None = None,
    require_command_hash: bool = False,
) -> list[str]:
    if not isinstance(proof, dict):
        return ["missing proof"]
    errors = []
    required = list(required_tests or [])

    status = proof.get("status")
    passed = proof.get("passed")
    accepted_signal = (status == "accepted") or (passed is True)
    if ("status" in proof and status != "accepted") or ("passed" in proof and passed is not True):
        errors.append("edited status flag cannot produce accepted evidence")
    commit = proof.get("testedCommit", proof.get("revision"))
    if not accepted_signal or commit != revision:
        errors.append("missing accepted proof on exact release revision")
    if proof.get("passes") is True and not accepted_signal:
        errors.append("worker self-report passes:true is not evidence")

    count = proof.get("testCount", proof.get("test_count"))
    if type(count) is not int or count < max(1, len(required)):
        errors.append("zero/insufficient real test count")
    obligations = proof.get("testObligations", proof.get("test_obligations", []))
    if not isinstance(obligations, list) or not set(required) <= set(obligations):
        errors.append("missing test obligations")

    frozen = proof.get("frozenTestsSha256", proof.get("frozen_tests_sha256", ""))
    if not HASH.fullmatch(str(frozen)):
        errors.append("missing frozen test hash")
    elif expected_frozen_sha256 is not None and frozen != expected_frozen_sha256:
        errors.append("frozen tests changed")
    if not proof.get("verifier") or not proof.get("implementer") or proof["verifier"] == proof["implementer"]:
        errors.append("independent verifier identity required")
    if proof.get("ignoredSuites", proof.get("ignored", proof.get("skipped", 0))):
        errors.append("ignored/skipped suites fail closed")

    red = proof.get("red")
    if red is not None and (
        not isinstance(red, dict)
        or red.get("compiled") is not True
        or not red.get("exit_code")
        or str(red.get("failure_reason", "")).replace("_", "-") != "missing-behavior"
        or red.get("revision", red.get("testedCommit", commit)) != revision
    ):
        errors.append("red proof must be a compiling test failing for missing behavior")

    command = proof.get("commandSha256", proof.get("command_sha256"))
    if require_command_hash and not HASH.fullmatch(str(command or "")):
        errors.append("missing command hash")
    elif command is not None and not HASH.fullmatch(str(command)):
        errors.append("malformed command hash")

    artifacts = proof.get("artifacts", [])
    if not isinstance(artifacts, list) or not 1 <= len(artifacts) <= MAX_ARTIFACTS:
        return errors + ["missing/bounded artifact list required"]
    if required and not REQUIRED_KINDS <= {a.get("kind") for a in artifacts if isinstance(a, dict)}:
        errors.append("missing RED/GREEN/integrated evidence")
    for artifact in artifacts:
        try:
            if not isinstance(artifact, dict) or not isinstance(artifact.get("kind"), str) or not artifact["kind"]:
                raise InvalidEvidence("malformed artifact entry")
            path = safe_path(pathlib.Path(root), artifact["path"])
            if not HASH.fullmatch(str(artifact.get("sha256", ""))) or file_hash(path) != artifact["sha256"]:
                errors.append("artifact hash mismatch")
        except (OSError, KeyError, TypeError, InvalidEvidence) as exc:
            errors.append(f"invalid artifact: {exc}")
    return errors


def validate_proof(
    proof: dict,
    root: pathlib.Path,
    revision: str,
    required_tests: list[str],
    *,
    expected_frozen_sha256: str | None = None,
    require_command_hash: bool = False,
    max_bytes: int = MAX_EVIDENCE_BYTES,
) -> dict:
    """Raise InvalidEvidence on any gap (writing nothing); else return the
    bounded redacted evidence record tied to the exact revision. The record
    carries allowlisted metadata only: no file contents, no secrets."""
    errors = proof_errors(
        proof, root, revision, required_tests,
        expected_frozen_sha256=expected_frozen_sha256,
        require_command_hash=require_command_hash,
    )
    if errors:
        raise InvalidEvidence("; ".join(errors))
    record = {
        "revision": revision,
        "verifier": proof["verifier"],
        "implementer": proof["implementer"],
        "testCount": proof.get("testCount", proof.get("test_count")),
        "frozenTestsSha256": proof.get("frozenTestsSha256", proof.get("frozen_tests_sha256")),
        "kinds": sorted({a["kind"] for a in proof["artifacts"] if isinstance(a, dict)}),
        "artifacts": [{"kind": a["kind"], "path": a["path"], "sha256": a["sha256"]} for a in proof["artifacts"]],
    }
    payload = (json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")
    if len(payload) > max_bytes:
        raise InvalidEvidence("evidence record exceeds byte budget")
    return record
