"""Independent V2 controller backpressure contracts.

These tests intentionally cover the hard candidate high-water requirement and
owned-task cancellation.  They are independent of the existing frozen V1/V2
controller tests.
"""
from __future__ import annotations

import asyncio
import pathlib
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tools.completion_scheduler import (  # noqa: E402
    Candidate,
    Integration,
    Task,
    Verification,
    run_rolling,
)


HASH = "a" * 64


def package(number: int) -> Task:
    return Task(
        f"BP-{number:03d}",
        f"tests/bootstrap/backpressure_{number}.py",
        HASH,
        (f"BP-{number:03d}-T01",),
    )


class BackpressureTests(unittest.IsolatedAsyncioTestCase):
    async def test_candidate_high_water_is_hard_capped_at_four(self) -> None:
        class Adapter:
            async def execute(self, task: Task) -> Candidate:
                return Candidate(task.id, "1" * 40, "worker", (task.owned_path,))

            async def verify(self, task: Task, candidate: Candidate) -> Verification:
                return Verification(
                    task.id, candidate.revision, "verifier", HASH, 1,
                    task.test_obligations, True,
                )

            async def integrate(self, task: Task, candidate: Candidate) -> Integration:
                return Integration(candidate.revision, "2" * 40, True)

            async def verify_integrated(
                self, task: Task, candidate: Candidate, revision: str,
            ) -> Verification:
                return Verification(
                    task.id, revision, "post-verifier", HASH, 1,
                    task.test_obligations, True,
                )

        tasks = [package(number) for number in range(6)]
        for requested in (5, 20):
            with self.subTest(requested=requested):
                with self.assertRaises(ValueError):
                    await run_rolling(
                        tasks, Adapter(), capacity=6,
                        max_unverified=requested, max_preverify=4,
                        stage_timeout=1,
                    )

    async def test_no_new_execute_starts_while_four_candidates_are_pending(self) -> None:
        class Adapter:
            def __init__(self) -> None:
                self.execute_started = 0
                self.verify_started = 0
                self.four_verifies_started = asyncio.Event()
                self.release_verification = asyncio.Event()

            async def execute(self, task: Task) -> Candidate:
                self.execute_started += 1
                await asyncio.sleep(0)
                return Candidate(task.id, f"{int(task.id[-3:]) + 1:040x}", "worker", (task.owned_path,))

            async def verify(self, task: Task, candidate: Candidate) -> Verification:
                self.verify_started += 1
                if self.verify_started == 4:
                    self.four_verifies_started.set()
                await self.release_verification.wait()
                return Verification(
                    task.id, candidate.revision, "verifier", HASH, 1,
                    task.test_obligations, True,
                )

            async def integrate(self, task: Task, candidate: Candidate) -> Integration:
                return Integration(candidate.revision, "2" * 40, True)

            async def verify_integrated(
                self, task: Task, candidate: Candidate, revision: str,
            ) -> Verification:
                return Verification(
                    task.id, revision, "post-verifier", HASH, 1,
                    task.test_obligations, True,
                )

        adapter = Adapter()
        controller = asyncio.create_task(
            run_rolling(
                [package(number) for number in range(6)], adapter,
                capacity=6, max_unverified=4, max_preverify=4,
                stage_timeout=1,
            )
        )
        try:
            await asyncio.wait_for(adapter.four_verifies_started.wait(), timeout=1)
            self.assertLessEqual(adapter.execute_started, 4)
        finally:
            adapter.release_verification.set()
            await asyncio.wait_for(controller, timeout=1)

    async def test_cancellation_joins_waiting_and_executing_workers(self) -> None:
        started = 0
        finished = 0
        release = asyncio.Event()

        class Adapter:
            async def execute(self, task: Task) -> Candidate:
                nonlocal started, finished
                started += 1
                try:
                    await release.wait()
                    return Candidate(task.id, "1" * 40, "worker", (task.owned_path,))
                finally:
                    finished += 1

            async def verify(self, task: Task, candidate: Candidate) -> Verification:
                return Verification(task.id, candidate.revision, "verifier", HASH, 1, task.test_obligations, True)

            async def integrate(self, task: Task, candidate: Candidate) -> Integration:
                return Integration(candidate.revision, "2" * 40, True)

            async def verify_integrated(self, task: Task, candidate: Candidate, revision: str) -> Verification:
                return Verification(task.id, revision, "post-verifier", HASH, 1, task.test_obligations, True)

        controller = asyncio.create_task(
            run_rolling(
                [package(number) for number in range(6)], Adapter(),
                capacity=6, max_unverified=4, max_preverify=4,
                stage_timeout=10,
            )
        )
        while started == 0:
            await asyncio.sleep(0)
        controller.cancel()
        with self.assertRaises(asyncio.CancelledError):
            await controller
        self.assertEqual(finished, started)
        self.assertFalse(
            [task for task in asyncio.all_tasks() if task is not asyncio.current_task()]
        )


if __name__ == "__main__":
    unittest.main()
