#!/usr/bin/env python3
"""Rolling native-harness scheduling primitive, not a model/OS adapter.

Only trusted adapters may implement execution, independent verification and VCS
integration. This module launches no CLI processes and writes no accepted flags.
Tasks represent frozen, one-file child lanes, NOT entire multi-file parents.
Durable leases, OS isolation, resource measurement and provider credentials must
be supplied by the host; COORD tasks keep those obligations explicitly open.
"""
from __future__ import annotations

import asyncio
import math
import pathlib
import re
from collections import deque
from dataclasses import dataclass
from typing import Protocol

REV = re.compile(r"[0-9a-f]{40}\Z")
HASH = re.compile(r"[0-9a-f]{64}\Z")


class RetryableFailure(RuntimeError):
    """Adapter-classified repairable failure; never use for authority failures."""


class Rejected(RuntimeError):
    pass


@dataclass(frozen=True)
class Task:
    id: str
    owned_path: str
    frozen_tests_sha256: str
    test_obligations: tuple[str, ...]
    dependencies: tuple[str, ...] = ()


@dataclass(frozen=True)
class Candidate:
    task_id: str
    revision: str
    worker: str
    changed_paths: tuple[str, ...]


@dataclass(frozen=True)
class Verification:
    task_id: str
    revision: str
    verifier: str
    frozen_tests_sha256: str
    test_count: int
    test_obligations: tuple[str, ...]
    passed: bool


@dataclass(frozen=True)
class Integration:
    candidate_revision: str
    integrated_revision: str
    integrated: bool


@dataclass
class Report:
    statuses: dict[str, str]
    attempts: dict[str, int]
    errors: dict[str, str]
    integrated_revisions: dict[str, str]

    @property
    def complete(self) -> bool:
        return bool(self.statuses) and all(s == "accepted" for s in self.statuses.values())


class TrustedAdapter(Protocol):
    async def execute(self, task: Task) -> Candidate:
        """Use actual native subagent API with a frozen one-file OS grant."""
        ...

    async def verify(self, task: Task, candidate: Candidate) -> Verification:
        """Independent verifier executes immutable tests outside worker authority."""
        ...

    async def integrate(self, task: Task, candidate: Candidate) -> Integration:
        """Serialize VCS change on current mainline; prove actual ancestry/change."""
        ...

    async def verify_integrated(self, task: Task, candidate: Candidate, revision: str) -> Verification:
        """Rerun frozen tests/regressions against the exact integrated revision."""
        ...


def normalized_path(value: str) -> str:
    if not isinstance(value, str) or not value or "\\" in value or value.endswith("/"):
        raise ValueError("one explicit relative owned file is required")
    if pathlib.PurePosixPath(value).is_absolute() or any(p in {"", ".", ".."} for p in value.split("/")) or any(c in value for c in "*?["):
        raise ValueError("wildcard, traversal or absolute ownership is forbidden")
    return value


def overlaps(a: str, b: str) -> bool:
    return a == b or a.startswith(b + "/") or b.startswith(a + "/")


def validate_tasks(tasks: list[Task]) -> dict[str, Task]:
    if not tasks or len(tasks) > 10000:
        raise ValueError("nonempty bounded task list required")
    by_id = {task.id: task for task in tasks}
    if len(by_id) != len(tasks):
        raise ValueError("duplicate task")
    remaining = {}
    for task in tasks:
        if not task.id or not HASH.fullmatch(task.frozen_tests_sha256):
            raise ValueError("task requires identity and trusted frozen test hash")
        normalized_path(task.owned_path)
        if not task.test_obligations or len(set(task.test_obligations)) != len(task.test_obligations):
            raise ValueError("nonempty unique test obligations required")
        if not set(task.dependencies) <= by_id.keys():
            raise ValueError("dependency absent from admitted task graph")
        remaining[task.id] = set(task.dependencies)
    accepted = set()
    while remaining:
        ready = {tid for tid, deps in remaining.items() if deps <= accepted}
        if not ready:
            raise ValueError("task dependency cycle")
        accepted.update(ready)
        for tid in ready:
            remaining.pop(tid)
    return by_id


def validate_candidate(task: Task, candidate: Candidate) -> None:
    if not isinstance(candidate, Candidate) or candidate.task_id != task.id or not REV.fullmatch(candidate.revision) or not candidate.worker:
        raise Rejected("candidate identity/revision missing")
    if candidate.changed_paths != (task.owned_path,):
        raise Rejected("candidate does not match one-file grant")


def validate_proof(task: Task, candidate: Candidate, proof: Verification, revision: str) -> None:
    if not isinstance(proof, Verification) or proof.passed is not True:
        raise Rejected("verification failed")
    if proof.task_id != task.id or proof.revision != revision:
        raise Rejected("verification is for a different task/revision")
    if proof.frozen_tests_sha256 != task.frozen_tests_sha256:
        raise Rejected("frozen tests changed")
    if not proof.verifier or proof.verifier == candidate.worker:
        raise Rejected("independent verifier required")
    if type(proof.test_count) is not int or proof.test_count < len(task.test_obligations):
        raise Rejected("zero or insufficient test count")
    if not set(task.test_obligations) <= set(proof.test_obligations):
        raise Rejected("required test obligations not executed")


async def run_rolling(tasks: list[Task], adapter: TrustedAdapter, *, capacity: int = 20,
                      max_unverified: int = 20, max_attempts: int = 3,
                      stage_timeout: float = 3600) -> Report:
    """Refill on individual completion; retain locks until post-merge proof.

    There is one verification/integration pipeline. Unverified capacity is
    reserved before execution so simultaneous completions cannot overflow the
    candidate queue. Backpressure may lower occupancy; it never disables gates.
    An adapter must honor cancellation and stop/join owned remote/native work.
    """
    by_id = validate_tasks(tasks)
    if type(capacity) is not int or not 1 <= capacity <= 20:
        raise ValueError("capacity must be between 1 and 20")
    if type(max_unverified) is not int or not 1 <= max_unverified <= 20:
        raise ValueError("unverified candidate cap must be between 1 and 20")
    if type(max_attempts) is not int or not 1 <= max_attempts <= 3:
        raise ValueError("attempt cap must be between 1 and 3")
    if not math.isfinite(stage_timeout) or stage_timeout <= 0:
        raise ValueError("finite positive timeout required")
    report = Report({t.id: "pending" for t in tasks}, {t.id: 0 for t in tasks}, {}, {})
    running: dict[asyncio.Task, str] = {}
    candidates: deque[tuple[Task, Candidate]] = deque()
    checking: asyncio.Task | None = None
    checking_id: str | None = None

    async def execute(task: Task) -> Candidate:
        async with asyncio.timeout(stage_timeout):
            candidate = await adapter.execute(task)
            validate_candidate(task, candidate)
            return candidate

    async def finalize(task: Task, candidate: Candidate) -> str:
        async with asyncio.timeout(stage_timeout):
            proof = await adapter.verify(task, candidate)
            validate_proof(task, candidate, proof, candidate.revision)
            try:
                integrated = await adapter.integrate(task, candidate)
                if not isinstance(integrated, Integration) or integrated.integrated is not True or integrated.candidate_revision != candidate.revision or not REV.fullmatch(integrated.integrated_revision):
                    raise Rejected("candidate was not integrated")
                proof = await adapter.verify_integrated(task, candidate, integrated.integrated_revision)
                validate_proof(task, candidate, proof, integrated.integrated_revision)
                return integrated.integrated_revision
            except RetryableFailure as exc:
                # Integration may already have changed mainline. Reconcile it;
                # never blindly run the implementation again after ambiguity.
                raise Rejected("integration outcome requires reconciliation") from exc

    def fail(tid: str, exc: Exception) -> None:
        # Detailed potentially sensitive adapter logs stay with the trusted host.
        report.errors[tid] = type(exc).__name__
        report.statuses[tid] = "pending" if isinstance(exc, RetryableFailure) and report.attempts[tid] < max_attempts else "blocked"

    try:
        while True:
            if checking is None and candidates:
                task, candidate = candidates.popleft()
                checking_id = task.id
                report.statuses[task.id] = "verifying"
                checking = asyncio.create_task(finalize(task, candidate))
            for task in tasks:
                if len(running) >= capacity or len(running) + len(candidates) >= max_unverified:
                    break
                if report.statuses[task.id] != "pending":
                    continue
                if any(report.statuses[dep] != "accepted" for dep in task.dependencies):
                    continue
                held = [by_id[tid].owned_path for tid, status in report.statuses.items() if status in {"running", "queued", "verifying"}]
                if any(overlaps(task.owned_path, path) for path in held):
                    continue
                report.statuses[task.id] = "running"
                report.attempts[task.id] += 1
                running[asyncio.create_task(execute(task))] = task.id
            active = set(running)
            if checking is not None:
                active.add(checking)
            if not active:
                return report
            done, _ = await asyncio.wait(active, return_when=asyncio.FIRST_COMPLETED)
            for future in done:
                if future is checking:
                    tid = checking_id
                    try:
                        report.integrated_revisions[tid] = future.result()
                        report.statuses[tid] = "accepted"
                        report.errors.pop(tid, None)
                    except Exception as exc:
                        fail(tid, exc)
                    checking, checking_id = None, None
                else:
                    tid = running.pop(future)
                    try:
                        candidates.append((by_id[tid], future.result()))
                        report.statuses[tid] = "queued"
                    except Exception as exc:
                        fail(tid, exc)
    finally:
        active = list(running)
        if checking is not None:
            active.append(checking)
        for future in active:
            future.cancel()
        if active:
            await asyncio.gather(*active, return_exceptions=True)
