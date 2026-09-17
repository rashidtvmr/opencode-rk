"""Structural tests with synthetic legacy data; not full-repository acceptance."""
import copy
import hashlib
import json
import pathlib
import shutil
import tempfile
import unittest

from tools.completion_plan import InvalidPlan, ROOT, graph_errors, legacy_report, load, proof_errors, release_errors, safe_path


class PlanTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.tmp.name) / "repo"
        self.root.mkdir()
        for path in ("ralph.completion.json", "config/completion-controller.json"):
            dest = self.root / path
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / path, dest)
        shutil.copytree(ROOT / "tasks/completion", self.root / "tasks/completion")
        self.old = [{"id": tid, "status": "accepted", "userStory": "TBD - see source audit", "dependencyIds": [], "testObligations": [tid + "-T01"]} for tid in ("BASE-001", "PROV-001", "NEW-001")]
        (self.root / "ralph.json").write_text(json.dumps({"userStories": self.old}))
        (self.root / "requirements").mkdir()
        (self.root / "requirements/user-requirements.json").write_text(json.dumps({"requirements": [{"id": "REQ-001", "mandatory": True, "tasks": [r["id"] for r in self.old]}]}))

    def tearDown(self):
        self.tmp.cleanup()

    def edit(self, relative, change):
        path = self.root / relative
        obj = json.loads(path.read_text())
        change(obj)
        path.write_text(json.dumps(obj))

    def test_additions_have_concrete_scenarios_and_twenty_audit_roots(self):
        plan = load(self.root)
        self.assertGreaterEqual(len(plan["stories"]), 90)
        self.assertEqual(len([s for s in plan["stories"].values() if not s["deps"]]), 20)
        self.assertTrue(all(len(s["tests"]) >= 5 and s["mandatory"] for s in plan["stories"].values()))
        self.assertTrue(all(s["status"] == "not-started" for s in plan["stories"].values()))

    def test_legacy_records_are_preserved_without_becoming_evidence(self):
        plan = load(self.root)
        self.assertEqual(list(plan["legacyStories"].values()), self.old)
        report = legacy_report(plan)
        flattened = [tid for ids in report["auditAssignments"].values() for tid in ids]
        self.assertEqual(set(flattened), {r["id"] for r in self.old})
        self.assertEqual(len(flattened), len(self.old))
        self.assertIn("NEW-001", report["auditAssignments"]["AUD-020"])
        self.assertEqual(len(report["requiresRevalidation"]), 3)

    def test_missing_include_fails(self):
        (self.root / "tasks/completion/local.json").unlink()
        with self.assertRaises(OSError):
            load(self.root)

    def test_unsafe_include_fails(self):
        self.edit("ralph.completion.json", lambda x: x["includes"].append("../outside.json"))
        with self.assertRaises(InvalidPlan):
            load(self.root)

    def test_duplicate_task_fails(self):
        self.edit("tasks/completion/local.json", lambda x: x["stories"].append(copy.deepcopy(x["stories"][0])))
        with self.assertRaises(InvalidPlan):
            load(self.root)

    def test_missing_dependency_and_cycle_fail(self):
        self.assertTrue(graph_errors({"A": {"deps": ["missing"]}}))
        self.assertTrue(graph_errors({"A": {"deps": ["B"]}, "B": {"deps": ["A"]}}))

    def test_missing_scenarios_fail(self):
        self.edit("tasks/completion/tui.json", lambda x: x["stories"][0].update(tests=["only one"]))
        with self.assertRaises(InvalidPlan):
            load(self.root)

    def test_unknown_requirement_fails(self):
        self.edit("tasks/completion/tui.json", lambda x: x.update(requirementIds=["missing"]))
        with self.assertRaises(InvalidPlan):
            load(self.root)

    def test_controller_caps_and_budget_cannot_be_silently_relaxed(self):
        self.edit("config/completion-controller.json", lambda x: x.update(maxHeavyValidations=20))
        with self.assertRaises(InvalidPlan):
            load(self.root)

    def test_empty_legacy_plan_fails(self):
        (self.root / "ralph.json").write_text('{"userStories": []}')
        with self.assertRaises(InvalidPlan):
            load(self.root)

    def test_removed_legacy_task_breaks_requirement_mapping(self):
        self.edit("ralph.json", lambda x: x["userStories"].pop())
        with self.assertRaises(InvalidPlan):
            load(self.root)

    def test_duplicate_json_keys_fail(self):
        (self.root / "ralph.json").write_text('{"userStories": [], "userStories": []}')
        with self.assertRaises(InvalidPlan):
            load(self.root)

    def test_no_acceptance_from_all_legacy_flags(self):
        plan = load(self.root)
        evidence = pathlib.Path(self.tmp.name) / "trusted"
        evidence.mkdir()
        (evidence / "release.json").write_text(json.dumps({"commit": "a" * 40, "planSha256": plan["digest"], "tasks": {}}))
        errors = release_errors(plan, evidence, "a" * 40, self.root)
        self.assertTrue(any("BASE-001" in e for e in errors))
        self.assertTrue(any("APP-001" in e for e in errors))
        self.assertTrue(any("ios-device" in e for e in errors))

    def test_worker_repository_is_not_trusted_evidence_root(self):
        plan = load(self.root)
        self.assertTrue(release_errors(plan, self.root / "evidence", "a" * 40, self.root))

    def test_missing_hash_and_zero_test_proof_rejected(self):
        proof = {"status": "accepted", "testedCommit": "a" * 40, "testCount": 0}
        self.assertTrue(proof_errors(proof, self.root, "a" * 40, ["A-T01"]))

    def test_evidence_hash_and_revision_are_checked(self):
        path = self.root / "run.log"
        path.write_text("validator fixture; not a product execution")
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        proof = {"status": "accepted", "testedCommit": "a" * 40, "testCount": 1, "testObligations": ["A-T01"], "frozenTestsSha256": "f" * 64, "verifier": "v", "implementer": "i", "artifacts": [{"path": "run.log", "sha256": digest, "kind": kind} for kind in ("red", "green", "integrated-green")]}
        self.assertEqual(proof_errors(proof, self.root, "a" * 40, ["A-T01"]), [])
        path.write_text("tampered")
        self.assertTrue(proof_errors(proof, self.root, "a" * 40, ["A-T01"]))
        self.assertTrue(proof_errors(proof, self.root, "b" * 40, ["A-T01"]))

    def test_evidence_path_traversal_is_rejected(self):
        with self.assertRaises(InvalidPlan):
            safe_path(self.root, "../secret")

    def test_plan_digest_changes_when_test_obligation_changes(self):
        before = load(self.root)["digest"]
        self.edit("tasks/completion/local.json", lambda x: x["stories"][0]["tests"].append("Additional required security case."))
        self.assertNotEqual(load(self.root)["digest"], before)


if __name__ == "__main__":
    unittest.main()
