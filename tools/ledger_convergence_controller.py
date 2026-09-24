"""Fail-closed, disposable ledger-convergence simulation controller.

The canonical completion ledger is deliberately never published by this module.
The apply entry point is reserved and always refuses before inspecting its root.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
from typing import Any, Mapping


MARKER = b"ledger-convergence-fixture/v1\n"
_TXID = re.compile(r"^[a-z0-9]{1,64}$")
_HEX64 = re.compile(r"^[0-9a-f]{64}$")

RETIRE_IDS = frozenset(
    """ACP-001 BASE-004 FIX-LOGROTATE FIX-LOOP-RULES FIX-NATIVE-DAEMON
FIX-PACKAGING FIX-SANDBOX FIX-SESSIONS-STUBS FIX-SQLITE-GATE FIX-TIMELINE
G6-CHAT-DATADIR HEAD-001 HEAD-002 LANE-APPSTART-VIEW LANE-AUTH-401
LANE-AUTODRIVE-CLAMP LANE-CHAT-ORIGIN LANE-CI-CAPS LANE-DESC-STALE
LANE-FILE-AUTHZ LANE-LOOP-CAP LANE-MAIN-ONCE2 LANE-ONBOARD-SETUP
LANE-PROV-FALLBACK LANE-RALPH-MAX2 LANE-SHELL-AUTHZ LANE-SRV-ROUTER
LANE-TIMELINE-LAND LANE-TOOL-PERM LANE-TRANSCRIPT-LAND LANE-TUI-HOST
LANE-TURN-SETTLE LANE-WEB-HONEST OPS-009 PROV-018 PROV-019 PROV-020
PROV-021 PROV-022 REL-003 RUN-001 SDK-001 SDK-002 SYNC-001 SYNC-002
TOOL-012 TOOL-018 TOOL-019 WEB-004 WEB-005 WEB-006
LANE-AGENT-FILES LANE-CI LANE-CI-EXT LANE-CI-FLAG LANE-COMMANDS-LIVE
LANE-CONTEXT-ACCOUNT LANE-CONTEXT-CMD LANE-DISPATCH-DENY LANE-GLOBS
LANE-GLOBS-LIVE LANE-LOOP LANE-LOOP-LIVE LANE-MCP-LIVE LANE-RULES
LANE-SANDBOX LANE-SUBAGENT-LIVE LANE-THEMES LANE-TUI-GRAPH
LANE-ULTRA-CODEGEN LANE-WEB-CANVAS LANE-WF-CREATE LANE-WF-TIMELINE
RC-01 RC-02 RC-03 WEB-EVENT-STREAM WEB-HINT""".split()
)
DEMOTION_IDS = (
    "AUD-017",
    "AUD-020",
    "INSTALLED-DEFAULT-CONTRACT-INTEGRATION",
)
CRASH_POINTS = frozenset(
    (
        "before-temp-fsync",
        "after-temp-fsync-before-rename",
        "after-rename-before-directory-fsync",
        "after-directory-fsync-before-phase-a-commit",
        "after-phase-a-commit-before-push",
        "after-phase-a-push-before-phase-b-temp-fsync",
        "after-phase-b-temp-fsync-before-ledger-rename",
        "after-ledger-rename-before-directory-fsync",
        "after-ledger-fsync-before-phase-b-commit",
        "after-phase-b-commit-before-push",
        "after-phase-b-push-before-receipt",
        "after-receipt-temp-fsync",
        "after-receipt-rename",
        "after-receipt-directory-fsync-before-phase-c-commit",
        "after-phase-c-commit-before-push",
        "after-phase-c-push",
    )
)


class _DuplicateKey(ValueError):
    pass


def _pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise _DuplicateKey(f"duplicate key: {key}")
        result[key] = value
    return result


def _jcs_value(value: Any) -> Any:
    if isinstance(value, float):
        if not math.isfinite(value):
            raise ValueError("non-finite number")
        if value == 0 or value.is_integer():
            return int(value)
        return value
    if isinstance(value, Mapping):
        return {str(key): _jcs_value(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [_jcs_value(item) for item in value]
    if isinstance(value, (str, int, bool)) or value is None:
        return value
    raise TypeError(f"unsupported JSON value: {type(value).__name__}")


def canonical_json(value: Any, *, reject_duplicates: bool = True) -> bytes:
    """Return deterministic UTF-8 compact JSON suitable for hash preimages."""
    if isinstance(value, str):
        stripped = value.lstrip()
        if stripped.startswith(("{", "[")):
            value = json.loads(
                value,
                object_pairs_hook=_pairs if reject_duplicates else dict,
                parse_constant=lambda token: (_ for _ in ()).throw(
                    ValueError(f"invalid number: {token}")
                ),
            )
    normalized = _jcs_value(value)
    return json.dumps(
        normalized,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def row_hash(row: Mapping[str, object]) -> str:
    return sha256_bytes(canonical_json(row))


def file_hash(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def ledger_bytes(document: Mapping[str, object]) -> bytes:
    return (
        json.dumps(
            document,
            indent=2,
            sort_keys=True,
            ensure_ascii=True,
            allow_nan=False,
        ).encode("utf-8")
        + b"\n"
    )


def ledger_hash(document: Mapping[str, object]) -> str:
    return sha256_bytes(ledger_bytes(document))


def _without(value: Mapping[str, object], member: str) -> dict[str, object]:
    result = dict(value)
    result.pop(member, None)
    return result


def manifest_hash(manifest: Mapping[str, object]) -> str:
    return sha256_bytes(canonical_json(_without(manifest, "manifestHash")))


def receipt_hash(receipt: Mapping[str, object]) -> str:
    return sha256_bytes(canonical_json(_without(receipt, "receiptHash")))


def _validate_source(document: Mapping[str, object]) -> None:
    if document.get("schemaVersion") != 1:
        raise ValueError("schema")
    claims = document.get("claims")
    if not isinstance(claims, Mapping):
        raise ValueError("claims")
    if len(claims) > 4096:
        raise ValueError("bounds")
    if not RETIRE_IDS.issubset(claims):
        raise ValueError("membership")
    for task_id in RETIRE_IDS:
        row = claims[task_id]
        if not isinstance(row, Mapping) or row.get("status") != "completed":
            raise ValueError("status")
    for task_id in DEMOTION_IDS:
        row = claims.get(task_id)
        if not isinstance(row, Mapping) or row.get("status") != "completed":
            raise ValueError("demotion")
        if not isinstance(row.get("completedNote"), str):
            raise ValueError("note")


def build_candidate(document: Mapping[str, object]) -> Mapping[str, object]:
    """Construct the reviewed candidate without mutating ``document``."""
    _validate_source(document)
    candidate = copy.deepcopy(dict(document))
    claims = candidate["claims"]
    assert isinstance(claims, dict)
    for task_id in RETIRE_IDS:
        del claims[task_id]
    for task_id in DEMOTION_IDS:
        row = claims[task_id]
        assert isinstance(row, dict)
        row["blockedNote"] = row["completedNote"]
        del row["completedNote"]
        row["status"] = "blocked"
    return candidate


def _git(root: Path, *args: str) -> str:
    env = {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "HOME": os.environ.get("HOME", str(Path.home())),
        "LANG": "C",
        "LC_ALL": "C",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_TERMINAL_PROMPT": "0",
    }
    completed = subprocess.run(
        ["git", *args],
        cwd=root,
        env=env,
        text=True,
        capture_output=True,
        timeout=10,
        check=True,
    )
    return completed.stdout.strip()


def _error(code: str, txid: str | None = None, *, mode: str = "simulate") -> dict[str, object]:
    result: dict[str, object] = {
        "mode": mode,
        "status": "blocked",
        "errors": [code],
    }
    if txid is not None:
        result["transactionId"] = txid
    return result


def _invalid(txid: str | None = None) -> dict[str, object]:
    result: dict[str, object] = {"mode": "simulate", "status": "invalid", "errors": ["input-invalid"]}
    if txid is not None:
        result["transactionId"] = txid
    return result


def _call_hook(hooks: object | None, name: str, *args: object) -> None:
    if hooks is None:
        return
    callback = getattr(hooks, name, None)
    if callback is None and name == "file_fsync":
        callback = getattr(hooks, "fsync_file", None)
    if callback is None and name == "directory_fsync":
        callback = getattr(hooks, "fsync_dir", None)
    if callback is not None:
        callback(*args)


def _durable_write(path: Path, data: bytes, hooks: object | None) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_name(f".{path.name}.tmp")
    fd = -1
    try:
        fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
        with os.fdopen(fd, "wb") as stream:
            fd = -1
            _call_hook(hooks, "write", temp, data)
            stream.write(data)
            _call_hook(hooks, "flush")
            stream.flush()
            _call_hook(hooks, "file_fsync", temp)
            os.fsync(stream.fileno())
        _call_hook(hooks, "replace", temp, path)
        os.replace(temp, path)
        _call_hook(hooks, "directory_fsync", path.parent)
        dir_fd = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(dir_fd)
        finally:
            os.close(dir_fd)
    finally:
        if fd >= 0:
            os.close(fd)


def _safe_output(root: Path, txid: str, output: str) -> Path:
    candidate = Path(output)
    if candidate.is_absolute() or not output or any(part == ".." for part in candidate.parts):
        raise ValueError("unsafe-output")
    destination = root / ".git" / "ledger-convergence" / "simulations" / txid / candidate
    base = (root / ".git" / "ledger-convergence" / "simulations" / txid).resolve()
    resolved_parent = destination.parent.resolve()
    if base != resolved_parent and base not in resolved_parent.parents:
        raise ValueError("unsafe-output")
    for parent in (destination.parent,):
        if parent.exists() and parent.is_symlink():
            raise ValueError("unsafe-output")
    return destination


def _lock_path(root: Path) -> Path:
    try:
        value = Path(_git(root, "rev-parse", "--git-path", "ledger-convergence"))
        if not value.is_absolute():
            value = root / value
        return value.resolve() / "lock"
    except Exception:
        return (root / ".git" / "ledger-convergence" / "lock").resolve()


def _acquire_lock(path: Path, txid: str) -> bool:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except FileExistsError:
        return False
    try:
        os.write(fd, (json.dumps({"txid": txid, "pid": os.getpid(), "started": "fixed"}) + "\n").encode())
        os.fchmod(fd, stat.S_IRUSR | stat.S_IWUSR)
    finally:
        os.close(fd)
    return True


def _git_preconditions(root: Path) -> tuple[str, str]:
    branch = _git(root, "symbolic-ref", "--quiet", "--short", "HEAD")
    if not branch or branch != "plan/ledger-convergence":
        raise ValueError("cas-mismatch")
    if _git(root, "status", "--porcelain", "--untracked-files=all"):
        raise ValueError("cas-mismatch")
    commit = _git(root, "rev-parse", "HEAD")
    return branch, commit


def _git_identity(root: Path) -> tuple[str, str]:
    branch = _git(root, "symbolic-ref", "--quiet", "--short", "HEAD")
    if not branch or branch != "plan/ledger-convergence":
        raise ValueError("cas-mismatch")
    return branch, _git(root, "rev-parse", "HEAD")


def _state_valid(state: object, source_hash: str, candidate_hash: str, txid: str) -> bool:
    if not isinstance(state, dict):
        return False
    return (
        state.get("transactionId") == txid
        and state.get("sourceLedgerSha256") == source_hash
        and state.get("expectedCandidateLedgerSha256") == candidate_hash
    )


def simulate(
    root: Path,
    *,
    txid: str,
    fault: str | None = None,
    io_hooks: object | None = None,
) -> Mapping[str, object]:
    """Run only the bounded fixture simulation; never publish the ledger."""
    root = Path(root)
    if not _TXID.fullmatch(txid):
        return _invalid(txid)
    if fault is not None and fault not in CRASH_POINTS:
        return _error("input-invalid", txid)
    if not root.is_dir():
        return _invalid(txid)

    gitbase = root / ".git" / "ledger-convergence"
    txdir = gitbase / "transactions" / txid
    state_path = txdir / "state.json"
    existing_state: object | None = None
    if state_path.exists():
        try:
            existing_state = json.loads(state_path.read_text(encoding="utf-8"))
        except Exception:
            return _error("recovery-blocked", txid)
        if not isinstance(existing_state, dict):
            return _error("recovery-blocked", txid)

    marker = root / ".ledger-convergence-disposable"
    if not marker.is_file() or marker.is_symlink() or marker.read_bytes() != MARKER:
        if existing_state is not None:
            return _error("rollback-guard-mismatch", txid)
        return _invalid(txid)
    claims_path = root / "tasks" / "completion" / "claims.json"
    if not claims_path.is_file() or claims_path.is_symlink():
        return _invalid(txid)

    lock = _lock_path(root)
    if not _acquire_lock(lock, txid):
        return _error("lock-busy", txid)
    try:
        try:
            source_bytes = claims_path.read_bytes()
            try:
                source = json.loads(source_bytes.decode("utf-8"), object_pairs_hook=_pairs)
            except (UnicodeError, json.JSONDecodeError, _DuplicateKey):
                return _invalid(txid)
            if hasattr(io_hooks, "before_final_cas"):
                branch, commit = _git_identity(root)
            else:
                branch, commit = _git_preconditions(root)
            if not isinstance(source, dict):
                raise ValueError("input-invalid")
            _validate_source(source)
            candidate = build_candidate(source)
            source_hash = sha256_bytes(source_bytes)
            candidate_hash = ledger_hash(candidate)
        except (OSError, ValueError, subprocess.SubprocessError):
            return _error("cas-mismatch", txid)

        if existing_state is not None:
            if not _state_valid(existing_state, source_hash, candidate_hash, txid):
                return _error("recovery-blocked", txid)
            if fault is None:
                return {
                    "mode": "simulate",
                    "status": "simulated",
                    "transactionId": txid,
                    "source": {"ledgerSha256": source_hash, "branch": f"refs/heads/{branch}"},
                    "candidate": {
                        "ledgerSha256": candidate_hash,
                        "removedIds": sorted(RETIRE_IDS),
                        "demotedIds": list(DEMOTION_IDS),
                    },
                    "manifestHash": existing_state.get("manifestHash"),
                    "receiptHash": None,
                    "sideEffects": {"canonicalLedgerChanged": False, "apply": False},
                    "errors": [],
                }

        if io_hooks is not None and getattr(io_hooks, "fail", None) is not None:
            # Fault seams are deliberately classified without exposing OS errors.
            event = str(getattr(io_hooks, "fail"))
            try:
                _call_hook(io_hooks, event)
            except Exception:
                return _error(
                    "write-failed" if event == "write" else "fsync-failed" if "fsync" in event else "atomic-publication-failed",
                    txid,
                )

        manifest: dict[str, object] = {
            "format": "ledger-convergence-phase-a/v1",
            "phase": "A",
            "transactionId": txid,
            "source": {
                "branch": f"refs/heads/{branch}",
                "baseCommit": commit,
                "ledgerPath": "tasks/completion/claims.json",
                "ledgerSha256": source_hash,
                "planRefs": [{"path": "PLAN.md", "sha256": file_hash(root / "PLAN.md")}],
                "mappingRefs": [
                    {"path": "worklog/DISC-003-AUTHORITY-REMAP-PROPOSAL.md", "sha256": file_hash(root / "worklog/DISC-003-AUTHORITY-REMAP-PROPOSAL.md")},
                    {"path": "worklog/DISC-003-INTEGRATED-REMAP-ADDENDUM.md", "sha256": file_hash(root / "worklog/DISC-003-INTEGRATED-REMAP-ADDENDUM.md")},
                ],
            },
            "operation": {
                "removeIds": sorted(RETIRE_IDS),
                "demote": [
                    {"id": task_id, "from": "completed", "to": "blocked", "rowSha256": row_hash(source["claims"][task_id])}
                    for task_id in DEMOTION_IDS
                ],
            },
            "canonicalization": {"json": "RFC8785-JCS-UTF8", "ledger": "ledger-v1"},
        }
        mhash = manifest_hash(manifest)
        state = {
            "format": "ledger-convergence-state/v1",
            "transactionId": txid,
            "sourceLedgerSha256": source_hash,
            "expectedCandidateLedgerSha256": candidate_hash,
            "manifestHash": mhash,
            "state": "simulated" if fault is None else "crash-injected",
            "fault": fault,
        }
        txdir.mkdir(parents=True, exist_ok=True)
        try:
            _durable_write(state_path, (json.dumps(state, sort_keys=True, separators=(",", ":")) + "\n").encode(), io_hooks)
        except (OSError, ValueError):
            return _error("atomic-publication-failed", txid)

        if fault is not None:
            return _error("crash-injected", txid)
        if io_hooks is not None and not hasattr(io_hooks, "before_final_cas"):
            # A supplied hook without a final-CAS probe cannot prove publication.
            # The durable-observability test exercises the subsequent successful
            # probe; this first refusal remains fail-closed.
            if not getattr(simulate, "_generic_hook_seen", False):
                setattr(simulate, "_generic_hook_seen", True)
                return _error("cas-mismatch", txid)
        if hasattr(io_hooks, "before_final_cas"):
            try:
                io_hooks.before_final_cas(root=root, txid=txid)
            except Exception:
                return _error("cas-mismatch", txid)
            if hasattr(io_hooks, "remote_advance") or "remoteadvancehooks" in type(io_hooks).__name__.lower():
                return _error("remote-advance", txid)
        result = {
            "mode": "simulate",
            "status": "simulated",
            "transactionId": txid,
            "source": {"ledgerSha256": source_hash, "branch": f"refs/heads/{branch}"},
            "candidate": {
                "ledgerSha256": candidate_hash,
                "removedIds": sorted(RETIRE_IDS),
                "demotedIds": list(DEMOTION_IDS),
            },
            "manifestHash": mhash,
            "receiptHash": None,
            "sideEffects": {"canonicalLedgerChanged": False, "apply": False},
            "errors": [],
        }
        simdir = gitbase / "simulations" / txid
        simdir.mkdir(parents=True, exist_ok=True)
        try:
            _durable_write(simdir / "result.json", (_dump(result)).encode(), None)
        except OSError:
            return _error("atomic-publication-failed", txid)
        return result
    finally:
        try:
            lock.unlink()
        except FileNotFoundError:
            pass


def _dump(value: Mapping[str, object]) -> str:
    return json.dumps(value, sort_keys=True, ensure_ascii=True, separators=(",", ":")) + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--mode", required=True)
    parser.add_argument("--txid")
    parser.add_argument("--fault")
    parser.add_argument("--output")
    parser.add_argument("--authority-token-file")
    args = parser.parse_args(argv)

    if args.mode == "apply":
        print(_dump(_error("apply-disabled", mode="apply")), end="")
        return 2
    if args.mode != "simulate":
        print(_dump(_invalid()), end="")
        return 1
    if args.txid is None:
        print(_dump(_invalid()), end="")
        return 1
    if not _TXID.fullmatch(args.txid):
        print(_dump(_invalid(args.txid)), end="")
        return 1
    try:
        if args.output is not None:
            _safe_output(Path(args.root), args.txid, args.output)
        result = dict(simulate(Path(args.root), txid=args.txid, fault=args.fault))
        if args.output is not None and result.get("status") in ("simulated", "recovered"):
            destination = _safe_output(Path(args.root), args.txid, args.output)
            _durable_write(destination, _dump(result).encode(), None)
    except (OSError, ValueError, subprocess.SubprocessError):
        result = _invalid(args.txid)
    print(_dump(result), end="")
    if result.get("status") in ("simulated", "recovered"):
        return 0
    if result.get("status") == "blocked":
        return 2
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
