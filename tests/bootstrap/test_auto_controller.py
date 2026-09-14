"""Deterministic tests for the plan model, validator and controller selection.

No network, no worker invocation. These test the controller's decision logic,
which is the part that must never lie about eligibility or acceptance.
"""
from __future__ import annotations

import importlib
import json
import pathlib
import sys
import tempfile
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

plan_model = importlib.import_module("tools.plan_model")
ralph_loop = importlib.import_module("tools.ralph_loop")


def _all_not_started_plan():
    plan = plan_model.load_plan(ROOT)
    for story in plan.stories.values():
        story.status = "not-started"
    return plan


class PlanModelTests(unittest.TestCase):
    def test_plan_loads_and_is_structurally_sound(self) -> None:
        plan = plan_model.load_plan(ROOT)
        self.assertEqual(plan.errors, [])
        self.assertEqual(len(plan.stories), 219)
        self.assertEqual(len(plan.requirements), 38)

    def test_m0_stories_are_ready_first(self) -> None:
        plan = _all_not_started_plan()
        ready = plan.ready(set(), set())
        self.assertIn("AUTO-001", ready)
        self.assertIn("AUTO-002", ready)
        self.assertIn("DISC-001", ready)
        # A later-milestone story is not ready before M0 is accepted.
        self.assertNotIn("SESS-001", ready)

    def test_accepting_m0_advances_the_queue(self) -> None:
        plan = _all_not_started_plan()
        m0 = sorted(t for t, s in plan.stories.items() if s.rank == 0)
        accepted = set(m0)
        ready = plan.ready(accepted, set())
        self.assertNotIn("SESS-001", ready)
        self.assertTrue(any(t.startswith("BASE") for t in ready))

    def test_blocked_dependency_holds_dependents_out(self) -> None:
        plan = _all_not_started_plan()
        m0 = sorted(t for t, s in plan.stories.items() if s.rank == 0)
        ready = plan.ready(set(), set(m0))
        # M0 itself is not gated by its own block, but nothing in M1+ may start.
        self.assertEqual(set(ready), set(m0))
        waiting = plan.blocked_dependents(set(m0))
        self.assertIn("SESS-001", waiting)
        self.assertIn("BASE-001", waiting)

    def test_rank_lookup_rejects_unknown_prefix(self) -> None:
        with self.assertRaises(plan_model.PlanError):
            plan_model._rank_for("ZZZ-001")


class ControllerSelectionTests(unittest.TestCase):
    def _ledger(self, *, all_not_started=False):
        plan = _all_not_started_plan() if all_not_started else plan_model.load_plan(ROOT)
        with tempfile.TemporaryDirectory() as tmp, mock.patch.object(
            ralph_loop, "LEDGER", pathlib.Path(tmp) / "controller.json"
        ):
            ledger = ralph_loop.Ledger(plan)
        return ledger, plan

    def test_ready_queue_is_bounded_by_limit(self) -> None:
        ledger, _ = self._ledger(all_not_started=True)
        self.assertLessEqual(len(ralph_loop.ready_queue(ledger, 2)), 2)

    def test_ownership_lock_serializes_same_task(self) -> None:
        ledger, _ = self._ledger()
        picked = ledger.claim(["AUTO-001", "AUTO-002"])
        self.assertEqual(picked, ["AUTO-001", "AUTO-002"])
        # A second claim while both are held yields nothing new.
        self.assertEqual(ledger.claim(["AUTO-001", "AUTO-002"]), [])

    def test_accepted_tasks_leave_the_ready_queue(self) -> None:
        ledger, _ = self._ledger(all_not_started=True)
        before = set(ralph_loop.ready_queue(ledger, 20))
        self.assertIn("AUTO-001", before)
        ledger.tasks["AUTO-001"].status = "accepted"
        after = set(ralph_loop.ready_queue(ledger, 20))
        self.assertNotIn("AUTO-001", after)

    def test_status_report_counts_statuses(self) -> None:
        ledger, plan = self._ledger()
        report = ralph_loop.status_report(ledger, plan)
        self.assertIn("stories=219", report)
        self.assertIn("accepted=133", report)
        self.assertIn("in-progress=30", report)
        self.assertIn("not-started=56", report)


class ValidatorTests(unittest.TestCase):
    def test_validator_passes_on_shipped_plan(self) -> None:
        module = importlib.import_module("tools.validate_plan")
        self.assertEqual(module.main(), 0)

    def test_obligation_ids_are_unique(self) -> None:
        module = importlib.import_module("tools.validate_plan")
        self.assertEqual(module._check_obligations(ROOT), [])


if __name__ == "__main__":
    unittest.main()
