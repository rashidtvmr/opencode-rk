import copy
import json
import pathlib
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]

from tools.reconcile_surfaces import build_manifest, expand_reconciliation, summarize, validate_reconciliation


def load(name: str):
    return json.loads((ROOT / name).read_text(encoding="utf-8"))


def repository_names():
    lock = load("sources/upstream.lock.json")
    result = {}
    for source in lock["repositories"]:
        result[source["id"]] = source["url"].removeprefix("https://github.com/").removesuffix(".git")
    return result


class Disc003ReconciliationTests(unittest.TestCase):
    def setUp(self):
        self.rules = load("sources/behavior-surface-rules.json")
        base = load("sources/evidence.json")
        supplemental = load("sources/disc-003-evidence.json")
        self.evidence = {"sources": [*base["sources"], *supplemental["sources"]]}
        self.plan = load("ralph.json")
        self.document = load("sources/disc-003-reconciliation.json")
        self.repositories = repository_names()

    def validate(self, document=None, evidence=None):
        return validate_reconciliation(
            document or self.document,
            self.rules,
            evidence or self.evidence,
            self.plan,
            self.repositories,
        )

    def test_current_reconciliation_is_structurally_valid(self):
        self.assertEqual(self.validate(), [])
        summary = summarize(self.document, self.rules)
        self.assertEqual(summary["surfaceFamilies"], len(self.rules["rules"]))
        self.assertGreater(summary["evidenceReferences"], 0)
        self.assertGreater(summary["unresolvedFindings"], 0)

    def test_every_disc002_candidate_is_represented_exactly_once(self):
        expected = {row["id"] for row in self.rules["rules"]}
        actual = [row["id"] for row in expand_reconciliation(self.document, self.rules)["surfaces"]]
        self.assertEqual(set(actual), expected)
        self.assertEqual(len(actual), len(set(actual)))

    def test_feature_scope_cannot_silently_shrink(self):
        bad = copy.deepcopy(self.document)
        target = bad["reviewedSurfaces"][0]
        target["featureIds"] = ["DISC-003"]
        expanded = expand_reconciliation(bad, self.rules)
        expected = next(row for row in self.rules["rules"] if row["id"] == target["id"])["featureIds"]
        actual = next(row for row in expanded["surfaces"] if row["id"] == target["id"])["featureIds"]
        self.assertEqual(actual, expected)

    def test_reconciled_state_requires_all_four_evidence_classes(self):
        bad = copy.deepcopy(self.document)
        target = bad["reviewedSurfaces"][0]
        target["reviewState"] = "reconciled"
        target["evidence"]["test"] = []
        self.assertTrue(any("reconciled surface lacks test evidence" in error for error in self.validate(bad)))

    def test_partial_state_requires_source_and_explicit_open_work(self):
        bad = copy.deepcopy(self.document)
        target = bad["reviewedSurfaces"][0]
        target["evidence"]["source"] = []
        target["unresolved"] = []
        errors = self.validate(bad)
        self.assertTrue(any("partial surface lacks pinned source evidence" in error for error in errors))
        self.assertTrue(any("partial surface must state unresolved work" in error for error in errors))

    def test_queued_state_cannot_claim_implementation(self):
        bad = copy.deepcopy(self.document)
        if bad["queuedSurfaceIds"]:
            sid = bad["queuedSurfaceIds"].pop()
        else:
            sid = bad["reviewedSurfaces"].pop()["id"]
        bad["reviewedSurfaces"].append({"id": sid, "reviewState": "queued", "implementationStatus": "implemented-v2", "evidence": {"source": [], "caller": [], "test": [], "spec": []}, "finding": "bad", "unresolved": ["bad"]})
        self.assertTrue(any("queued surface must keep implementation status unresolved" in error for error in self.validate(bad)))

    def test_evidence_must_be_pinned_and_from_the_locked_repository(self):
        bad_evidence = copy.deepcopy(self.evidence)
        target_id = self.document["reviewedSurfaces"][0]["evidence"]["source"][0]["evidenceId"]
        row = next(item for item in bad_evidence["sources"] if item["id"] == target_id)
        row["blobSha"] = None
        self.assertTrue(any("lacks pinned blob SHA" in error for error in self.validate(evidence=bad_evidence)))

    def test_manifest_hash_binds_reconciliation_rules_evidence_and_plan(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = pathlib.Path(tmp)
            paths = {}
            for name, source in {
                "reconciliation": ROOT / "sources/disc-003-reconciliation.json",
                "rules": ROOT / "sources/behavior-surface-rules.json",
                "evidence": ROOT / "sources/evidence.json",
                "supplemental": ROOT / "sources/disc-003-evidence.json",
                "plan": ROOT / "ralph.json",
            }.items():
                destination = tmp / source.name
                destination.write_bytes(source.read_bytes())
                paths[name] = destination
            manifest = build_manifest(
                paths["reconciliation"], paths["rules"], paths["evidence"], paths["supplemental"], paths["plan"], self.document
            )
            self.assertEqual(manifest["status"], "in-progress-not-release-evidence")
            for digest in manifest["inputs"].values():
                self.assertRegex(digest, r"^[0-9a-f]{64}$")


if __name__ == "__main__":
    unittest.main()
