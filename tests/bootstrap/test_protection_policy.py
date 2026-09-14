import copy
import json
import pathlib
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]

from tools.validate_protection_policy import (  # noqa: E402
    EXPECTED_CODEOWNER_PATTERNS,
    EXPECTED_RULESET,
    OWNER,
    PROTECTED_PATHS,
    protection_policy_errors,
)
from tools.render_ruleset_import import build_import_artifact, import_artifact_errors


def _copy_policy_fixture(destination: pathlib.Path) -> None:
    for rel in PROTECTED_PATHS:
        source = ROOT / rel
        target = destination / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(source.read_bytes())


class RepositoryProtectionPolicyTests(unittest.TestCase):
    def test_current_policy_is_exact_and_desired_state_only(self):
        self.assertEqual(protection_policy_errors(ROOT), [])
        policy = json.loads((ROOT / ".github/protection-policy.json").read_text(encoding="utf-8"))
        self.assertEqual(policy["desiredExternalRuleset"], EXPECTED_RULESET)
        self.assertFalse(policy["platformState"]["verified"])
        self.assertEqual(len(policy["sourceControlled"]["protectedPaths"]), len(PROTECTED_PATHS))
        artifact = json.loads((ROOT / ".github/rulesets/main.disabled.json").read_text(encoding="utf-8"))
        self.assertEqual(artifact, build_import_artifact(policy))
        self.assertEqual(artifact["enforcement"], "disabled")
        self.assertEqual(artifact["bypass_actors"], [])

    def test_codeowners_owner_and_patterns_cannot_drift(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = pathlib.Path(tmp)
            _copy_policy_fixture(tmp)
            path = tmp / ".github/CODEOWNERS"
            text = path.read_text(encoding="utf-8")
            path.write_text(text.replace(OWNER, "@someone-else", 1), encoding="utf-8")
            self.assertTrue(any("owner drift" in error for error in protection_policy_errors(tmp)))

            path.write_bytes((ROOT / ".github/CODEOWNERS").read_bytes())
            text = path.read_text(encoding="utf-8")
            path.write_text(text.replace(EXPECTED_CODEOWNER_PATTERNS[0], "/.github/workflows/", 1), encoding="utf-8")
            self.assertTrue(any("pattern set/order drifted" in error for error in protection_policy_errors(tmp)))

    def test_protected_path_removal_or_rename_is_rejected(self):
        for rel in (
            "tools/validate_repository.py",
            "tools/render_ruleset_import.py",
            ".github/rulesets/main.disabled.json",
        ):
            with self.subTest(rel=rel), tempfile.TemporaryDirectory() as tmp:
                tmp = pathlib.Path(tmp)
                _copy_policy_fixture(tmp)
                path = tmp / rel
                path.rename(path.with_name(path.name + ".renamed"))
                self.assertTrue(any(f"missing or renamed: {rel}" in error for error in protection_policy_errors(tmp)))

    def test_required_check_review_and_bypass_policy_cannot_weaken(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = pathlib.Path(tmp)
            _copy_policy_fixture(tmp)
            path = tmp / ".github/protection-policy.json"
            policy = json.loads(path.read_text(encoding="utf-8"))
            weak = copy.deepcopy(policy)
            weak["desiredExternalRuleset"]["requiredStatusChecks"] = []
            weak["desiredExternalRuleset"]["requireCodeOwnerReview"] = False
            weak["desiredExternalRuleset"]["allowBypass"] = True
            path.write_text(json.dumps(weak, indent=2) + "\n", encoding="utf-8")
            self.assertTrue(any("desired external ruleset drifted" in error for error in protection_policy_errors(tmp)))

            weak = copy.deepcopy(policy)
            weak["platformState"]["verified"] = True
            path.write_text(json.dumps(weak, indent=2) + "\n", encoding="utf-8")
            self.assertTrue(any("must not claim hosting-platform protection is verified" in error for error in protection_policy_errors(tmp)))

    def test_import_artifact_mutations_fail_closed(self):
        def mutate_bypass(artifact):
            artifact["bypass_actors"] = [{"actor_id": 1, "actor_type": "RepositoryRole", "bypass_mode": "always"}]

        def mutate_branch(artifact):
            artifact["conditions"]["ref_name"]["include"] = ["refs/heads/not-main"]

        def mutate_check(artifact):
            artifact["rules"][-1]["parameters"]["required_status_checks"][0]["context"] = "not-planning"

        def remove_rule(artifact):
            artifact["rules"].pop(0)

        def add_rule(artifact):
            artifact["rules"].append({"type": "creation"})

        def add_server_metadata(artifact):
            artifact["id"] = 123456

        mutations = {
            "active enforcement": lambda artifact: artifact.__setitem__("enforcement", "active"),
            "evaluate enforcement": lambda artifact: artifact.__setitem__("enforcement", "evaluate"),
            "bypass actor": mutate_bypass,
            "branch target": mutate_branch,
            "required check": mutate_check,
            "rule removal": remove_rule,
            "extra rule": add_rule,
            "server metadata": add_server_metadata,
        }
        for label, mutate in mutations.items():
            with self.subTest(label=label), tempfile.TemporaryDirectory() as tmp:
                tmp = pathlib.Path(tmp)
                _copy_policy_fixture(tmp)
                path = tmp / ".github/rulesets/main.disabled.json"
                artifact = json.loads(path.read_text(encoding="utf-8"))
                mutate(artifact)
                path.write_text(json.dumps(artifact, indent=2) + "\n", encoding="utf-8")
                errors = import_artifact_errors(tmp)
                self.assertTrue(any("drifted from desired protection policy" in error for error in errors))
                if "enforcement" in label:
                    self.assertTrue(any("must remain disabled" in error for error in errors))
                if label == "bypass actor":
                    self.assertTrue(any("must not declare bypass actors" in error for error in errors))

    def test_policy_artifact_mismatch_fails_closed(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = pathlib.Path(tmp)
            _copy_policy_fixture(tmp)
            path = tmp / ".github/protection-policy.json"
            policy = json.loads(path.read_text(encoding="utf-8"))
            policy["desiredExternalRuleset"]["targetBranch"] = "not-main"
            path.write_text(json.dumps(policy, indent=2) + "\n", encoding="utf-8")
            self.assertTrue(any("drifted from desired protection policy" in error for error in import_artifact_errors(tmp)))
            self.assertTrue(any("desired external ruleset drifted" in error for error in protection_policy_errors(tmp)))


if __name__ == "__main__":
    unittest.main()
