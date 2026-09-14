import copy
import json
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]

from tools.validate_backlog_exhaustion import (  # noqa: E402
    CATEGORY_IDS,
    build_expected_ledger,
    disc_status_errors,
    validate_ledger,
)


def load_ledger():
    return json.loads((ROOT / "sources/backlog-exhaustion.json").read_text(encoding="utf-8"))


class BacklogExhaustionTests(unittest.TestCase):
    def test_current_ledger_is_valid_and_exact(self):
        document = load_ledger()
        self.assertEqual(validate_ledger(document, ROOT), [])
        self.assertEqual(document["summary"]["storyCount"], 219)
        self.assertEqual(document["summary"]["controllerAccepted"], 133)
        self.assertEqual(document["summary"]["nonAccepted"], 86)
        self.assertEqual(
            document["summary"]["classificationCounts"],
            {
                "dependency-constrained": 22,
                "explicit-blocker": 9,
                "local-implemented-stale": 21,
                "unresolved-decomposition": 34,
            },
        )

    def test_missing_story_is_rejected(self):
        bad = copy.deepcopy(load_ledger())
        bad["stories"].pop()
        self.assertTrue(any("missing non-accepted stories" in error for error in validate_ledger(bad, ROOT)))

    def test_duplicate_story_is_rejected(self):
        bad = copy.deepcopy(load_ledger())
        bad["stories"].append(copy.deepcopy(bad["stories"][0]))
        self.assertTrue(any("duplicate classifications" in error for error in validate_ledger(bad, ROOT)))

    def test_accepted_story_cannot_enter_residual_ledger(self):
        bad = copy.deepcopy(load_ledger())
        bad["stories"][0]["id"] = "AGENT-001"
        errors = validate_ledger(bad, ROOT)
        self.assertTrue(any("accepted/unknown stories" in error for error in errors))

    def test_explicit_blocker_category_is_locked(self):
        bad = copy.deepcopy(load_ledger())
        row = next(item for item in bad["stories"] if item["id"] == "OPS-009")
        row["category"] = "unresolved-decomposition"
        self.assertTrue(any("category drift for explicit-blocker" in error for error in validate_ledger(bad, ROOT)))
        self.assertEqual(len(CATEGORY_IDS["explicit-blocker"]), 9)

    def test_dependency_constrained_category_is_locked(self):
        bad = copy.deepcopy(load_ledger())
        row = next(item for item in bad["stories"] if item["id"] == "UI-014")
        row["category"] = "unresolved-decomposition"
        self.assertTrue(any("category drift for dependency-constrained" in error for error in validate_ledger(bad, ROOT)))
        self.assertEqual(len(CATEGORY_IDS["dependency-constrained"]), 22)

    def test_surface_and_evidence_projection_cannot_silently_drift(self):
        bad = copy.deepcopy(load_ledger())
        row = next(item for item in bad["stories"] if item["id"] == "SHARE-003")
        row["surfaceIds"] = []
        row["evidenceIds"] = []
        errors = validate_ledger(bad, ROOT)
        self.assertTrue(any("surfaceIds drifted" in error for error in errors))
        self.assertTrue(any("evidenceIds drifted" in error for error in errors))

    def test_accidental_disc_acceptance_is_rejected_by_expected_projection(self):
        reconciliation = json.loads((ROOT / "sources/disc-003-reconciliation.json").read_text(encoding="utf-8"))
        manifest = json.loads((ROOT / "sources/disc-003-reconciliation.manifest.json").read_text(encoding="utf-8"))
        source_map = json.loads((ROOT / "workspaces/DISC-003/source-map.json").read_text(encoding="utf-8"))
        task_text = (ROOT / "tasks/DISC-003.md").read_text(encoding="utf-8")
        progress_text = (ROOT / "workspaces/DISC-003/progress.md").read_text(encoding="utf-8")
        self.assertEqual(disc_status_errors(reconciliation, manifest, source_map, task_text, progress_text), [])

        bad_reconciliation = copy.deepcopy(reconciliation)
        bad_reconciliation["status"] = "accepted"
        self.assertTrue(any("reconciliation status changed" in error for error in disc_status_errors(
            bad_reconciliation, manifest, source_map, task_text, progress_text
        )))

        bad_reconciliation = copy.deepcopy(reconciliation)
        bad_reconciliation["reviewedSurfaces"][0]["reviewState"] = "reconciled"
        self.assertTrue(any("all reviewed surfaces to remain partial" in error for error in disc_status_errors(
            bad_reconciliation, manifest, source_map, task_text, progress_text
        )))

        bad_manifest = copy.deepcopy(manifest)
        bad_manifest["status"] = "release-evidence"
        self.assertTrue(any("manifest status changed" in error for error in disc_status_errors(
            reconciliation, bad_manifest, source_map, task_text, progress_text
        )))

        bad_source_map = copy.deepcopy(source_map)
        bad_source_map["status"] = "accepted"
        self.assertTrue(any("source-map status changed" in error for error in disc_status_errors(
            reconciliation, manifest, bad_source_map, task_text, progress_text
        )))

        expected = build_expected_ledger(ROOT)
        self.assertEqual(expected["status"], "in-progress-not-release-evidence")


if __name__ == "__main__":
    unittest.main()
