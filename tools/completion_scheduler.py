#!/usr/bin/env python3
"""Rolling native-harness scheduling primitive, not a model/OS adapter.

Only trusted adapters may implement execution, independent verification and VCS
integration. This module launches no CLI processes and writes no accepted flags.
Tasks represent bounded observable work packages. A package may own multiple
explicit paths when one product behavior genuinely spans them.
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
    owned_paths: tuple[str, ...] = ()


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
        """Use the native subagent API with the package's explicit path grant."""
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
        raise ValueError("an explicit relative owned path is required")
    if pathlib.PurePosixPath(value).is_absolute() or any(p in {"", ".", ".."} for p in value.split("/")) or any(c in value for c in "*?["):
        raise ValueError("wildcard, traversal or absolute ownership is forbidden")
    return value


def overlaps(a: str, b: str) -> bool:
    return a == b or a.startswith(b + "/") or b.startswith(a + "/")


def task_paths(task: Task) -> tuple[str, ...]:
    """Return the exact bounded grant while preserving the V1 constructor API."""
    paths = (task.owned_path, *task.owned_paths)
    if len(set(paths)) != len(paths):
        raise ValueError("duplicate owned path in package grant")
    for path in paths:
        normalized_path(path)
    for index, path in enumerate(paths):
        if any(overlaps(path, other) for other in paths[index + 1:]):
            raise ValueError("overlapping owned paths inside one package are redundant")
    return paths


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
        task_paths(task)
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
    grant = set(task_paths(task))
    changed = tuple(candidate.changed_paths)
    if not changed or len(set(changed)) != len(changed):
        raise Rejected("candidate changed path set must be nonempty and unique")
    try:
        normalized = tuple(normalized_path(path) for path in changed)
    except ValueError as exc:
        raise Rejected("candidate contains an unsafe changed path") from exc
    if any(path not in grant for path in normalized):
        raise Rejected("candidate changed path is outside explicit package grant")


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
                      max_unverified: int = 4, max_preverify: int = 4,
                      max_attempts: int = 3, stage_timeout: float = 3600) -> Report:
    """Run packages with parallel preverification and one integration writer.

    Implementations may run concurrently on non-overlapping grants. At most
    ``max_unverified`` candidates may enter the candidate pipeline at once;
    additional implementers block before publishing their candidate. Independent
    preverification runs in parallel, while VCS integration plus post-integration
    verification remains serialized through exactly one integration future.
    """
    by_id = validate_tasks(tasks)
    if type(capacity) is not int or not 1 <= capacity <= 20:
        raise ValueError("capacity must be between 1 and 20")
    if type(max_unverified) is not int or not 1 <= max_unverified <= 20:
        raise ValueError("unverified candidate cap must be between 1 and 20")
    if type(max_preverify) is not int or not 1 <= max_preverify <= 20:
        raise ValueError("preverification capacity must be between 1 and 20")
    if type(max_attempts) is not int or not 1 <= max_attempts <= 3:
        raise ValueError("attempt cap must be between 1 and 3")
    if not math.isfinite(stage_timeout) or stage_timeout <= 0:
        raise ValueError("finite positive timeout required")

    report = Report({t.id: "pending" for t in tasks}, {t.id: 0 for t in tasks}, {}, {})
    running: dict[asyncio.Task, str] = {}
    candidates: deque[tuple[Task, Candidate]] = deque()
    verifying: dict[asyncio.Task, tuple[Task, Candidate]] = {}
    integration_ready: deque[tuple[Task, Candidate]] = deque()
    integrating: asyncio.Task | None = None
    integrating_item: tuple[Task, Candidate] | None = None
    candidate_slots = asyncio.Semaphore(max_unverified)

    async def execute(task: Task) -> Candidate:
        acquired = False
        try:
            candidate = await asyncio.wait_for(adapter.execute(task), timeout=stage_timeout)
            validate_candidate(task, candidate)
            await asyncio.wait_for(candidate_slots.acquire(), timeout=stage_timeout)
            acquired = True
            return candidate
        except BaseException:
            if acquired:
                candidate_slots.release()
            raise

    async def preverify(task: Task, candidate: Candidate) -> None:
        proof = await asyncio.wait_for(adapter.verify(task, candidate), timeout=stage_timeout)
        validate_proof(task, candidate, proof, candidate.revision)

    async def integrate_verified(task: Task, candidate: Candidate) -> str:
        try:
            integrated = await asyncio.wait_for(adapter.integrate(task, candidate), timeout=stage_timeout)
            if (
                not isinstance(integrated, Integration)
                or integrated.integrated is not True
                or integrated.candidate_revision != candidate.revision
                or not REV.fullmatch(integrated.integrated_revision)
            ):
                raise Rejected("candidate was not integrated")
            proof = await asyncio.wait_for(
                adapter.verify_integrated(task, candidate, integrated.integrated_revision),
                timeout=stage_timeout,
            )
            validate_proof(task, candidate, proof, integrated.integrated_revision)
            return integrated.integrated_revision
        except RetryableFailure as exc:
            raise Rejected("integration outcome requires reconciliation") from exc

    def fail(tid: str, exc: Exception) -> None:
        report.errors[tid] = type(exc).__name__
        report.statuses[tid] = (
            "pending"
            if isinstance(exc, RetryableFailure) and report.attempts[tid] < max_attempts
            else "blocked"
        )

    def held_paths() -> list[str]:
        return [
            path
            for tid, status in report.statuses.items()
            if status in {"running", "queued", "verifying", "integration-ready", "integrating"}
            for path in task_paths(by_id[tid])
        ]

    try:
        while True:
            while candidates and len(verifying) < max_preverify:
                task, candidate = candidates.popleft()
                report.statuses[task.id] = "verifying"
                verifying[asyncio.create_task(preverify(task, candidate))] = (task, candidate)

            if integrating is None and integration_ready:
                task, candidate = integration_ready.popleft()
                integrating_item = (task, candidate)
                report.statuses[task.id] = "integrating"
                integrating = asyncio.create_task(integrate_verified(task, candidate))

            for task in tasks:
                if len(running) >= capacity:
                    break
                if report.statuses[task.id] != "pending":
                    continue
                if any(report.statuses[dep] != "accepted" for dep in task.dependencies):
                    continue
                held = held_paths()
                if any(overlaps(path, other) for path in task_paths(task) for other in held):
                    continue
                report.statuses[task.id] = "running"
                report.attempts[task.id] += 1
                running[asyncio.create_task(execute(task))] = task.id

            active = set(running)
            active.update(verifying)
            if integrating is not None:
                active.add(integrating)
            if not active:
                return report

            done, _ = await asyncio.wait(active, return_when=asyncio.FIRST_COMPLETED)
            for future in done:
                if future is integrating:
                    assert integrating_item is not None
                    task, _candidate = integrating_item
                    tid = task.id
                    try:
                        report.integrated_revisions[tid] = future.result()
                        report.statuses[tid] = "accepted"
                        report.errors.pop(tid, None)
                    except Exception as exc:
                        fail(tid, exc)
                    finally:
                        candidate_slots.release()
                    integrating = None
                    integrating_item = None
                    continue

                if future in verifying:
                    task, candidate = verifying.pop(future)
                    tid = task.id
                    try:
                        future.result()
                        report.statuses[tid] = "integration-ready"
                        integration_ready.append((task, candidate))
                    except Exception as exc:
                        candidate_slots.release()
                        fail(tid, exc)
                    continue

                tid = running.pop(future)
                try:
                    candidates.append((by_id[tid], future.result()))
                    report.statuses[tid] = "queued"
                except Exception as exc:
                    fail(tid, exc)
    finally:
        active = list(running)
        active.extend(verifying)
        if integrating is not None:
            active.append(integrating)
        for future in active:
            future.cancel()
        if active:
            await asyncio.gather(*active, return_exceptions=True)
