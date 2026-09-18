"""Native-harness adapter scaffolding for COORD-001.

Binds the real host harness to ``tools.completion_scheduler.TrustedAdapter``
with explicit capabilities, role separation and bounded resources.

What this module does:
  - mints one-file :class:`Capability` grants per scheduler call;
  - enforces implementer != verifier/integrator and one-file write scope;
  - delegates to a host-supplied :class:`NativeHarness` (real native API);
  - tracks owned inner tasks so :meth:`HarnessAdapter.shutdown` cancels/joins;
  - keeps per-task logs byte-bounded and redacted.

What it never does:
  - spawns CLI/OS processes (no subprocess/pty/shell);
  - invents credentials, provider budget or concurrency (all explicit);
  - fakes execution with echo/sleep/PASS text (fail closed instead).
"""
from __future__ import annotations

import asyncio
import re
from collections import deque
from dataclasses import dataclass, field
from typing import Awaitable, Callable, Protocol

from tools.completion_scheduler import (
    Candidate,
    Integration,
    Rejected,
    RetryableFailure,
    Task,
    Verification,
    normalized_path,
    validate_candidate,
    validate_proof,
)

IMPLEMENT = "implement"
VERIFY = "verify"
INTEGRATE = "integrate"
ROLES = (IMPLEMENT, VERIFY, INTEGRATE)

MAX_LOG_BYTES = 65536
MAX_INFLIGHT = 20

PROTECTED_PREFIXES = ("tests/", "state/", "config/", "sources/")
PROTECTED_EXACT = frozenset({
    "docs/SECURITY.md",
    "tools/validate_repository.py",
    "tools/lane_gate.py",
    "tools/completion_scheduler.py",
    "tools/completion_plan.py",
    "PLAN.md",
})


class AdapterUnavailable(RetryableFailure):
    """Native harness unavailable or bounded; never a CLI-fallback cue."""


class AuthorityError(Rejected):
    """Role, identity or write-scope violation; fails closed, no retry."""


def _is_protected(path: str) -> bool:
    return path in PROTECTED_EXACT or path.startswith(PROTECTED_PREFIXES)


def assert_role_separation(implementer: str, verifiers: tuple[str, ...], integrator: str) -> None:
    """Real identity check: verifier set must exclude the implementer."""
    if not implementer or not isinstance(implementer, str):
        raise AuthorityError("implementer identity required")
    if not verifiers or not all(isinstance(v, str) and v for v in verifiers):
        raise AuthorityError("nonempty verifier identities required")
    if not integrator or not isinstance(integrator, str):
        raise AuthorityError("integrator identity required")
    if implementer in verifiers:
        raise AuthorityError("independent verifier required")
    if integrator == implementer:
        raise AuthorityError("integrator must differ from implementer")


@dataclass(frozen=True)
class Capability:
    """Explicit one-file grant for a single task and role."""

    task_id: str
    owned_path: str
    role: str
    owner: str
    log_byte_budget: int = MAX_LOG_BYTES

    def allows_write(self, path: str) -> bool:
        """True only for the exact owned file; implementers get no protected writes."""
        try:
            want = normalized_path(path)
        except ValueError:
            return False
        if want != self.owned_path:
            return False
        if self.role == IMPLEMENT and _is_protected(want):
            return False
        return True


def mint_capability(task: Task, role: str, owner: str, *, log_byte_budget: int = MAX_LOG_BYTES) -> Capability:
    """Build a validated one-file grant; rejects protected implementer scope."""
    if not isinstance(task, Task) or not task.id:
        raise AuthorityError("task identity required")
    owned = normalized_path(task.owned_path)
    if role not in ROLES or not isinstance(owner, str) or not owner:
        raise AuthorityError("role and owner identity required")
    if type(log_byte_budget) is not int or not 1 <= log_byte_budget <= 1 << 20:
        raise ValueError("log byte budget must be between 1 and 1048576")
    if role == IMPLEMENT and _is_protected(owned):
        raise AuthorityError("implementer grant excludes protected paths")
    return Capability(task.id, owned, role, owner, log_byte_budget)


_SECRET = re.compile(r"(?im)^(?P<k>[^\n:=]*(?:api[_-]?key|bearer|secret|password|token)[^\n:=]*)\s*[:=]\s*[^\n]+")
_SK = re.compile(r"sk-[A-Za-z0-9]{6,}")


def redact(text: str) -> str:
    """Mask credential-shaped substrings; never logs secrets verbatim."""
    if not isinstance(text, str):
        return ""
    text = _SECRET.sub(lambda m: m.group("k").strip() + "=***", text)
    return _SK.sub("sk-***", text)


class BoundedLog:
    """Byte-bounded FIFO log; oldest entries evicted first."""

    def __init__(self, max_bytes: int = MAX_LOG_BYTES) -> None:
        if type(max_bytes) is not int or not 1 <= max_bytes <= 1 << 20:
            raise ValueError("log byte budget must be between 1 and 1048576")
        self._max = max_bytes
        self._parts: deque[str] = deque()
        self._bytes = 0
        self.dropped = 0

    @property
    def byte_size(self) -> int:
        return self._bytes

    def __len__(self) -> int:
        return len(self._parts)

    def append(self, text: str) -> None:
        line = redact(text) if isinstance(text, str) else ""
        if not line:
            return
        if not line.endswith("\n"):
            line += "\n"
        size = len(line.encode("utf-8"))
        if size > self._max:
            line = line.encode("utf-8")[-self._max:].decode("utf-8", "ignore")
            size = len(line.encode("utf-8"))
        self._parts.append(line)
        self._bytes += size
        while self._bytes > self._max and len(self._parts) > 1:
            old = self._parts.popleft()
            self._bytes -= len(old.encode("utf-8"))
            self.dropped += 1

    def tail(self, lines: int = 12) -> str:
        if type(lines) is not int or lines < 1:
            raise ValueError("positive line count required")
        return "\n".join("".join(self._parts).splitlines()[-lines:])


class NativeHarness(Protocol):
    """Real host harness API; supplied by the trusted host, never invented here."""

    async def execute(self, task: Task, cap: Capability) -> Candidate: ...
    async def verify(self, task: Task, candidate: Candidate, cap: Capability) -> Verification: ...
    async def integrate(self, task: Task, candidate: Candidate, cap: Capability) -> Integration: ...
    async def verify_integrated(self, task: Task, candidate: Candidate, revision: str, cap: Capability) -> Verification: ...


class UnavailableHarness:
    """Honest closed backend: reports missing native concurrency, never fakes it."""

    async def _fail(self, stage: str) -> None:
        raise AdapterUnavailable(f"native harness unavailable for {stage}; refusing CLI fallback")

    async def execute(self, task: Task, cap: Capability) -> Candidate:
        await self._fail("execute")

    async def verify(self, task: Task, candidate: Candidate, cap: Capability) -> Verification:
        await self._fail("verify")

    async def integrate(self, task: Task, candidate: Candidate, cap: Capability) -> Integration:
        await self._fail("integrate")

    async def verify_integrated(self, task: Task, candidate: Candidate, revision: str, cap: Capability) -> Verification:
        await self._fail("verify_integrated")


class HarnessAdapter:
    """TrustedAdapter binding: capability minting, role checks, bounded logs, join."""

    def __init__(
        self,
        *,
        implementer: str,
        verifiers: tuple[str, ...],
        integrator: str,
        backend: NativeHarness,
        max_inflight: int,
        log_byte_budget: int = MAX_LOG_BYTES,
    ) -> None:
        assert_role_separation(implementer, verifiers, integrator)
        if backend is None or not all(hasattr(backend, m) for m in ("execute", "verify", "integrate", "verify_integrated")):
            raise AuthorityError("real native harness backend required")
        if type(max_inflight) is not int or not 1 <= max_inflight <= MAX_INFLIGHT:
            raise ValueError("max_inflight must be between 1 and 20")
        if type(log_byte_budget) is not int or not 1 <= log_byte_budget <= 1 << 20:
            raise ValueError("log byte budget must be between 1 and 1048576")
        self._implementer = implementer
        self._verifiers = tuple(verifiers)
        self._integrator = integrator
        self._backend = backend
        self._max_inflight = max_inflight
        self._log_budget = log_byte_budget
        self._owned: set[asyncio.Task] = set()
        self._logs: dict[str, BoundedLog] = {}
        self.cancelled = 0

    @property
    def inflight(self) -> int:
        return len(self._owned)

    def _log(self, task_id: str) -> BoundedLog:
        log = self._logs.get(task_id)
        if log is None:
            log = BoundedLog(self._log_budget)
            self._logs[task_id] = log
        return log

    def tail(self, task_id: str, lines: int = 12) -> str:
        log = self._logs.get(task_id)
        return log.tail(lines) if log is not None else ""

    async def _join(self, factory: Callable[[], Awaitable]) -> object:
        if len(self._owned) >= self._max_inflight:
            raise AdapterUnavailable("adapter at bounded inflight cap; backpressure, no CLI fallback")
        inner = asyncio.create_task(factory())
        self._owned.add(inner)
        try:
            return await inner
        except asyncio.CancelledError:
            self.cancelled += 1
            if not inner.done():
                inner.cancel()
            await asyncio.gather(inner, return_exceptions=True)
            raise
        finally:
            self._owned.discard(inner)

    def _note(self, task_id: str, stage: str, detail: str) -> None:
        self._log(task_id).append(f"{stage} {detail}")

    async def execute(self, task: Task) -> Candidate:
        cap = mint_capability(task, IMPLEMENT, self._implementer, log_byte_budget=self._log_budget)
        result = await self._join(lambda: self._backend.execute(task, cap))
        if not isinstance(result, Candidate):
            raise AuthorityError("backend must return a Candidate")
        validate_candidate(task, result)
        if not cap.allows_write(result.changed_paths[0]):
            raise AuthorityError("candidate outside one-file grant")
        self._note(task.id, "execute", f"worker={result.worker} rev={result.revision}")
        return result

    async def verify(self, task: Task, candidate: Candidate) -> Verification:
        cap = mint_capability(task, VERIFY, self._verifiers[0], log_byte_budget=self._log_budget)
        result = await self._join(lambda: self._backend.verify(task, candidate, cap))
        if not isinstance(result, Verification):
            raise AuthorityError("backend must return a Verification")
        if result.verifier not in self._verifiers or result.verifier == candidate.worker:
            raise AuthorityError("independent verifier required")
        validate_proof(task, candidate, result, candidate.revision)
        self._note(task.id, "verify", f"verifier={result.verifier} count={result.test_count} passed={result.passed}")
        return result

    async def integrate(self, task: Task, candidate: Candidate) -> Integration:
        cap = mint_capability(task, INTEGRATE, self._integrator, log_byte_budget=self._log_budget)
        result = await self._join(lambda: self._backend.integrate(task, candidate, cap))
        if (
            not isinstance(result, Integration)
            or result.integrated is not True
            or result.candidate_revision != candidate.revision
        ):
            raise Rejected("candidate was not integrated")
        self._note(task.id, "integrate", f"rev={result.integrated_revision}")
        return result

    async def verify_integrated(self, task: Task, candidate: Candidate, revision: str) -> Verification:
        cap = mint_capability(task, VERIFY, self._verifiers[0], log_byte_budget=self._log_budget)
        result = await self._join(lambda: self._backend.verify_integrated(task, candidate, revision, cap))
        if not isinstance(result, Verification):
            raise AuthorityError("backend must return a Verification")
        if result.verifier not in self._verifiers or result.verifier == candidate.worker:
            raise AuthorityError("independent verifier required")
        validate_proof(task, candidate, result, revision)
        self._note(task.id, "verify_integrated", f"verifier={result.verifier} rev={revision} passed={result.passed}")
        return result

    async def shutdown(self) -> None:
        """Cancel owned inner work and join it; never orphans delegated calls."""
        owned = list(self._owned)
        for pending in owned:
            pending.cancel()
        if owned:
            await asyncio.gather(*owned, return_exceptions=True)
            self.cancelled += sum(1 for t in owned if t.cancelled())
        self._owned.clear()

    aclose = shutdown
