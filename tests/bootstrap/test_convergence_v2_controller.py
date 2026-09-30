"""Focused Convergence V2 controller invariants.

These tests do not alter the legacy V1 bootstrap contract. They cover only the
new package-grant, acceptance, backpressure and integration semantics used by
main-v2.
"""
from __future__ import annotations

import asyncio
import json
import pathlib
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tools.completion_claims import ClaimError, ready_tasks
from tools.completion_ownership import OwnershipDenied, OwnershipTable
from tools.completion_scheduler import (
    Candidate,
    Integration,
    Rejected,
    Task,
    Verification,
    run_rolling,
    validate_candidate,
    validate_tasks,
)
from tools.harness_adapter import AuthorityError, IMPLEMENT, mint_capability

HASH = "a" * 64


def task(task_id: str, path: str, *, extra: tuple[str, ...] = (), deps: tuple[str, ...] = ()) -> Task:
    return Task(task_id, path, HASH, (f"{task_id}-T01",), deps, extra)


class PackageGrantTests(unittest.TestCase):
    def test_candidate_may_change_nonempty_subset_of_explicit_grant(self) -> None:
        package = task("PKG-001", "crates/a.rs", extra=("crates/b.rs",))
        validate_candidate(package, Candidate("PKG-001", "1" * 40, "worker", ("crates/a.rs",)))
        validate_candidate(package, Candidate("PKG-001", "1" * 40, "worker", ("crates/b.rs",)))
        validate_candidate(
            package,
            Candidate("PKG-001", "1" * 40, "worker", ("crates/a.rs", "crates/b.rs")),
        )

    def test_candidate_rejects_empty_duplicate_or_outside_paths(self) -> None:
        package = task("PKG-001", "crates/a.rs", extra=("crates/b.rs",))
        for changed in ((), ("crates/a.rs", "crates/a.rs"), ("crates/c.rs",), ("../escape",)):
            with self.subTest(changed=changed), self.assertRaises(Rejected):
                validate_candidate(package, Candidate("PKG-001", "1" * 40, "worker", changed))

    def test_package_rejects_overlapping_or_duplicate_grants(self) -> None:
        with self.assertRaises(ValueError):
            validate_tasks([task("PKG-001", "crates/a", extra=("crates/a/b.rs",))])
        with self.assertRaises(ValueError):
            validate_tasks([task("PKG-001", "crates/a.rs", extra=("crates/a.rs",))])

    def test_harness_capability_accepts_each_granted_file_only(self) -> None:
        package = task("PKG-001", "crates/a.rs", extra=("crates/b.rs",))
        cap = mint_capability(package, IMPLEMENT, "worker")
        self.assertTrue(cap.allows_write("crates/a.rs"))
        self.assertTrue(cap.allows_write("crates/b.rs"))
        self.assertFalse(cap.allows_write("crates/c.rs"))
        with self.assertRaises(AuthorityError):
            mint_capability(task("PKG-002", "tests/forbidden.rs"), IMPLEMENT, "worker")


class OwnershipTests(unittest.TestCase):
    def test_multi_path_grant_allows_subset_and_blocks_overlap(self) -> None:
        table = OwnershipTable()
        self.assertEqual(
            table.admit_many("A", ("crates/a.rs", "crates/b.rs")),
            ("crates/a.rs", "crates/b.rs"),
        )
        table.check_output("A", ("crates/a.rs",))
        table.check_output("A", ("crates/b.rs",))
        table.check_output("A", ("crates/a.rs", "crates/b.rs"))
        with self.assertRaises(OwnershipDenied):
            table.check_output("A", ())
        with self.assertRaises(OwnershipDenied):
            table.check_output("A", ("crates/c.rs",))
        with self.assertRaises(OwnershipDenied):
            table.admit_many("B", ("crates/b.rs", "crates/c.rs"))

        restored = OwnershipTable.restore(table.snapshot())
        self.assertEqual(restored.grant_paths_for("A"), ("crates/a.rs", "crates/b.rs"))


class AcceptanceReadinessTests(unittest.TestCase):
    def _write_completed_claim(self, root: pathlib.Path) -> None:
        path = root / "tasks" / "completion" / "claims.json"
        path.parent.mkdir(parents=True)
        path.write_text(
            json.dumps(
                {
                    "schemaVersion": 1,
                    "claims": {
                        "A-001": {
                            "status": "completed",
                            "session": "worker",
                            "scratchpad": "worklog/A-001.md",
                            "completedNote": "candidate green only",
                        }
                    },
                }
            ),
            encoding="utf-8",
        )

    def _write_receipt(self, root: pathlib.Path, task_id: str = "A-001") -> pathlib.Path:
        path = root / "state" / "completion-integration-receipts" / f"{task_id}.json"
        path.parent.mkdir(parents=True)
        path.write_text(
            json.dumps(
                {
                    "task": task_id,
                    "candidate_revision": "1" * 40,
                    "integrated_revision": "2" * 40,
                    "frozen_tests_sha256": HASH,
                    "rerun": "passed",
                }
            ),
            encoding="utf-8",
        )
        return path

    def test_completed_claim_neither_reissues_task_nor_unlocks_dependent(self) -> None:
        stories = {"A-001": {"deps": []}, "B-001": {"deps": ["A-001"]}}
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            self._write_completed_claim(root)
            self.assertEqual(ready_tasks(root, stories), [])
            self._write_receipt(root)
            self.assertEqual(ready_tasks(root, stories), ["B-001"])

    def test_corrupt_acceptance_receipt_fails_closed(self) -> None:
        stories = {"A-001": {"deps": []}, "B-001": {"deps": ["A-001"]}}
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            receipt = self._write_receipt(root)
            payload = json.loads(receipt.read_text(encoding="utf-8"))
            payload["integrated_revision"] = "not-a-revision"
            receipt.write_text(json.dumps(payload), encoding="utf-8")
            with self.assertRaises(ClaimError):
                ready_tasks(root, stories)


class SchedulerTests(unittest.IsolatedAsyncioTestCase):
    async def test_parallel_preverify_single_integrator_and_accepted_dependencies(self) -> None:
        first = task("PKG-001", "crates/a.rs")
        second = task("PKG-002", "crates/b.rs")
        dependent = task("PKG-003", "crates/c.rs", deps=("PKG-001",))
        candidates = {"PKG-001": "1" * 40, "PKG-002": "2" * 40, "PKG-003": "3" * 40}
        integrated = {"PKG-001": "4" * 40, "PKG-002": "5" * 40, "PKG-003": "6" * 40}

        class Adapter:
            def __init__(self) -> None:
                self.verifying = 0
                self.max_verifying = 0
                self.integrating = 0
                self.max_integrating = 0
                self.verify_pair = asyncio.Event()
                self.postverified: set[str] = set()

            async def execute(self, package: Task) -> Candidate:
                if package.id == "PKG-003":
                    assert "PKG-001" in self.postverified, "dependent executed before ACCEPTED"
                await asyncio.sleep(0)
                return Candidate(package.id, candidates[package.id], f"impl-{package.id}", (package.owned_path,))

            async def verify(self, package: Task, candidate: Candidate) -> Verification:
                self.verifying += 1
                self.max_verifying = max(self.max_verifying, self.verifying)
                if package.id in {"PKG-001", "PKG-002"}:
                    if self.verifying >= 2:
                        self.verify_pair.set()
                    await asyncio.wait_for(self.verify_pair.wait(), timeout=1)
                await asyncio.sleep(0.01)
                self.verifying -= 1
                return Verification(package.id, candidate.revision, f"verify-{package.id}", HASH, 1, package.test_obligations, True)

            async def integrate(self, package: Task, candidate: Candidate) -> Integration:
                self.integrating += 1
                self.max_integrating = max(self.max_integrating, self.integrating)
                await asyncio.sleep(0.01)
                self.integrating -= 1
                return Integration(candidate.revision, integrated[package.id], True)

            async def verify_integrated(self, package: Task, candidate: Candidate, revision: str) -> Verification:
                await asyncio.sleep(0)
                self.postverified.add(package.id)
                return Verification(package.id, revision, f"post-{package.id}", HASH, 1, package.test_obligations, True)

        adapter = Adapter()
        report = await run_rolling(
            [first, second, dependent],
            adapter,
            capacity=3,
            max_unverified=2,
            max_preverify=2,
            stage_timeout=2,
        )
        self.assertTrue(report.complete, (report.statuses, report.errors, report.attempts))
        self.assertEqual(set(report.statuses.values()), {"accepted"})
        self.assertGreaterEqual(adapter.max_verifying, 2)
        self.assertEqual(adapter.max_integrating, 1)
        self.assertEqual(adapter.postverified, {"PKG-001", "PKG-002", "PKG-003"})


if __name__ == "__main__":
    unittest.main()
