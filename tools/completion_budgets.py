#!/usr/bin/env python3
"""Budget enforcer for completion runs (stdlib only).

Enforces the bounds behind COORD-007 without launching work itself:
memory-admission gate over the whole process tree, a single heavyweight
validation semaphore, provider concurrency/rate/cost limits with Retry-After,
a capped repairable-retry policy, and bounded logs/output with full reclaim
on shutdown.

Thresholds come from config/completion-controller.json (hostBudgetMiB 8192,
reservedMemoryMiB 2048, stopAdmittingBelowAvailableMiB 1024, cargoBuildJobs 2,
maxHeavyValidations 1, maxAttemptsPerTask 3, allowBudgetIncrease false).
This module never raises budgets; the operator-owned config is read-only here.
"""
from __future__ import annotations

import collections
import contextlib
import os
import pathlib
import subprocess
import time
from dataclasses import dataclass, field

PROC = pathlib.Path("/proc")

# Failures that must block immediately: never retried, never converted into a
# repairable wait. Matched case-insensitively as substrings so adapter-defined
# error kinds ("auth-expired", "SecurityDenied", "SIGNING_ERROR", ...) close.
BLOCKED_SUBSTRINGS = frozenset({
    "auth", "unauthor", "forbidden", "credential", "token-expired",
    "security", "sandbox", "isolation",
    "sign", "signature",
    "budget", "cost", "quota", "payment", "billing",
})

DEFAULT_MAX_ATTEMPTS = 3
DEFAULT_STOP_BELOW_MIB = 1024
DEFAULT_MAX_BYTES = 262144
DEFAULT_MAX_ENTRIES = 1000
MAX_RETRY_AFTER_SECONDS = 3600.0


@dataclass(frozen=True)
class BudgetConfig:
    host_budget_mib: int = 8192
    reserved_mib: int = 2048
    stop_below_available_mib: int = 1024
    cargo_build_jobs: int = 2
    max_heavy_validations: int = 1
    max_attempts: int = 3

    @classmethod
    def from_mapping(cls, cfg: dict) -> "BudgetConfig":
        try:
            host = cfg["hostBudgetMiB"]
            reserved = cfg["reservedMemoryMiB"]
            stop_below = cfg["stopAdmittingBelowAvailableMiB"]
            jobs = cfg["cargoBuildJobs"]
            heavy = cfg["maxHeavyValidations"]
            attempts = cfg["maxAttemptsPerTask"]
            allow_raise = cfg["allowBudgetIncrease"]
        except (KeyError, TypeError) as exc:
            raise ValueError(f"missing controller budget key: {exc}") from exc
        for name, value in (("hostBudgetMiB", host), ("reservedMemoryMiB", reserved),
                            ("stopAdmittingBelowAvailableMiB", stop_below)):
            if type(value) is not int or value <= 0:
                raise ValueError(f"invalid controller setting {name}")
        if type(jobs) is not int or not 1 <= jobs <= 2:
            raise ValueError("cargoBuildJobs must be 1 or 2")
        if heavy != 1:
            raise ValueError("maxHeavyValidations must be exactly 1")
        if type(attempts) is not int or not 1 <= attempts <= 3:
            raise ValueError("maxAttemptsPerTask must be between 1 and 3")
        if reserved >= host:
            raise ValueError("reservedMemoryMiB must be smaller than hostBudgetMiB")
        if allow_raise is not False:
            raise ValueError("implicit provider budget increase forbidden")
        return cls(host, reserved, stop_below, jobs, heavy, attempts)


def load_controller_config(path: str | os.PathLike = "config/completion-controller.json",
                           *, root: str | os.PathLike | None = None) -> BudgetConfig:
    """Read the operator-owned controller config. Raises ValueError if unsafe."""
    import json

    candidate = pathlib.Path(root) / path if root is not None else pathlib.Path(path)
    if candidate.stat().st_size > 1024 * 1024:
        raise ValueError("controller config exceeds 1 MiB")
    raw = json.loads(candidate.read_text(encoding="utf-8"))
    if not isinstance(raw, dict):
        raise ValueError("controller config must be a JSON object")
    return BudgetConfig.from_mapping(raw)


def mem_available_mib(meminfo: str | os.PathLike = "/proc/meminfo") -> int | None:
    """Host-available memory in MiB. None when unmeasurable (fail closed)."""
    try:
        text = pathlib.Path(meminfo).read_text(encoding="utf-8")
    except OSError:
        return None
    available_kb: int | None = None
    free_kb: int | None = None
    for line in text.splitlines():
        if line.startswith("MemAvailable:"):
            available_kb = int(line.split()[1])
            break
        if line.startswith("MemFree:"):
            free_kb = int(line.split()[1])
    kb = available_kb if available_kb is not None else free_kb
    if kb is None or kb < 0:
        return None
    return kb // 1024


def _proc_children() -> dict[int, list[int]]:
    """ppid -> [child pids] snapshot from /proc. Raises OSError when absent."""
    children: dict[int, list[int]] = {}
    for entry in PROC.iterdir():
        if not entry.name.isdigit():
            continue
        try:
            stat = (entry / "stat").read_text(encoding="utf-8")
        except OSError:
            continue
        tail = stat[stat.rfind(")") + 1:].split()
        if len(tail) < 2:
            continue
        try:
            pid, ppid = int(entry.name), int(tail[1])
        except ValueError:
            continue
        children.setdefault(ppid, []).append(pid)
    return children


def _rss_kb_from_proc(pid: int) -> int:
    try:
        for line in (PROC / str(pid) / "status").read_text(encoding="utf-8").splitlines():
            if line.startswith("VmRSS:"):
                return max(0, int(line.split()[1]))
    except (OSError, ValueError, IndexError):
        pass
    return 0


def _tree_rss_via_ps(root_pid: int) -> int | None:
    """Fallback when /proc is unavailable: walk ppid links with ps(1)."""
    try:
        out = subprocess.run(
            ["ps", "-eo", "pid=,ppid=,rss="], capture_output=True, text=True,
            timeout=10, check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if out.returncode != 0:
        return None
    children: dict[int, list[int]] = {}
    rss: dict[int, int] = {}
    for line in out.stdout.splitlines():
        parts = line.split()
        if len(parts) != 3:
            continue
        try:
            pid, ppid, kb = int(parts[0]), int(parts[1]), int(parts[2])
        except ValueError:
            continue
        children.setdefault(ppid, []).append(pid)
        rss[pid] = max(0, kb)
    if root_pid not in rss and root_pid not in children:
        return None
    total, queue = 0, collections.deque([root_pid])
    seen = {root_pid}
    while queue:
        pid = queue.popleft()
        total += rss.get(pid, 0)
        for child in children.get(pid, ()):
            if child not in seen:
                seen.add(child)
                queue.append(child)
    return total // 1024


def process_tree_rss_mib(root_pid: int | None = None) -> int | None:
    """RSS of root_pid plus all descendants in MiB. None when unmeasurable."""
    root = os.getpid() if root_pid is None else root_pid
    if type(root) is not int or root <= 0:
        raise ValueError("positive pid required")
    if PROC.is_dir():
        try:
            children = _proc_children()
        except OSError:
            return _tree_rss_via_ps(root)
        total_kb, queue, seen = 0, collections.deque([root]), {root}
        while queue:
            pid = queue.popleft()
            total_kb += _rss_kb_from_proc(pid)
            for child in children.get(pid, ()):
                if child not in seen:
                    seen.add(child)
                    queue.append(child)
        return total_kb // 1024
    return _tree_rss_via_ps(root)


def admission_allowed(available_mib: int | None, stop_below_mib: int = DEFAULT_STOP_BELOW_MIB) -> bool:
    """True only when measured availability clears the stop-below floor.

    Unknown availability (None) fails closed: no new work is admitted.
    """
    if type(stop_below_mib) is not int or stop_below_mib <= 0:
        raise ValueError("positive stop-below threshold required")
    return available_mib is not None and available_mib >= stop_below_mib


def classify_failure(kind: str) -> str:
    """'blocked' for auth/security/signing/budget failures, else 'repairable'."""
    if not isinstance(kind, str) or not kind.strip():
        raise ValueError("nonempty failure kind required")
    lowered = kind.strip().lower()
    if any(token in lowered for token in BLOCKED_SUBSTRINGS):
        return "blocked"
    return "repairable"


def should_retry(kind: str, attempts_used: int, max_attempts: int = DEFAULT_MAX_ATTEMPTS) -> bool:
    """True only for repairable failures under the attempt cap (max 3)."""
    if type(attempts_used) is not int or attempts_used < 0:
        raise ValueError("nonnegative attempts_used required")
    if type(max_attempts) is not int or not 1 <= max_attempts <= 3:
        raise ValueError("max_attempts must be between 1 and 3")
    return classify_failure(kind) == "repairable" and attempts_used < max_attempts


def parse_retry_after(value: object) -> float | None:
    """Parse a Retry-After value (delay seconds) into bounded seconds.

    Returns None for absent/invalid values. HTTP-date forms are not accepted:
    callers must pass delay seconds so waiting is explicit, never evaded.
    """
    if value is None:
        return None
    try:
        seconds = float(value)  # type: ignore[arg-type]
    except (TypeError, ValueError):
        return None
    if seconds != seconds or seconds < 0:  # NaN or negative
        return None
    return min(seconds, MAX_RETRY_AFTER_SECONDS)


class HeavyValidationLock:
    """One-heavy-validation semaphore backed by a file lock.

    Only one holder across threads and processes may run a heavyweight
    validation (full-workspace cargo test, browser session). Twenty logical
    workers share this single permit; the rest wait or do light work.
    """

    def __init__(self, path: str | os.PathLike = "state/heavy-validation.lock"):
        self.path = pathlib.Path(path)
        self._handle = None
        self._held = False

    @property
    def held(self) -> bool:
        return self._held

    def acquire(self, blocking: bool = False) -> bool:
        """Try to take the permit. Non-blocking by default; True when held."""
        if self._held:
            return True
        try:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            handle = open(self.path, "a+b")
        except OSError:
            return False
        try:
            import fcntl

            flags = fcntl.LOCK_EX if blocking else fcntl.LOCK_EX | fcntl.LOCK_NB
            try:
                fcntl.flock(handle.fileno(), flags)
            except OSError:
                handle.close()
                return False
        except ImportError:  # non-POSIX fallback: exclusive sentinel
            sentinel = self.path.with_suffix(".held")
            try:
                fd = os.open(str(sentinel), os.O_CREAT | os.O_EXCL | os.O_WRONLY)
                os.close(fd)
            except OSError:
                handle.close()
                return False
        self._handle = handle
        self._held = True
        return True

    def release(self) -> None:
        handle, self._handle = self._handle, None
        self._held = False
        if handle is not None:
            with contextlib.suppress(OSError):
                try:
                    import fcntl

                    fcntl.flock(handle.fileno(), fcntl.LOCK_UN)
                except ImportError:
                    with contextlib.suppress(OSError):
                        self.path.with_suffix(".held").unlink()
                handle.close()

    def __enter__(self) -> "HeavyValidationLock":
        if not self.acquire(blocking=False):
            raise RuntimeError("heavy validation already in progress")
        return self

    def __exit__(self, *exc: object) -> None:
        self.release()


@dataclass
class ProviderLimits:
    max_concurrent: int = 6
    max_requests_per_minute: int = 60
    total_budget_usd: float = 0.0
    retry_after_seconds: float = 0.0

    def __post_init__(self) -> None:
        if type(self.max_concurrent) is not int or self.max_concurrent <= 0:
            raise ValueError("positive max_concurrent required")
        if type(self.max_requests_per_minute) is not int or self.max_requests_per_minute <= 0:
            raise ValueError("positive max_requests_per_minute required")
        if not isinstance(self.total_budget_usd, (int, float)) or self.total_budget_usd < 0:
            raise ValueError("nonnegative total_budget_usd required")


class ProviderBudget:
    """Honest provider accounting: concurrency, rate, cost, Retry-After.

    No limit evasion: over-limit acquire() returns False, rate-limit waits
    carry the server's Retry-After deadline, and spend over budget is refused.
    """

    def __init__(self, limits: ProviderLimits | None = None):
        self.limits = limits or ProviderLimits()
        self.in_use = 0
        self.spent_usd = 0.0
        self._window: collections.deque[float] = collections.deque()
        self._retry_after_until: float = 0.0

    def _prune(self, now: float) -> None:
        while self._window and self._window[0] <= now - 60.0:
            self._window.popleft()

    def gated_until(self) -> float:
        return self._retry_after_until

    def try_acquire(self, *, now: float | None = None) -> bool:
        """Take one provider slot. False when gated, full, or rate-capped."""
        moment = time.monotonic() if now is None else now
        if moment < self._retry_after_until:
            return False
        if self.in_use >= self.limits.max_concurrent:
            return False
        self._prune(moment)
        if len(self._window) >= self.limits.max_requests_per_minute:
            return False
        self.in_use += 1
        self._window.append(moment)
        return True

    def release(self) -> None:
        self.in_use = max(0, self.in_use - 1)

    def note_rate_limited(self, retry_after: object, *, now: float | None = None) -> float:
        """Honor Retry-After: gate all acquisition until the delay elapses."""
        delay = parse_retry_after(retry_after)
        if delay is None:
            delay = self.limits.retry_after_seconds
        delay = max(0.0, min(delay, MAX_RETRY_AFTER_SECONDS))
        moment = time.monotonic() if now is None else now
        self._retry_after_until = max(self._retry_after_until, moment + delay)
        return self._retry_after_until

    def can_spend(self, amount_usd: float) -> bool:
        if not isinstance(amount_usd, (int, float)) or amount_usd < 0:
            raise ValueError("nonnegative spend amount required")
        return self.spent_usd + amount_usd <= self.limits.total_budget_usd

    def note_spend(self, amount_usd: float) -> None:
        if not self.can_spend(amount_usd):
            raise RuntimeError("provider cost budget exhausted")
        self.spent_usd += amount_usd


def cap_bytes(data: bytes | str, limit: int) -> bytes | str:
    """Truncate one output artifact to limit bytes, keeping the tail."""
    if type(limit) is not int or limit <= 0:
        raise ValueError("positive byte limit required")
    if isinstance(data, str):
        encoded = data.encode("utf-8", "replace")
        if len(encoded) <= limit:
            return data
        return "…[truncated]…" + encoded[-limit:].decode("utf-8", "replace")
    if isinstance(data, bytes):
        if len(data) <= limit:
            return data
        return b"...[truncated]..." + data[-limit:]
    raise ValueError("bytes or str required")


class BoundedLog:
    """Append-only bounded log: oldest entries drop once caps are hit."""

    def __init__(self, max_bytes: int = DEFAULT_MAX_BYTES,
                 max_entries: int = DEFAULT_MAX_ENTRIES):
        if type(max_bytes) is not int or max_bytes <= 0:
            raise ValueError("positive max_bytes required")
        if type(max_entries) is not int or max_entries <= 0:
            raise ValueError("positive max_entries required")
        self.max_bytes = max_bytes
        self.max_entries = max_entries
        self._entries: collections.deque[str] = collections.deque()
        self._bytes = 0
        self.dropped = 0

    def append(self, record: str) -> None:
        if not isinstance(record, str):
            raise ValueError("str record required")
        encoded_len = len(record.encode("utf-8", "replace"))
        if encoded_len > self.max_bytes:
            record = cap_bytes(record, self.max_bytes)
            if not isinstance(record, str):  # pragma: no cover - typing guard
                raise ValueError("str record required")
            encoded_len = len(record.encode("utf-8", "replace"))
        self._entries.append(record)
        self._bytes += encoded_len
        while len(self._entries) > self.max_entries or self._bytes > self.max_bytes:
            old = self._entries.popleft()
            self._bytes -= len(old.encode("utf-8", "replace"))
            self.dropped += 1

    def __len__(self) -> int:
        return len(self._entries)

    @property
    def bytes_held(self) -> int:
        return self._bytes

    def snapshot(self, *, limit: int = 64) -> list[str]:
        if type(limit) is not int or limit <= 0:
            raise ValueError("positive snapshot limit required")
        return list(self._entries)[-limit:]

    def clear(self) -> None:
        self._entries.clear()
        self._bytes = 0


@dataclass
class BudgetEnforcer:
    """Single owner of all budget state; shutdown() reclaims everything."""

    config: BudgetConfig = field(default_factory=BudgetConfig)
    lock: HeavyValidationLock = field(default_factory=HeavyValidationLock)
    provider: ProviderBudget = field(default_factory=ProviderBudget)
    log: BoundedLog = field(default_factory=BoundedLog)
    pending_integrations: int = 0
    max_pending: int = 20

    def check_memory_admission(self) -> tuple[bool, int | None]:
        """(admitted, available_mib). Admits only above the stop-below floor."""
        available = mem_available_mib()
        return admission_allowed(available, self.config.stop_below_available_mib), available

    def check_pending(self, extra: int = 1) -> bool:
        if type(extra) is not int or extra < 0:
            raise ValueError("nonnegative extra required")
        return self.pending_integrations + extra <= self.max_pending

    def note_pending(self, extra: int = 1) -> None:
        if not self.check_pending(extra):
            raise RuntimeError("pending integration bound reached")
        self.pending_integrations += extra

    def note_settled(self, count: int = 1) -> None:
        if type(count) is not int or count < 0:
            raise ValueError("nonnegative count required")
        self.pending_integrations = max(0, self.pending_integrations - count)

    def shutdown(self) -> None:
        """Release the heavy permit, provider slots, pending counts and logs."""
        self.lock.release()
        self.provider.in_use = 0
        self.pending_integrations = 0
        self.log.clear()


__all__ = [
    "BLOCKED_SUBSTRINGS",
    "BoundedLog",
    "BudgetConfig",
    "BudgetEnforcer",
    "HeavyValidationLock",
    "ProviderBudget",
    "ProviderLimits",
    "admission_allowed",
    "cap_bytes",
    "classify_failure",
    "load_controller_config",
    "mem_available_mib",
    "parse_retry_after",
    "process_tree_rss_mib",
    "should_retry",
]
