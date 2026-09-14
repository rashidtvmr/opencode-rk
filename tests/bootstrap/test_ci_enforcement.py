import json
import pathlib
import tempfile
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parents[2]

from tools import ralph_loop, validate_repository  # noqa: E402
from tools.reconcile_surfaces import build_manifest, manifest_drift_errors  # noqa: E402


class CiEnforcementTests(unittest.TestCase):
    def test_canonical_repository_validator_contains_all_required_guards(self):
        self.assertEqual(
            validate_repository.REQUIRED_CHECKS,
            (
                ("ruleset import", ("tools/render_ruleset_import.py", "--check")),
                ("ruleset readback verifier fixtures", ("tools/verify_ruleset_readback.py", "--self-test")),
                ("repository protection", ("tools/validate_protection_policy.py",)),
                ("backlog exhaustion", ("tools/validate_backlog_exhaustion.py",)),
                ("DISC-003 reconciliation", ("tools/reconcile_surfaces.py", "--check-manifest")),
                ("plan", ("tools/validate_plan.py",)),
            ),
        )

    def test_pull_request_ci_runs_canonical_validator_and_bootstrap_suite(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        self.assertIn("pull_request:", workflow)
        self.assertIn("    name: planning", workflow)
        self.assertIn("actions/checkout@11d5960a326750d5838078e36cf38b85af677262", workflow)
        self.assertIn("persist-credentials: false", workflow)
        self.assertIn("/usr/bin/python3 tools/validate_repository.py", workflow)
        self.assertIn(
            "/usr/bin/python3 -m unittest discover -s tests/bootstrap -p 'test_*.py' -v",
            workflow,
        )
        self.assertEqual(validate_repository.workflow_contract_errors(ROOT), [])

        with tempfile.TemporaryDirectory() as tmp:
            tmp = pathlib.Path(tmp)
            workflow_path = tmp / ".github/workflows"
            workflow_path.mkdir(parents=True)
            stale = workflow.replace(
                "        run: /usr/bin/python3 tools/validate_repository.py",
                "        run: /usr/bin/python3 tools/validate_plan.py",
            )
            (workflow_path / "ci.yml").write_text(stale, encoding="utf-8")
            self.assertTrue(any("missing exact enforcement line" in error for error in validate_repository.workflow_contract_errors(tmp)))

            bypassed = workflow.replace(
                "      - name: Validate repository contracts",
                "      - name: Validate repository contracts\n        continue-on-error: true",
            )
            (workflow_path / "ci.yml").write_text(bypassed, encoding="utf-8")
            self.assertTrue(any("continue-on-error" in error for error in validate_repository.workflow_contract_errors(tmp)))

            renamed = workflow.replace("  planning:\n    name: planning", "  planning-renamed:\n    name: planning")
            (workflow_path / "ci.yml").write_text(renamed, encoding="utf-8")
            self.assertTrue(any("missing the planning job" in error for error in validate_repository.workflow_contract_errors(tmp)))

            dependent = workflow.replace("    runs-on: ubuntu-latest", "    needs: rust\n    runs-on: ubuntu-latest", 1)
            (workflow_path / "ci.yml").write_text(dependent, encoding="utf-8")
            self.assertTrue(any("must not use needs" in error for error in validate_repository.workflow_contract_errors(tmp)))

            filtered = workflow.replace("  pull_request:\n", "  pull_request:\n    paths: ['tools/**']\n", 1)
            (workflow_path / "ci.yml").write_text(filtered, encoding="utf-8")
            self.assertTrue(any("trigger/permission prefix drifted" in error for error in validate_repository.workflow_contract_errors(tmp)))

            writable = workflow.replace("  contents: read", "  contents: write", 1)
            (workflow_path / "ci.yml").write_text(writable, encoding="utf-8")
            self.assertTrue(any("trigger/permission prefix drifted" in error for error in validate_repository.workflow_contract_errors(tmp)))

    def test_controller_verification_uses_canonical_repository_validator(self):
        commands = ralph_loop.DEFAULT_SETTINGS["verificationCommands"]
        self.assertTrue(commands)
        self.assertEqual(commands[0][1:], ["tools/validate_repository.py"])
        auto_drive = (ROOT / "tools/auto_drive.py").read_text(encoding="utf-8")
        self.assertIn('[sys.executable, "tools/validate_repository.py"]', auto_drive)

        with tempfile.TemporaryDirectory() as tmp:
            settings = pathlib.Path(tmp) / "controller.settings.json"
            settings.write_text(json.dumps({"verificationCommands": [["/bin/true"]]}), encoding="utf-8")
            with mock.patch.object(ralph_loop, "SETTINGS", settings):
                loaded = ralph_loop.load_settings()
            self.assertEqual(loaded["verificationCommands"][:2], ralph_loop.MANDATORY_VERIFICATION_COMMANDS)
            self.assertEqual(loaded["verificationCommands"][2], ["/bin/true"])

    def test_checked_in_disc_manifest_matches_canonical_inputs(self):
        reconciliation_path = ROOT / "sources/disc-003-reconciliation.json"
        rules_path = ROOT / "sources/behavior-surface-rules.json"
        evidence_path = ROOT / "sources/evidence.json"
        supplemental_path = ROOT / "sources/disc-003-evidence.json"
        plan_path = ROOT / "ralph.json"
        manifest_path = ROOT / "sources/disc-003-reconciliation.manifest.json"
        reconciliation = json.loads(reconciliation_path.read_text(encoding="utf-8"))
        expected = build_manifest(
            reconciliation_path,
            rules_path,
            evidence_path,
            supplemental_path,
            plan_path,
            reconciliation,
        )
        actual = json.loads(manifest_path.read_text(encoding="utf-8"))
        self.assertEqual(manifest_drift_errors(actual, expected), [])

        stale = dict(actual)
        stale["evidenceReferences"] = actual["evidenceReferences"] - 1
        self.assertTrue(any("evidenceReferences" in error for error in manifest_drift_errors(stale, expected)))


if __name__ == "__main__":
    unittest.main()
