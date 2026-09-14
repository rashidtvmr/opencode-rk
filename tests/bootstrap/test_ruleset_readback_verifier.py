import copy
import json
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]

from tools.verify_ruleset_readback import (  # noqa: E402
    INCOMPLETE_FIXTURE,
    MATCH_FIXTURE,
    MISMATCH_FIXTURE,
    STATUS_MISMATCH,
    STATUS_UNVERIFIED,
    STATUS_VERIFIED,
    fixture_self_test,
    verify_document,
)


def load(path):
    return json.loads(path.read_text(encoding="utf-8"))


class RulesetReadbackVerifierTests(unittest.TestCase):
    def test_checked_in_fixtures_cover_match_mismatch_and_incomplete(self):
        self.assertEqual(fixture_self_test(ROOT), [])
        self.assertEqual(verify_document(load(MATCH_FIXTURE), ROOT)["status"], STATUS_VERIFIED)
        self.assertEqual(verify_document(load(MISMATCH_FIXTURE), ROOT)["status"], STATUS_MISMATCH)
        self.assertEqual(verify_document(load(INCOMPLETE_FIXTURE), ROOT)["status"], STATUS_UNVERIFIED)

    def test_missing_bypass_evidence_is_unverified_not_a_match(self):
        result = verify_document(load(INCOMPLETE_FIXTURE), ROOT)
        self.assertFalse(result["verified"])
        self.assertTrue(any("cannot prove the no-bypass requirement" in error for error in result["errors"]))

    def test_missing_repository_source_is_unverified_not_a_match(self):
        document = copy.deepcopy(load(MATCH_FIXTURE))
        document.pop("source")
        result = verify_document(document, ROOT)
        self.assertEqual(result["status"], STATUS_UNVERIFIED)
        self.assertTrue(any("repository source" in error for error in result["errors"]))

    def test_semantic_drift_fails_closed(self):
        mutations = {}

        def enforcement(document):
            document["enforcement"] = "disabled"

        mutations["enforcement"] = enforcement

        def branch(document):
            document["conditions"]["ref_name"]["include"] = ["refs/heads/not-main"]

        mutations["branch"] = branch

        def bypass(document):
            document["bypass_actors"] = [{"actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "always"}]

        mutations["bypass"] = bypass

        def check(document):
            rule = next(item for item in document["rules"] if item["type"] == "required_status_checks")
            rule["parameters"]["required_status_checks"][0]["context"] = "not-planning"

        mutations["check"] = check

        def remove_rule(document):
            document["rules"] = [item for item in document["rules"] if item["type"] != "deletion"]

        mutations["rule-removal"] = remove_rule

        def extra_rule(document):
            document["rules"].append({"type": "creation"})

        mutations["extra-rule"] = extra_rule

        for label, mutate in mutations.items():
            with self.subTest(label=label):
                document = copy.deepcopy(load(MATCH_FIXTURE))
                mutate(document)
                self.assertEqual(verify_document(document, ROOT)["status"], STATUS_MISMATCH)

    def test_unowned_integration_or_merge_policy_is_not_guessed(self):
        document = copy.deepcopy(load(MATCH_FIXTURE))
        status = next(item for item in document["rules"] if item["type"] == "required_status_checks")
        status["parameters"]["required_status_checks"][0]["integration_id"] = 12345
        result = verify_document(document, ROOT)
        self.assertEqual(result["status"], STATUS_UNVERIFIED)
        self.assertTrue(any("integration_id" in error for error in result["errors"]))

        document = copy.deepcopy(load(MATCH_FIXTURE))
        pull_request = next(item for item in document["rules"] if item["type"] == "pull_request")
        pull_request["parameters"]["allowed_merge_methods"] = ["merge"]
        result = verify_document(document, ROOT)
        self.assertEqual(result["status"], STATUS_UNVERIFIED)
        self.assertTrue(any("merge-method policy" in error for error in result["errors"]))

    def test_only_documented_server_metadata_is_tolerated(self):
        document = copy.deepcopy(load(MATCH_FIXTURE))
        document["unknown_server_field"] = "surprise"
        result = verify_document(document, ROOT)
        self.assertEqual(result["status"], STATUS_MISMATCH)
        self.assertTrue(any("unsupported top-level fields" in error for error in result["errors"]))

    def test_fixture_success_never_changes_repository_platform_attestation(self):
        self.assertEqual(verify_document(load(MATCH_FIXTURE), ROOT)["status"], STATUS_VERIFIED)
        policy = json.loads((ROOT / ".github/protection-policy.json").read_text(encoding="utf-8"))
        self.assertFalse(policy["platformState"]["verified"])


if __name__ == "__main__":
    unittest.main()
