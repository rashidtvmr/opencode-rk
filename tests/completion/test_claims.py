"""Frozen tests for the claim/status ledger (tools/completion_claims.py)."""
import json
import pathlib
import tempfile
import unittest

from tools.completion_claims import (
    ROOT,
    ClaimError,
    LEDGER_RELPATH,
    claim,
    drift_errors,
    load_ledger,
    plan_stories,
    ready_tasks,
    reclaim,
    release,
    save_ledger,
    scratchpad_report,
    update,
)


class ClaimsTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.tmp.name)

    def ledger(self):
        return load_ledger(self.root)

    def test_t01_claim_moves_not_started_to_in_progress_with_scratchpad(self):
        doc = claim(self.root, "APP-005", "ses_worker1", "worklog/APP-005-scratch.md")
        row = doc["claims"]["APP-005"]
        self.assertEqual(row["status"], "in-progress")
        self.assertEqual(row["session"], "ses_worker1")
        self.assertEqual(row["scratchpad"], "worklog/APP-005-scratch.md")
        save_ledger(self.root, doc)
        self.assertTrue((self.root / LEDGER_RELPATH).is_file())

    def test_t02_double_claim_of_in_progress_task_is_rejected(self):
        claim(self.root, "APP-005", "ses_worker1", "worklog/a.md")
        with self.assertRaises(ClaimError):
            claim(self.root, "APP-005", "ses_worker2", "worklog/b.md")

    def test_t03_only_owner_can_update_and_transitions_are_legal(self):
        claim(self.root, "APP-005", "ses_worker1", "worklog/a.md")
        with self.assertRaises(ClaimError):
            update(self.root, "APP-005", "ses_worker2", "completed", "done")
        with self.assertRaises(ClaimError):
            update(self.root, "APP-005", "ses_worker1", "not-started", "rewind")
        doc = update(self.root, "APP-005", "ses_worker1", "completed", "tests green")
        self.assertEqual(doc["claims"]["APP-005"]["status"], "completed")
        self.assertEqual(doc["claims"]["APP-005"]["completedNote"], "tests green")

    def test_t04_blocked_is_reachable_from_both_active_states(self):
        claim(self.root, "APP-005", "ses_worker1", "worklog/a.md")
        doc = update(self.root, "APP-005", "ses_worker1", "blocked", "needs creds")
        self.assertEqual(doc["claims"]["APP-005"]["status"], "blocked")
        doc = update(self.root, "APP-005", "ses_worker1", "in-progress", "")
        self.assertEqual(doc["claims"]["APP-005"]["status"], "in-progress")

    def test_t05_release_returns_task_to_not_started(self):
        claim(self.root, "APP-005", "ses_worker1", "worklog/a.md")
        doc = release(self.root, "APP-005", "ses_worker1")
        self.assertEqual(doc["claims"]["APP-005"]["status"], "not-started")

    def test_t06_ready_tasks_respect_claims_and_completed_deps(self):
        save_ledger(
            self.root,
            {
                "schemaVersion": 1,
                "claims": {
                    "APP-001": {"status": "completed", "session": "s1", "scratchpad": "w/1.md"},
                    "APP-002": {"status": "in-progress", "session": "s2", "scratchpad": "w/2.md"},
                },
            },
        )
        stories = {
            "APP-005": {"deps": ["APP-001"]},
            "APP-006": {"deps": ["APP-002"]},
            "APP-007": {"deps": []},
        }
        self.assertEqual(ready_tasks(self.root, stories), ["APP-005", "APP-007"])

    def test_t07_scratchpad_report_hands_back_paths_to_orchestrator(self):
        save_ledger(
            self.root,
            {
                "schemaVersion": 1,
                "claims": {
                    "APP-005": {"status": "in-progress", "session": "s1", "scratchpad": "worklog/a.md"},
                    "APP-006": {"status": "completed", "session": "s1", "scratchpad": "worklog/b.md"},
                    "APP-007": {"status": "in-progress", "session": "s2", "scratchpad": "worklog/c.md"},
                },
            },
        )
        doc = load_ledger(self.root)
        self.assertEqual(scratchpad_report(doc, "s1"), ["worklog/a.md", "worklog/b.md"])

    def test_t08_ledger_rows_validate_and_unknown_task_drift_is_reported(self):
        with self.assertRaises(ClaimError):
            save_ledger(
                self.root,
                {"schemaVersion": 1, "claims": {"X": {"status": "bogus", "session": "s", "scratchpad": "w"}}},
            )
        doc = claim(self.root, "APP-005", "s1", "worklog/a.md")
        errors = drift_errors(doc, {"APP-006": {}})
        self.assertTrue(any("unknown plan task" in e for e in errors))
        self.assertEqual(drift_errors(doc, {"APP-005": {}}), [])

    def test_t09_blocked_task_is_fenced_against_new_claims(self):
        save_ledger(
            self.root,
            {"schemaVersion": 1, "claims": {"APP-005": {"status": "blocked", "session": "s1", "scratchpad": "worklog/a.md"}}},
        )
        with self.assertRaises(ClaimError):
            claim(self.root, "APP-005", "s2", "worklog/b.md")

    def test_t10_plan_stories_loads_union_plan_with_deps(self):
        stories = plan_stories(ROOT)
        self.assertIn("APP-001", stories)
        for tid, story in stories.items():
            self.assertIsInstance(story.get("deps"), list)

    def test_t11_reclaim_is_orchestrator_only_with_evidence_note(self):
        claim(self.root, "APP-005", "s1", "worklog/a.md")
        with self.assertRaises(ClaimError):
            reclaim(self.root, "APP-005", "s-orch", "")
        doc = reclaim(self.root, "APP-005", "s-orch", "heartbeat expired, owner stopped")
        row = doc["claims"]["APP-005"]
        self.assertEqual(row["status"], "not-started")
        self.assertEqual(row["reclaimedBy"], "s-orch")

    def test_t12_completed_and_blocked_require_evidence_note(self):
        claim(self.root, "APP-005", "s1", "worklog/a.md")
        with self.assertRaises(ClaimError):
            update(self.root, "APP-005", "s1", "completed", "")
        update(self.root, "APP-005", "s1", "completed", "cargo test -p cli: 152 passed")

    def test_t13_ready_tasks_excludes_blocked_and_in_progress(self):
        save_ledger(
            self.root,
            {
                "schemaVersion": 1,
                "claims": {
                    "APP-002": {"status": "in-progress", "session": "s2", "scratchpad": "w/2.md"},
                    "APP-006": {"status": "blocked", "session": "s3", "scratchpad": "w/3.md"},
                },
            },
        )
        stories = {"APP-002": {"deps": []}, "APP-006": {"deps": []}, "APP-007": {"deps": []}}
        self.assertEqual(ready_tasks(self.root, stories), ["APP-007"])


if __name__ == "__main__":
    unittest.main()
