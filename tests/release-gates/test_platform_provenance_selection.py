"""R1 contract: Mac and Web must select different trusted provenance bytes."""
from __future__ import annotations

import hashlib
import json
import shlex
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/release-gates.yml"
sys.path.insert(0, str(ROOT))
from release.receipts import ReceiptError, validate_trusted_provenance  # noqa: E402


def _psych(source: str) -> dict:
    """Use Ruby Psych, rather than a Python YAML implementation, for workflow YAML."""
    import subprocess

    script = (
        'require "yaml"; require "json"; '
        'puts JSON.generate(YAML.safe_load(STDIN.read, permitted_classes: [], aliases: false))'
    )
    result = subprocess.run(["ruby", "-e", script], input=source, text=True,
                            capture_output=True, check=True, timeout=10)
    return json.loads(result.stdout)


def _jobs(document: dict) -> dict:
    return document["jobs"]


def _download_selection(job: dict) -> tuple[dict, list[str]]:
    for step in job["steps"]:
        command = str(step.get("run", ""))
        if command.startswith("gh run download ") and "trusted" in command:
            return step.get("env", {}), shlex.split(command)
    raise AssertionError("trusted provenance download step missing")


def _selector(job: dict) -> tuple[str, str, str, str]:
    env, argv = _download_selection(job)
    name = argv[argv.index("-n") + 1]
    repo = argv[argv.index("--repo") + 1]
    destination = argv[argv.index("-D") + 1]
    return name, str(env["PROVENANCE_RUN_ID"]), str(env["GITHUB_REPOSITORY"]), destination


def _assert_distinct(a: dict, b: dict) -> None:
    left, right = _selector(a), _selector(b)
    # The same run and repository are allowed; the selected trusted payload must not be.
    if left == right:
        raise AssertionError("Mac and Web download the same trusted provenance selection")
    if left[1:] == right[1:]:
        if left[0] == right[0]:
            raise AssertionError("Mac and Web share artifact name, run input, repository, and path")


class PlatformProvenanceSelectionTests(unittest.TestCase):
    def test_real_workflow_selects_distinct_mac_and_web_provenance(self) -> None:
        document = _psych(WORKFLOW.read_text(encoding="utf-8"))
        jobs = _jobs(document)
        # This is intentionally RED on the current workflow: both jobs download the
        # same artifact into the same trusted-provenance.json path.
        _assert_distinct(jobs["mac-native"], jobs["web-journey"])

    def test_distinct_artifact_names_are_a_valid_selection_control(self) -> None:
        common = {
            "PROVENANCE_RUN_ID": "${{ inputs.provenance_run_id }}",
            "GITHUB_REPOSITORY": "${{ github.repository }}",
        }

        def job(name: str) -> dict:
            return {"steps": [{"run": f"gh run download \"$PROVENANCE_RUN_ID\" -n {name} -D \"$RUNNER_TEMP\" --repo \"$GITHUB_REPOSITORY\"", "env": common}]}

        _assert_distinct(job("mac-trusted-provenance.json"), job("web-trusted-provenance.json"))

    def test_same_artifact_and_inputs_are_rejected(self) -> None:
        env = {"PROVENANCE_RUN_ID": "${{ inputs.provenance_run_id }}", "GITHUB_REPOSITORY": "${{ github.repository }}"}
        job = {"steps": [{"run": 'gh run download "$PROVENANCE_RUN_ID" -n trusted-provenance.json -D "$RUNNER_TEMP" --repo "$GITHUB_REPOSITORY"', "env": env}]}
        with self.assertRaises(AssertionError):
            _assert_distinct(job, job)

    def test_validator_binds_platform_to_distinct_synthetic_receipts(self) -> None:
        source, tree = "a" * 40, "b" * 40
        contract = {"frozen_tests": {"tests/frozen.py": "1" * 64}, "runner_sha256": "2" * 64,
                    "binary_sha256": "3" * 64, "native_library_sha256": "4" * 64,
                    "archive_sha256": "5" * 64}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = {"artifact_contract": {**contract, "source_sha": source}}
            receipts = {}
            for platform in ("mac", "web"):
                data = {**contract, "source_sha": source, "tree": tree, "platform": platform,
                        "profile": "release", "native": True}
                path = root / f"{platform}.json"
                path.write_text(json.dumps(data), encoding="utf-8")
                receipts[platform] = (path, hashlib.sha256(path.read_bytes()).hexdigest())
                self.assertEqual(validate_trusted_provenance(path, receipts[platform][1], source, tree, platform, manifest)["platform"], platform)
            shared = root / "shared.json"
            shared.write_bytes(receipts["mac"][0].read_bytes())
            with self.assertRaises(ReceiptError):
                validate_trusted_provenance(shared, hashlib.sha256(shared.read_bytes()).hexdigest(), source, tree, "web", manifest)


if __name__ == "__main__":
    unittest.main()
