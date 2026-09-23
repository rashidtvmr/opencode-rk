"""Independent REL-002 verifier checks.

The release validator and fixtures predate this verifier lane. These tests keep
the observable receipt contract frozen while exposing the still-missing CI
caller as a RED check.
"""

from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "fixtures" / "release-tdd"
VALID_REVISION = "863a0019fc6f0b9779d867792fe00c4a9ab84c87"
REPORT_LIMIT = 64 * 1024


class ReleaseTddVerifierTests(unittest.TestCase):
    def _run(self, fixture: str = "") -> tuple[int, dict, bytes, Path]:
        source = FIXTURES / fixture
        with tempfile.TemporaryDirectory(prefix="rel002-verify-") as tmp:
            temp_root = Path(tmp)
            candidate = Path(tmp) / "candidate"
            shutil.copytree(source, candidate)
            output = Path(tmp) / "report.json"
            before_candidate = {
                path.relative_to(candidate): hashlib.sha256(path.read_bytes()).digest()
                for path in candidate.rglob("*")
                if path.is_file()
            }
            completed = subprocess.run(
                [
                    sys.executable,
                    str(ROOT / "tools" / "check_release_tdd.py"),
                    "--revision",
                    VALID_REVISION,
                    "--manifest",
                    str(candidate / "frozen.json"),
                    "--receipts",
                    str(candidate / "receipts"),
                    "--gates",
                    str(candidate / "gates"),
                    "--out",
                    str(output),
                    "--timeout",
                    "5",
                ],
                cwd=temp_root,
                check=False,
                capture_output=True,
            )
            self.assertEqual(completed.stderr, b"")
            self.assertTrue(output.is_file())
            payload = output.read_bytes()
            self.assertLessEqual(len(payload), REPORT_LIMIT)
            self.assertEqual(payload, completed.stdout)
            self.assertEqual(
                before_candidate,
                {
                    path.relative_to(candidate): hashlib.sha256(path.read_bytes()).digest()
                    for path in candidate.rglob("*")
                    if path.is_file()
                },
            )
            self.assertEqual(sorted(path.name for path in temp_root.iterdir()), ["candidate", "report.json"])
            return completed.returncode, json.loads(payload), payload, candidate

    def test_full_exact_revision_proof_passes(self) -> None:
        code, report, _, _ = self._run()
        self.assertEqual(code, 0)
        self.assertTrue(report["passed"])
        self.assertEqual(
            report["checks"],
            {
                "red_proof": "pass",
                "frozen_intact": "pass",
                "green_on_frozen": "pass",
                "verifier_rerun": "pass",
            },
        )
        self.assertEqual(
            report["partition"],
            "strict-tdd-independent-verification-validator",
        )
        self.assertEqual(report["evidence_rev"], VALID_REVISION)

    def test_missing_or_import_only_red_proof_fails(self) -> None:
        for fixture in ("fail-no-red", "fail-import-only"):
            with self.subTest(fixture=fixture):
                code, report, _, _ = self._run(fixture)
                self.assertEqual(code, 2)
                self.assertFalse(report["passed"])
                self.assertEqual(report["checks"]["red_proof"], "fail")
                self.assertIn("red", report["failing_fixture"])

    def test_mutated_frozen_hash_fails(self) -> None:
        code, report, _, _ = self._run("fail-mutated")
        self.assertEqual(code, 2)
        self.assertEqual(report["checks"]["frozen_intact"], "fail")
        self.assertIn("tests/test_slice.py", report["failing_fixture"])

    def test_wrong_verifier_revision_fails_closed(self) -> None:
        code, report, _, _ = self._run("fail-wrong-rev")
        self.assertEqual(code, 2)
        self.assertEqual(report["checks"]["verifier_rerun"], "fail")
        self.assertEqual(report["evidence_rev"], VALID_REVISION)
        self.assertEqual(report["reason"], "verifier-revision-mismatch")

    def test_worker_self_report_is_not_independent_evidence(self) -> None:
        code, report, _, _ = self._run("fail-no-verifier")
        self.assertEqual(code, 2)
        self.assertEqual(report["checks"]["verifier_rerun"], "fail")
        self.assertEqual(report["reason"], "self-report-not-evidence")
        self.assertIn("verifier", report["failing_fixture"])

    def test_auto005_material_is_rejected_as_duplicate_owner(self) -> None:
        code, report, _, _ = self._run("pipeline-only")
        self.assertEqual(code, 2)
        self.assertFalse(report["passed"])
        self.assertEqual(
            report["reason"],
            "duplicate-of-existing-owner:AUTO-005",
        )

    def test_deterministic_and_fixture_read_only(self) -> None:
        source_hashes = {
            path.relative_to(FIXTURES): hashlib.sha256(path.read_bytes()).digest()
            for path in FIXTURES.rglob("*")
            if path.is_file()
        }
        with tempfile.TemporaryDirectory(prefix="rel002-determinism-") as tmp:
            reports: list[bytes] = []
            for index in (1, 2):
                candidate = Path(tmp) / f"candidate-{index}"
                shutil.copytree(FIXTURES, candidate)
                output = Path(tmp) / f"report-{index}.json"
                completed = subprocess.run(
                    [
                        sys.executable,
                        str(ROOT / "tools" / "check_release_tdd.py"),
                        "--revision",
                        VALID_REVISION,
                        "--manifest",
                        str(candidate / "frozen.json"),
                        "--receipts",
                        str(candidate / "receipts"),
                        "--gates",
                        str(candidate / "gates"),
                        "--out",
                        str(output),
                        "--timeout",
                        "5",
                    ],
                    cwd=Path(tmp),
                    check=False,
                    capture_output=True,
                )
                self.assertEqual(completed.returncode, 0)
                self.assertEqual(completed.stderr, b"")
                reports.append(output.read_bytes())
                self.assertLessEqual(len(reports[-1]), REPORT_LIMIT)
            self.assertEqual(reports[0], reports[1])

        self.assertEqual(
            source_hashes,
            {
                path.relative_to(FIXTURES): hashlib.sha256(path.read_bytes()).digest()
                for path in FIXTURES.rglob("*")
                if path.is_file()
            },
        )

    def test_ci_calls_release_validator(self) -> None:
        """RED until the release validator has an actual CI caller."""
        workflow = (ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("tools/check_release_tdd.py", workflow)


if __name__ == "__main__":
    unittest.main()
