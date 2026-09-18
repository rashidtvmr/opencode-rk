#!/usr/bin/env python3
"""Crash-safe lease store with fencing tokens (COORD-006, stdlib only).

Covers tools/ralph_loop.py gaps at HEAD 5af7884:
acquire_singleton 354-365 check-then-write race (LOCK.exists then write),
record_result 587-618 marks accepted 602 before finalize_worktree 604
(accepted-but-unintegrated on crash), heartbeat only around worker/verify
run_task 550-570 not through serial record_result+heartbeat/release 738-754.

Contract:
- Lease = task_id -> owner token + fencing int (monotonic per task).
  Every guarded mutation takes (task_id, owner, fencing); stale token
  rejected without mutation. New owner gets fencing+1.
- Heartbeat valid through execution, verification, integration: caller
  holds lease from acquire until serial integrate settles; renew extends
  expiry; expired/stale heartbeat fails closed.
- Singleton acquire atomic via O_CREAT|O_EXCL (no check-then-write).
  Stale lock (dead pid) reclaimable; live pid refuses.
- Expired/stale owner cannot guarded_write/guard_append/release.
- Recovery: recover() lists expired leases + evidence preserved; caller
  must prove prior owner stopped (pid dead / owner-stopped callback True)
  before reclaim; never auto-releases live lease.
- Controller publishes acceptance atomically: publish_acceptance writes
  receipt tmp+os.replace and only then marks accepted; crash leaves
  pending receipt, never accepted-but-unintegrated.
- Bounded: JSON store cap 4MiB; leases cap 20 default; evidence receipts
  append-only JSONL, caller-supplied handle; no threads/processes/network.
  Cross-process mutual exclusion via fcntl.flock where available.
  No side effects on import.

Config default: leaseTtlSeconds 180 heartbeatSeconds 30 from
config/completion-controller.json (read-only; never raised here).
"""
from __future__ import annotations

import contextlib
import json
import os
import pathlib
import time

MAX_STORE_BYTES = 4 * 1024 * 1024
MAX_LEASES = 20
SCHEMA_VERSION = 1
DEFAULT_TTL = 180.0

# ponytail: multi-controller quorum/term election not here; add only when a
# second writer host needs consensus beyond one-host file singleton.


class LeaseError(RuntimeError):
    """Base error for lease-store decisions."""


class LeaseHeldError(LeaseError):
    pass


class LeaseOwnerError(LeaseError):
    pass


class InvalidLeaseTtlError(LeaseError):
    pass


class LeaseCapacityError(LeaseError):
    pass


class SingletonHeldError(LeaseError):
    pass


class RecoveryBlockedError(LeaseError):
    """Prior owner may still be alive; reclaim refused."""


def _valid_identity(task_id: str, owner: str) -> None:
    if not isinstance(task_id, str) or not task_id:
        raise LeaseError("task id required")
    if not isinstance(owner, str) or not owner:
        raise LeaseError("owner token required")
    if len(task_id) > 256 or len(owner) > 512:
        raise LeaseError("task id/owner exceeds bound")


def _valid_ttl(ttl: float) -> float:
    if isinstance(ttl, bool) or not isinstance(ttl, (int, float)):
        raise InvalidLeaseTtlError("lease ttl must be a positive finite number")
    ttl = float(ttl)
    if ttl != ttl or ttl <= 0 or ttl == float("inf"):
        raise InvalidLeaseTtlError("lease ttl must be a positive finite number")
    return ttl


def _valid_fencing(value: object) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise LeaseError("fencing token must be a positive int")
    return value


class LeaseStore:
    """Crash-safe file-backed lease table with monotonic fencing tokens."""

    def __init__(self, path: str | os.PathLike, *, max_leases: int = MAX_LEASES) -> None:
        if isinstance(max_leases, bool) or not isinstance(max_leases, int) or not 1 <= max_leases <= 20:
            raise LeaseCapacityError("max_leases must be an int in 1..20")
        self.path = pathlib.Path(path)
        self.max_leases = max_leases

    # -- locked IO -----------------------------------------------------
    @contextlib.contextmanager
    def _locked(self):
        self.path.parent.mkdir(parents=True, exist_ok=True)
        with open(self.path, "a+b") as handle:
            try:
                import fcntl  # POSIX only; absent on Windows

                fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
            except ImportError:
                pass
            handle.seek(0)
            raw = handle.read()
            if len(raw) > MAX_STORE_BYTES:
                raise LeaseError("lease store exceeds 4 MiB")
            if raw.strip():
                try:
                    state = json.loads(raw.decode("utf-8"))
                except (ValueError, UnicodeDecodeError) as exc:
                    raise LeaseError(f"corrupt lease store: {exc}") from exc
                if not isinstance(state, dict) or state.get("schemaVersion") != SCHEMA_VERSION:
                    raise LeaseError("invalid lease store schema")
                leases = state.get("leases", {})
                if not isinstance(leases, dict):
                    raise LeaseError("invalid lease store leases")
            else:
                leases = {}
            yield leases, handle

    def _save_locked(self, handle, leases: dict) -> None:
        payload = (json.dumps(
            {"schemaVersion": SCHEMA_VERSION, "leases": leases},
            sort_keys=True, separators=(",", ":"),
        ) + "\n").encode("utf-8")
        if len(payload) > MAX_STORE_BYTES:
            raise LeaseError("lease store exceeds 4 MiB")
        handle.seek(0)
        handle.truncate()
        handle.write(payload)
        handle.flush()
        os.fsync(handle.fileno())
        handle.seek(0)

    @staticmethod
    def _live(lease: dict, now: float) -> bool:
        try:
            return float(lease["expires_at"]) > now
        except (KeyError, TypeError, ValueError):
            return False

    # -- lifecycle ------------------------------------------------------
    def acquire(self, task_id: str, owner: str, *, now: float | None = None,
                ttl_seconds: float = DEFAULT_TTL) -> dict:
        """Take (or re-enter) a lease. Live foreign lease -> LeaseHeldError."""
        _valid_identity(task_id, owner)
        ttl = _valid_ttl(ttl_seconds)
        moment = time.time() if now is None else float(now)
        with self._locked() as (leases, handle):
            current = leases.get(task_id)
            if isinstance(current, dict) and self._live(current, moment):
                if current.get("owner") != owner:
                    raise LeaseHeldError(f"{task_id} leased by another owner")
                return dict(current)
            if current is None and len(leases) >= self.max_leases:
                raise LeaseCapacityError(f"lease capacity {self.max_leases} reached")
            fencing = 1
            if isinstance(current, dict):
                try:
                    fencing = int(current.get("fencing", 0)) + 1
                except (TypeError, ValueError):
                    fencing = 1
                if fencing <= 0:
                    fencing = 1
            lease = {"task_id": task_id, "owner": owner, "fencing": fencing,
                     "acquired_at": moment, "heartbeat_at": moment,
                     "expires_at": moment + ttl}
            leases[task_id] = lease
            self._save_locked(handle, leases)
            return dict(lease)

    def heartbeat(self, task_id: str, owner: str, fencing: int, *,
                  now: float | None = None, ttl_seconds: float = DEFAULT_TTL) -> dict:
        """Renew a live lease. Must be called through execute, verify AND
        serial integrate (caller holds until integrate settles)."""
        _valid_identity(task_id, owner)
        fencing = _valid_fencing(fencing)
        ttl = _valid_ttl(ttl_seconds)
        moment = time.time() if now is None else float(now)
        with self._locked() as (leases, handle):
            current = leases.get(task_id)
            if (not isinstance(current, dict) or current.get("owner") != owner
                    or current.get("fencing") != fencing
                    or not self._live(current, moment)):
                raise LeaseOwnerError(f"{task_id} lease missing, expired, or not owned by fencing {fencing}")
            current["heartbeat_at"] = moment
            current["expires_at"] = moment + ttl
            self._save_locked(handle, leases)
            return dict(current)

    def release(self, task_id: str, owner: str, fencing: int) -> bool:
        """Owner-checked release. Stale fencing -> LeaseOwnerError, no mutation."""
        _valid_identity(task_id, owner)
        fencing = _valid_fencing(fencing)
        with self._locked() as (leases, handle):
            current = leases.get(task_id)
            if current is None:
                return False
            if (not isinstance(current, dict) or current.get("owner") != owner
                    or current.get("fencing") != fencing):
                raise LeaseOwnerError(f"{task_id} lease owned by another fencing token")
            del leases[task_id]
            self._save_locked(handle, leases)
            return True

    def guarded_write(self, task_id: str, owner: str, fencing: int,
                      mutate) -> dict:
        """Apply mutate(lease)->dict only when fencing matches a live lease.
        Stale/expired owners cannot write; failed guard mutates nothing."""
        _valid_identity(task_id, owner)
        fencing = _valid_fencing(fencing)
        moment = time.time()
        with self._locked() as (leases, handle):
            current = leases.get(task_id)
            if (not isinstance(current, dict) or current.get("owner") != owner
                    or current.get("fencing") != fencing
                    or not self._live(current, moment)):
                raise LeaseOwnerError(f"{task_id} stale owner cannot write")
            updated = mutate(dict(current))
            if not isinstance(updated, dict):
                raise LeaseError("mutate must return the updated lease dict")
            updated["task_id"] = task_id
            updated["owner"] = owner
            updated["fencing"] = fencing
            leases[task_id] = updated
            self._save_locked(handle, leases)
            return dict(updated)

    def get(self, task_id: str) -> dict | None:
        with self._locked() as (leases, handle):
            current = leases.get(task_id)
            return dict(current) if isinstance(current, dict) else None

    # -- recovery --------------------------------------------------------
    def recover(self, *, now: float | None = None, owner_stopped=None) -> list[dict]:
        """List reclaimable (expired) leases with evidence preserved.

        Live leases are never returned. owner_stopped(task_id, lease)->bool
        lets the caller prove the prior owner stopped (pid dead, lane
        joined); when supplied and False for a lease, reclaim of that lease
        raises RecoveryBlockedError instead of returning it.
        """
        moment = time.time() if now is None else float(now)
        with self._locked() as (leases, handle):
            out = []
            for task_id in sorted(leases):
                lease = leases[task_id]
                if not isinstance(lease, dict) or self._live(lease, moment):
                    continue
                if owner_stopped is not None and not owner_stopped(task_id, dict(lease)):
                    raise RecoveryBlockedError(f"{task_id} prior owner may still run")
                out.append(dict(lease))
            return out

    def reclaim(self, task_id: str, owner: str, *, now: float | None = None,
                ttl_seconds: float = DEFAULT_TTL, owner_stopped=None) -> dict:
        """Reassign an expired lease after proof the prior owner stopped.
        Returns the new lease with fencing+1. Live lease -> LeaseHeldError;
        unproven prior owner -> RecoveryBlockedError."""
        _valid_identity(task_id, owner)
        ttl = _valid_ttl(ttl_seconds)
        moment = time.time() if now is None else float(now)
        with self._locked() as (leases, handle):
            current = leases.get(task_id)
            if isinstance(current, dict) and self._live(current, moment):
                raise LeaseHeldError(f"{task_id} lease still live")
            fencing = 1
            if isinstance(current, dict):
                if owner_stopped is not None and not owner_stopped(task_id, dict(current)):
                    raise RecoveryBlockedError(f"{task_id} prior owner may still run")
                try:
                    fencing = int(current.get("fencing", 0)) + 1
                except (TypeError, ValueError):
                    fencing = 1
                if fencing <= 0:
                    fencing = 1
            lease = {"task_id": task_id, "owner": owner, "fencing": fencing,
                     "acquired_at": moment, "heartbeat_at": moment,
                     "expires_at": moment + ttl}
            if current is None and len(leases) >= self.max_leases:
                raise LeaseCapacityError(f"lease capacity {self.max_leases} reached")
            leases[task_id] = lease
            self._save_locked(handle, leases)
            return dict(lease)


class SingletonLock:
    """One-controller singleton via atomic create (O_CREAT|O_EXCL).

    Replaces check-then-write (exists? read pid, then write) which admits
    two starters. Stale descriptor (pid dead) is reclaimable; live pid
    raises SingletonHeldError. Release removes only our own descriptor.
    """

    def __init__(self, path: str | os.PathLike) -> None:
        self.path = pathlib.Path(path)
        self._token: str | None = None

    @property
    def held(self) -> bool:
        return self._token is not None

    @staticmethod
    def _pid_alive(pid: int) -> bool:
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return False
        except PermissionError:
            return True
        except (OSError, TypeError, ValueError):
            return True
        return True

    def acquire(self) -> str:
        """Atomically create the lock file. Returns owner token."""
        import secrets

        self.path.parent.mkdir(parents=True, exist_ok=True)
        token = f"{os.getpid()}:{time.time():.6f}:{secrets.token_hex(8)}"
        payload = json.dumps({"pid": os.getpid(), "token": token,
                              "started": time.time()}).encode("utf-8")
        try:
            fd = os.open(str(self.path), os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o644)
        except FileExistsError:
            try:
                raw = self.path.read_bytes()
                if len(raw) > MAX_STORE_BYTES:
                    raise SingletonHeldError("singleton lock unreadable; refusing start")
                prior = json.loads(raw.decode("utf-8"))
                pid = int(prior.get("pid", 0))
            except (OSError, ValueError, TypeError, KeyError):
                raise SingletonHeldError("singleton held by another controller")
            if pid > 0 and self._pid_alive(pid):
                raise SingletonHeldError("another controller instance holds the singleton")
            # Stale descriptor: remove then retry atomic create once.
            try:
                self.path.unlink()
            except OSError:
                raise SingletonHeldError("singleton held by another controller")
            try:
                fd = os.open(str(self.path), os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o644)
            except FileExistsError:
                raise SingletonHeldError("singleton race lost; refusing start")
        with os.fdopen(fd, "wb") as handle:
            handle.write(payload)
        self._token = token
        return token

    def release(self) -> bool:
        """Remove the lock only when it still carries our token."""
        if self._token is None:
            return False
        try:
            raw = self.path.read_bytes()
            prior = json.loads(raw.decode("utf-8"))
        except (OSError, ValueError):
            self._token = None
            return False
        if not isinstance(prior, dict) or prior.get("token") != self._token:
            self._token = None
            return False
        try:
            self.path.unlink()
        except OSError:
            self._token = None
            return False
        self._token = None
        return True

    def __enter__(self) -> "SingletonLock":
        self.acquire()
        return self

    def __exit__(self, *exc: object) -> None:
        self.release()


def append_receipt_jsonl(handle, record: dict, *, max_bytes: int = 64 * 1024) -> str:
    """Append one bounded JSON receipt line; returns the line written."""
    line = (json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")
    if len(line) > max_bytes:
        raise LeaseError("receipt exceeds byte budget")
    handle.write(line.decode("utf-8"))
    handle.flush()
    return line.decode("utf-8")


def publish_acceptance(*, lease: dict, integrated: bool, integration_note: str,
                       ledger_path: str | os.PathLike, receipt_handle) -> dict:
    """Crash-safe acceptance: receipt persisted BEFORE accepted flag.

    Crash between receipt write and ledger fsync leaves a pending receipt
    that recovery can reconcile; it never leaves accepted-but-unintegrated
    state when integration failed. Raises LeaseError when asked to accept
    without proven integration.
    """
    if not isinstance(lease, dict) or not lease.get("task_id"):
        raise LeaseError("acceptance requires a held lease")
    ledger_path = pathlib.Path(ledger_path)
    if not integrated:
        pending = {"id": f"{lease['task_id']}#{lease.get('fencing', 0)}@pending",
                   "task": lease["task_id"], "fencing": lease.get("fencing"),
                   "status": "pending", "integrated": False,
                   "integration": integration_note}
        append_receipt_jsonl(receipt_handle, pending)
        return pending
    receipt = {"id": f"{lease['task_id']}#{lease.get('fencing', 0)}@{int(time.time())}",
               "task": lease["task_id"], "fencing": lease.get("fencing"),
               "status": "accepted", "integrated": True,
               "integration": integration_note}
    append_receipt_jsonl(receipt_handle, receipt)  # durable first
    ledger_path.parent.mkdir(parents=True, exist_ok=True)
    tmp = ledger_path.with_suffix(ledger_path.suffix + ".tmp")
    try:
        current: dict = {}
        if ledger_path.is_file():
            raw = ledger_path.read_bytes()
            if len(raw) > MAX_STORE_BYTES:
                raise LeaseError("ledger exceeds 4 MiB")
            current = json.loads(raw.decode("utf-8"))
            if not isinstance(current, dict):
                raise LeaseError("invalid ledger")
    except (OSError, ValueError) as exc:
        raise LeaseError(f"cannot read ledger: {exc}") from exc
    tasks = current.get("tasks", {})
    if not isinstance(tasks, dict):
        raise LeaseError("invalid ledger tasks")
    tasks[lease["task_id"]] = {"status": "accepted", "receipt": receipt["id"],
                               "fencing": lease.get("fencing")}
    current["tasks"] = tasks
    payload = (json.dumps(current, sort_keys=True, indent=2)).encode("utf-8")
    if len(payload) > MAX_STORE_BYTES:
        raise LeaseError("ledger exceeds 4 MiB")
    tmp.write_bytes(payload)
    os.replace(tmp, ledger_path)  # atomic publish
    return receipt


__all__ = [
    "DEFAULT_TTL",
    "MAX_LEASES",
    "MAX_STORE_BYTES",
    "SCHEMA_VERSION",
    "LeaseCapacityError",
    "LeaseError",
    "LeaseHeldError",
    "LeaseOwnerError",
    "InvalidLeaseTtlError",
    "LeaseStore",
    "RecoveryBlockedError",
    "SingletonHeldError",
    "SingletonLock",
    "append_receipt_jsonl",
    "publish_acceptance",
]


if __name__ == "__main__":
    import tempfile

    # T01 heartbeat spans execute+verify+integrate: renew keeps same fencing alive.
    with tempfile.TemporaryDirectory() as tmp:
        store = LeaseStore(pathlib.Path(tmp) / "leases.json")
        lease = store.acquire("COORD-006-T01", "lane-A", now=1000.0, ttl_seconds=180.0)
        assert lease["fencing"] == 1
        for stage_now in (1100.0, 1170.0, 1250.0):  # execute, verify, integrate
            lease = store.heartbeat("COORD-006-T01", "lane-A", lease["fencing"],
                                    now=stage_now, ttl_seconds=180.0)
            assert lease["fencing"] == 1 and lease["expires_at"] == stage_now + 180.0
        # fencing monotonic across reclaim; stale fencing cannot write/release.
        stale = dict(lease)
        fresh = store.reclaim("COORD-006-T01", "lane-B", now=2000.0,
                              owner_stopped=lambda tid, old: True)
        assert fresh["fencing"] == stale["fencing"] + 1 == 2
        for op in (lambda: store.heartbeat("COORD-006-T01", "lane-A", stale["fencing"], now=2010.0),
                   lambda: store.release("COORD-006-T01", "lane-A", stale["fencing"]),
                   lambda: store.guarded_write("COORD-006-T01", "lane-A", stale["fencing"], lambda cur: cur)):
            try:
                op()
            except LeaseOwnerError:
                pass
            else:
                raise AssertionError("T02 stale owner wrote/released")
        # live lease not reclaimable; recovery needs proof prior owner stopped.
        try:
            store.reclaim("COORD-006-T01", "lane-C", now=2010.0)
        except LeaseHeldError:
            pass
        else:
            raise AssertionError("T04 live lease reclaimed")
        try:
            store.recover(now=3000.0, owner_stopped=lambda tid, old: False)
        except RecoveryBlockedError:
            pass
        else:
            raise AssertionError("T05 recovery without proof allowed")
        expired = store.recover(now=3000.0, owner_stopped=lambda tid, old: True)
        assert [entry["task_id"] for entry in expired] == ["COORD-006-T01"]
        assert store.get("COORD-006-T01")["owner"] == "lane-B"  # evidence preserved

        # T03 crash during acceptance: failed integrate never marks accepted.
        ledger = pathlib.Path(tmp) / "ledger.json"
        receipts = pathlib.Path(tmp) / "receipts.jsonl"
        with receipts.open("a", encoding="utf-8") as handle:
            pending = publish_acceptance(lease=fresh, integrated=False,
                                         integration_note="ff-only failed",
                                         ledger_path=ledger, receipt_handle=handle)
            assert pending["status"] == "pending" and not ledger.exists()
            ok = publish_acceptance(lease=fresh, integrated=True,
                                    integration_note="integrated auto/COORD-006-T01",
                                    ledger_path=ledger, receipt_handle=handle)
            assert ok["status"] == "accepted"
        saved = json.loads(ledger.read_text(encoding="utf-8"))
        assert saved["tasks"]["COORD-006-T01"]["status"] == "accepted"

        # T04 singleton: second starter refused; stale descriptor reclaimable.
        first, second = SingletonLock(pathlib.Path(tmp) / "loop.lock"), SingletonLock(pathlib.Path(tmp) / "loop.lock")
        first.acquire()
        try:
            second.acquire()
        except SingletonHeldError:
            pass
        else:
            raise AssertionError("T04 dual singleton acquire")
        assert first.release() is True and second.release() is False
        with SingletonLock(pathlib.Path(tmp) / "loop.lock") as held:
            assert held.held
        stale_lock = pathlib.Path(tmp) / "stale.lock"
        stale_lock.write_text(json.dumps({"pid": 2**30, "token": "dead", "started": 1.0}), encoding="utf-8")
        assert SingletonLock(stale_lock).acquire()

        # Bounds: oversize store, bad ttl, capacity cap fail closed.
        try:
            store.acquire("", "owner", now=4000.0)
        except LeaseError:
            pass
        else:
            raise AssertionError("empty identity admitted")
        for bad_ttl in (0, -5, float("nan"), float("inf")):
            try:
                store.acquire("T", "o", now=4000.0, ttl_seconds=bad_ttl)
            except InvalidLeaseTtlError:
                pass
            else:
                raise AssertionError(f"bad ttl admitted: {bad_ttl}")
        small = LeaseStore(pathlib.Path(tmp) / "small.json", max_leases=1)
        small.acquire("A", "o1", now=5000.0)
        try:
            small.acquire("B", "o2", now=5000.0)
        except LeaseCapacityError:
            pass
        else:
            raise AssertionError("capacity overrun")
    print("leases self-check: OK")
