"""Tests the real scheduler with a deterministic external-harness test adapter.

These are NOT product, model-provider, sandbox or independent-release proofs.
"""
import asyncio
from dataclasses import replace
import unittest

from tools.completion_scheduler import (
    Candidate, Integration, RetryableFailure, Task, Verification,
    overlaps, run_rolling, validate_tasks,
)

HASH = "f" * 64
REV = "a" * 40
MERGED = "b" * 40


def task(tid, path=None, deps=()):
    return Task(tid, path or f"files/{tid}.rs", HASH, (f"{tid}-T01",), deps)


class Adapter:
    def __init__(self):
        self.started = []
        self.calls = {}
        self.proof_change = {}
        self.integration = True
        self.post_pass = True
        self.pre_integrate = []
        self.releases = {}
        self.started_twenty = asyncio.Event()
        self.started_next = asyncio.Event()
        self.active = 0
        self.peak = 0
        self.cancelled = 0
        self.retry = 0
        self.ambiguous_integration = False
        self.bad_path = False

    async def execute(self, t):
        self.started.append(t.id)
        self.calls[t.id] = self.calls.get(t.id, 0) + 1
        self.active += 1
        self.peak = max(self.peak, self.active)
        if len(self.started) == 20:
            self.started_twenty.set()
        if len(self.started) == 21:
            self.started_next.set()
        try:
            if t.id in self.releases:
                await self.releases[t.id].wait()
            if self.retry > 0:
                self.retry -= 1
                raise RetryableFailure("test-only repairable failure")
            return Candidate(t.id, REV, "implementer", ("other.rs" if self.bad_path else t.owned_path,))
        except asyncio.CancelledError:
            self.cancelled += 1
            raise
        finally:
            self.active -= 1

    def proof(self, t, rev):
        return Verification(t.id, rev, "independent-verifier", HASH, 1, t.test_obligations, True)

    async def verify(self, t, candidate):
        return replace(self.proof(t, candidate.revision), **self.proof_change)

    async def integrate(self, t, candidate):
        self.pre_integrate.append(t.id)
        if self.ambiguous_integration:
            raise RetryableFailure("unknown whether mainline changed")
        return Integration(candidate.revision, MERGED, self.integration)

    async def verify_integrated(self, t, candidate, revision):
        return replace(self.proof(t, revision), passed=self.post_pass)


class RollingTests(unittest.IsolatedAsyncioTestCase):
    async def test_refills_twenty_before_slowest_original_finishes(self):
        adapter = Adapter()
        jobs = [task(f"J{i:02}") for i in range(25)]
        adapter.releases = {t.id: asyncio.Event() for t in jobs}
        controller = asyncio.create_task(run_rolling(jobs, adapter))
        try:
            await asyncio.wait_for(adapter.started_twenty.wait(), 2)
            self.assertEqual(len(adapter.started), 20)
            adapter.releases["J00"].set()
            await asyncio.wait_for(adapter.started_next.wait(), 2)
            self.assertFalse(adapter.releases["J19"].is_set())
            self.assertLessEqual(adapter.peak, 20)
        finally:
            for event in adapter.releases.values():
                event.set()
            report = await asyncio.wait_for(controller, 2)
        self.assertTrue(report.complete)
        self.assertEqual(len(report.integrated_revisions), 25)

    async def test_integration_failure_does_not_unlock_dependency(self):
        adapter = Adapter()
        adapter.integration = False
        report = await run_rolling([task("A"), task("B", deps=("A",))], adapter)
        self.assertFalse(report.complete)
        self.assertEqual(report.statuses, {"A": "blocked", "B": "pending"})
        self.assertEqual(adapter.started, ["A"])
        self.assertFalse(report.integrated_revisions)

    async def test_post_merge_failure_is_not_accepted(self):
        adapter = Adapter()
        adapter.post_pass = False
        report = await run_rolling([task("A")], adapter)
        self.assertEqual(report.statuses["A"], "blocked")
        self.assertFalse(report.complete)

    async def test_zero_tests_rejected_before_merge(self):
        adapter = Adapter()
        adapter.proof_change = {"test_count": 0}
        report = await run_rolling([task("A")], adapter)
        self.assertFalse(report.complete)
        self.assertEqual(adapter.pre_integrate, [])

    async def test_changed_frozen_tests_rejected(self):
        adapter = Adapter()
        adapter.proof_change = {"frozen_tests_sha256": "e" * 64}
        report = await run_rolling([task("A")], adapter)
        self.assertFalse(report.complete)
        self.assertEqual(adapter.pre_integrate, [])

    async def test_wrong_candidate_revision_rejected(self):
        adapter = Adapter()
        adapter.proof_change = {"revision": MERGED}
        report = await run_rolling([task("A")], adapter)
        self.assertFalse(report.complete)

    async def test_self_verification_rejected(self):
        adapter = Adapter()
        adapter.proof_change = {"verifier": "implementer"}
        report = await run_rolling([task("A")], adapter)
        self.assertFalse(report.complete)

    async def test_omitted_obligation_rejected(self):
        adapter = Adapter()
        adapter.proof_change = {"test_obligations": ()}
        report = await run_rolling([task("A")], adapter)
        self.assertFalse(report.complete)

    async def test_out_of_scope_file_rejected(self):
        adapter = Adapter()
        adapter.bad_path = True
        report = await run_rolling([task("A")], adapter)
        self.assertFalse(report.complete)
        self.assertEqual(adapter.pre_integrate, [])

    async def test_same_path_is_serial_but_unrelated_work_proceeds(self):
        adapter = Adapter()
        jobs = [task("A", "same.rs"), task("B", "same.rs"), task("C")]
        report = await run_rolling(jobs, adapter, capacity=2)
        self.assertTrue(report.complete)
        self.assertEqual(adapter.started[:2], ["A", "C"])
        self.assertEqual(adapter.started[-1], "B")

    async def test_safe_retries_are_capped(self):
        adapter = Adapter()
        adapter.retry = 10
        report = await run_rolling([task("A")], adapter)
        self.assertEqual(report.attempts["A"], 3)
        self.assertEqual(report.statuses["A"], "blocked")

    async def test_safe_retry_can_succeed(self):
        adapter = Adapter()
        adapter.retry = 2
        report = await run_rolling([task("A")], adapter)
        self.assertTrue(report.complete)
        self.assertEqual(report.attempts["A"], 3)

    async def test_ambiguous_integration_is_never_blindly_retried(self):
        adapter = Adapter()
        adapter.ambiguous_integration = True
        report = await run_rolling([task("A")], adapter)
        self.assertFalse(report.complete)
        self.assertEqual(report.attempts["A"], 1)

    async def test_cancellation_joins_owned_adapter_calls(self):
        adapter = Adapter()
        jobs = [task(f"J{i:02}") for i in range(20)]
        adapter.releases = {t.id: asyncio.Event() for t in jobs}
        controller = asyncio.create_task(run_rolling(jobs, adapter))
        await asyncio.wait_for(adapter.started_twenty.wait(), 2)
        controller.cancel()
        with self.assertRaises(asyncio.CancelledError):
            await controller
        self.assertEqual(adapter.active, 0)
        self.assertEqual(adapter.cancelled, 20)

    async def test_worker_timeout_is_blocked(self):
        adapter = Adapter()
        adapter.releases["A"] = asyncio.Event()
        report = await run_rolling([task("A")], adapter, stage_timeout=0.01)
        self.assertFalse(report.complete)
        self.assertEqual(adapter.active, 0)

    async def test_invalid_capacity_rejected(self):
        with self.assertRaises(ValueError):
            await run_rolling([task("A")], Adapter(), capacity=21)
        with self.assertRaises(ValueError):
            await run_rolling([task("A")], Adapter(), capacity=True)


class GraphTests(unittest.TestCase):
    def test_empty_graph_is_not_completion(self):
        with self.assertRaises(ValueError):
            validate_tasks([])

    def test_missing_dependency_and_cycle_fail(self):
        with self.assertRaises(ValueError):
            validate_tasks([task("A", deps=("missing",))])
        with self.assertRaises(ValueError):
            validate_tasks([task("A", deps=("B",)), task("B", deps=("A",))])

    def test_overlap_is_path_bounded(self):
        self.assertTrue(overlaps("crates/foo", "crates/foo/bar.rs"))
        self.assertFalse(overlaps("crates/foo", "crates/foobar"))
        self.assertFalse(overlaps("fork:opentui/a.rs", "a.rs"))

    def test_unsafe_ownership_rejected(self):
        for path in ("../secret", "/tmp/file", "crates/*", "a/../b", "a//b"):
            with self.assertRaises(ValueError):
                validate_tasks([task("A", path)])


if __name__ == "__main__":
    unittest.main()
