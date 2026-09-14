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
        with tempfile.TemporaryDirectory() as tmp:
            tmp = pathlib.Path(tmp)
            _copy_policy_fixture(tmp)
            (tmp / "tools/validate_repository.py").rename(tmp / "tools/validate_repository-renamed.py")
            self.assertTrue(any("missing or renamed: tools/validate_repository.py" in error for error in protection_policy_errors(tmp)))

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


if __name__ == "__main__":
    unittest.main()
