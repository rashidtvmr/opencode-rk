"""Offline tests for the non-destructive integration pump."""
import unittest
from unittest.mock import patch

from tools import recovery_delivery as delivery


class DeliveryTests(unittest.TestCase):
    def setUp(self):
        self.info = {"ready": [{"id": "TASK-1", "stale": False}],
                     "canonicalSha": "a" * 40, "dirtyCanonical": False,
                     "integrationLocked": False}

    def test_no_candidate(self):
        self.info["ready"] = []
        self.assertEqual(delivery.integrate_one(self.info, owner="delivery", enabled=True)["action"], "idle")

    def test_dirty_tree_prevents_merge(self):
        self.info["dirtyCanonical"] = True
        with patch.object(delivery.subprocess, "run") as run:
            actual = delivery.integrate_one(self.info, owner="delivery", enabled=True)
        self.assertEqual(actual["action"], "blocked")
        run.assert_not_called()

    def test_stale_and_lock_block(self):
        self.info["ready"][0]["stale"] = True
        self.assertEqual(delivery.integrate_one(self.info, owner="delivery", enabled=True)["action"], "blocked")
        self.info["ready"][0]["stale"] = False
        self.info["integrationLocked"] = True
        self.assertEqual(delivery.integrate_one(self.info, owner="delivery", enabled=True)["action"], "blocked")

    def test_default_does_not_merge(self):
        with patch.object(delivery.subprocess, "run") as run:
            actual = delivery.integrate_one(self.info, owner="delivery", enabled=False)
        self.assertEqual(actual["action"], "would-integrate")
        run.assert_not_called()

    def test_opt_in_calls_recovery_state(self):
        from subprocess import CompletedProcess
        with patch.object(delivery, "git", return_value="a" * 40), patch.object(
            delivery.subprocess, "run", return_value=CompletedProcess([], 0, stdout="ok")
        ) as run:
            actual = delivery.integrate_one(self.info, owner="delivery", enabled=True)
        self.assertEqual(actual["action"], "integrated")
        self.assertTrue(actual["requiresJudge"])
        self.assertIn("recovery_state.py", run.call_args.args[0][1])

    def test_guard_failure_blocks_canonical_change(self):
        from subprocess import CompletedProcess
        with patch.object(delivery.subprocess, "run", return_value=CompletedProcess([], 1, stdout="blocked", stderr="")) as run:
            actual = delivery.integrate_one(self.info, owner="delivery", enabled=True)
        self.assertEqual(actual["action"], "blocked")
        self.assertEqual(run.call_count, 1)


if __name__ == "__main__":
    unittest.main()
