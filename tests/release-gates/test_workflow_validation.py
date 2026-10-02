"""Static release-workflow contract checks using Ruby's real Psych YAML parser."""
from __future__ import annotations

import json
import re
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from typing import Optional


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/release-gates.yml"
YAML_TO_JSON = (
    'require "yaml"; require "json"; '
    'puts JSON.generate(YAML.safe_load(STDIN.read, permitted_classes: [], aliases: false))'
)


class WorkflowYamlError(AssertionError):
    pass


def parse_yaml(source: str) -> dict:
    result = subprocess.run(
        ["ruby", "-e", YAML_TO_JSON], input=source, capture_output=True,
        text=True, check=False,
    )
    if result.returncode:
        raise WorkflowYamlError(result.stderr.strip())
    value = json.loads(result.stdout)
    if not isinstance(value, dict):
        raise WorkflowYamlError("workflow YAML root must be a mapping")
    return value


def workflow_on(document: dict) -> dict:
    # Psych uses YAML 1.1, where an unquoted `on` key becomes boolean true.
    value = document.get("on", document.get("true"))
    if not isinstance(value, dict):
        raise AssertionError("workflow must define an on mapping")
    return value


def expression_input(value: object) -> Optional[str]:
    match = re.fullmatch(r"\s*\$\{\{\s*inputs\.([A-Za-z0-9_-]+)\s*}}\s*", str(value))
    return match.group(1) if match else None


def checkout_source_input(value: object) -> Optional[str]:
    if expression_input(value) == "source_sha":
        return "source_sha"
    if re.fullmatch(
        r"\s*\$\{\{\s*inputs\.source_sha\s*\|\|\s*github\.sha\s*}}\s*",
        str(value),
    ):
        return "source_sha"
    return None


def shell_variable(command: str, option: str) -> Optional[str]:
    match = re.search(rf"{re.escape(option)}\s+['\"]?\$([A-Za-z_][A-Za-z0-9_]*)", command)
    return match.group(1) if match else None


def step_payload(step: dict) -> str:
    return " ".join(str(step.get(key, "")) for key in ("name", "run", "uses", "with"))


def explicit_repository(command: str) -> bool:
    return re.search(r"(?:--repo|-R)\s+[^\s]+", command) is not None


def valid_control_workflow(source_ref: str = "${{ inputs.source_sha }}") -> str:
    return """name: control
on:
  workflow_dispatch:
    inputs:
      source_sha:
        required: true
        type: string
      artifact_run_id:
        required: true
        type: string
      provenance_run_id:
        required: true
        type: string
      mac_receipt_digest:
        required: true
        type: string
      web_receipt_digest:
        required: true
        type: string
permissions:
  contents: read
env:
  CARGO_TERM_COLOR: always
jobs:
  mac-native:
    runs-on: macos-14
    timeout-minutes: 120
    steps:
      - uses: actions/checkout@1111111111111111111111111111111111111111
        with:
          ref: SOURCE_REF
      - name: Download build artifact
        run: gh run download "$ARTIFACT_RUN_ID" -n macos-arm64-release -D "$RUNNER_TEMP/artifact" --repo "$GITHUB_REPOSITORY"
        env:
          ARTIFACT_RUN_ID: ${{ inputs.artifact_run_id }}
          GITHUB_REPOSITORY: owner/repository
      - name: Download trusted provenance
        run: gh run download "$PROVENANCE_RUN_ID" -n trusted-provenance.json -D "$RUNNER_TEMP" --repo "$GITHUB_REPOSITORY"
        env:
          PROVENANCE_RUN_ID: ${{ inputs.provenance_run_id }}
          GITHUB_REPOSITORY: owner/repository
      - name: Run gate
        run: /usr/bin/python3 release/run_release_gates.py --trusted-provenance "$TRUSTED_PROVENANCE" --expected-receipt-digest "$TRUSTED_RECEIPT_DIGEST" --platform mac --run
        env:
          TRUSTED_PROVENANCE: ${{ runner.temp }}/trusted-provenance.json
          TRUSTED_RECEIPT_DIGEST: ${{ inputs.mac_receipt_digest }}
  web-journey:
    runs-on: ubuntu-latest
    timeout-minutes: 60
    steps:
      - uses: actions/checkout@2222222222222222222222222222222222222222
        with:
          ref: SOURCE_REF
      - name: Download build artifact
        run: gh run download "$ARTIFACT_RUN_ID" -n linux-x64-release -D "$RUNNER_TEMP/artifact" --repo "$GITHUB_REPOSITORY"
        env:
          ARTIFACT_RUN_ID: ${{ inputs.artifact_run_id }}
          GITHUB_REPOSITORY: owner/repository
      - name: Download trusted provenance
        run: gh run download "$PROVENANCE_RUN_ID" -n trusted-provenance.json -D "$RUNNER_TEMP" --repo "$GITHUB_REPOSITORY"
        env:
          PROVENANCE_RUN_ID: ${{ inputs.provenance_run_id }}
          GITHUB_REPOSITORY: owner/repository
      - name: Run gate
        run: /usr/bin/python3 release/run_release_gates.py --trusted-provenance "$TRUSTED_PROVENANCE" --expected-receipt-digest "$TRUSTED_RECEIPT_DIGEST" --platform web --run
        env:
          TRUSTED_PROVENANCE: ${{ runner.temp }}/trusted-provenance.json
          TRUSTED_RECEIPT_DIGEST: ${{ inputs.web_receipt_digest }}
""".replace("SOURCE_REF", source_ref)


class ReleaseWorkflowValidationTests(unittest.TestCase):
    def load_workflow(self) -> dict:
        return parse_yaml(WORKFLOW.read_text(encoding="utf-8"))

    def test_yaml_parser_accepts_valid_control_and_rejects_malformed_yaml(self) -> None:
        valid = """name: control
on:
  workflow_dispatch:
    inputs:
      source_sha:
        required: true
        type: string
jobs:
  contract:
    runs-on: ubuntu-latest
    steps:
      - run: echo valid
"""
        parsed = parse_yaml(valid)
        self.assertEqual(parsed["name"], "control")
        self.assertIn("workflow_dispatch", workflow_on(parsed))
        with self.assertRaises(WorkflowYamlError):
            parse_yaml("jobs:\n  contract:\n    steps:\n      - run: echo bad\n       env:\n         BAD: value\n")

    def test_real_workflow_parses_as_yaml_and_has_github_actions_shape(self) -> None:
        document = self.load_workflow()
        self.assertIsInstance(document.get("name"), str)
        triggers = workflow_on(document)
        self.assertIn("workflow_dispatch", triggers)
        dispatch = triggers["workflow_dispatch"]
        self.assertIsInstance(dispatch, dict)
        self.assertIsInstance(dispatch.get("inputs"), dict)
        self.assertTrue(set(document).issubset({"name", "on", "true", "permissions", "env", "jobs"}))
        self.assertIsInstance(document.get("permissions"), dict)
        self.assertIsInstance(document.get("env"), dict)
        jobs = document.get("jobs")
        self.assertIsInstance(jobs, dict)
        job_keys = {"name", "if", "runs-on", "timeout-minutes", "steps", "permissions", "env", "needs", "concurrency"}
        step_keys = {
            "id", "if", "name", "uses", "run", "with", "env",
            "continue-on-error", "timeout-minutes", "shell", "working-directory",
        }
        for name, job in jobs.items():
            with self.subTest(job=name):
                self.assertIsInstance(job, dict)
                self.assertTrue(set(job).issubset(job_keys))
                self.assertIsInstance(job.get("runs-on"), (str, list, dict))
                self.assertIsInstance(job.get("steps"), list)
                self.assertTrue(job["steps"])
                self.assertIsInstance(job.get("timeout-minutes"), int)
                for step in job["steps"]:
                    self.assertIsInstance(step, dict)
                    self.assertTrue(set(step).issubset(step_keys))
                    self.assertEqual(sum(key in step for key in ("run", "uses")), 1)
                    if "uses" in step:
                        self.assertIsInstance(step["uses"], str)
                        self.assertRegex(step["uses"], r"@[0-9a-f]{40}$")
                    if "with" in step:
                        self.assertIsInstance(step["with"], dict)
                    if "env" in step:
                        self.assertIsInstance(step["env"], dict)

    def test_valid_full_contract_control_satisfies_all_authority_and_order_checks(self) -> None:
        control = valid_control_workflow()
        parsed = parse_yaml(control)
        self.assertEqual(parsed["name"], "control")
        controls = (
            control,
            valid_control_workflow("${{ inputs.source_sha || github.sha }}"),
        )
        for fixture in controls:
            with self.subTest(source_ref=fixture.split("ref: ", 1)[1].splitlines()[0]):
                with tempfile.TemporaryDirectory() as temp_dir:
                    workflow_path = Path(temp_dir) / "release-gates.yml"
                    workflow_path.write_text(fixture, encoding="utf-8")
                    with patch(__name__ + ".WORKFLOW", workflow_path):
                        parser = ReleaseWorkflowValidationTests()
                        parser.test_real_workflow_parses_as_yaml_and_has_github_actions_shape()
                        parser.test_dispatch_requires_build_and_independent_provenance_run_ids()
                        parser.test_platform_digest_inputs_are_distinct_required_controller_values()
                        parser.test_mac_and_web_download_build_artifacts_from_explicit_repository()
                        parser.test_each_runtime_gate_downloads_external_provenance_before_run()
        for invalid_ref in (
            "refs/heads/main",
            "${{ github.sha }}",
            "${{ inputs.other_sha }}",
            "${{ inputs.source_sha || github.ref }}",
            "${{ inputs.source_sha || 'release' }}",
            "prefix-${{ inputs.source_sha }}",
        ):
            with self.subTest(invalid_checkout_ref=invalid_ref):
                with tempfile.TemporaryDirectory() as temp_dir:
                    workflow_path = Path(temp_dir) / "release-gates.yml"
                    workflow_path.write_text(valid_control_workflow(invalid_ref), encoding="utf-8")
                    with patch(__name__ + ".WORKFLOW", workflow_path):
                        with self.assertRaises(AssertionError):
                            ReleaseWorkflowValidationTests().test_each_runtime_gate_downloads_external_provenance_before_run()

    def test_dispatch_requires_build_and_independent_provenance_run_ids(self) -> None:
        document = self.load_workflow()
        inputs = workflow_on(document)["workflow_dispatch"]["inputs"]
        self.assertIs(inputs.get("source_sha", {}).get("required"), True)
        artifact_ids = [
            name for name in inputs
            if "artifact" in name.lower() and "run" in name.lower() and "id" in name.lower()
        ]
        provenance_ids = [
            name for name in inputs
            if "provenance" in name.lower() and "run" in name.lower() and "id" in name.lower()
        ]
        self.assertEqual(len(artifact_ids), 1, "require one controller-selected build artifact run")
        self.assertEqual(len(provenance_ids), 1, "require a separate trusted provenance run input")
        self.assertNotEqual(artifact_ids[0], provenance_ids[0])
        self.assertIs(inputs[artifact_ids[0]].get("required"), True)
        self.assertIs(inputs[provenance_ids[0]].get("required"), True)

    def test_platform_digest_inputs_are_distinct_required_controller_values(self) -> None:
        document = self.load_workflow()
        inputs = workflow_on(document)["workflow_dispatch"]["inputs"]
        selected = {}
        for platform in ("mac", "web"):
            matches = [
                name for name in inputs
                if "digest" in name.lower() and platform in name.lower()
            ]
            self.assertEqual(len(matches), 1, f"require one {platform} controller digest input")
            selected[platform] = matches[0]
            self.assertIs(inputs[matches[0]].get("required"), True)
        self.assertNotEqual(selected["mac"], selected["web"])
        digest_inputs = {name for name in inputs if "digest" in name.lower()}
        self.assertEqual(digest_inputs, set(selected.values()), "no generic digest input may authorize both platforms")

    def test_mac_and_web_download_build_artifacts_from_explicit_repository(self) -> None:
        document = self.load_workflow()
        inputs = workflow_on(document)["workflow_dispatch"]["inputs"]
        artifact_id = next(
            name for name in inputs
            if "artifact" in name.lower() and "run" in name.lower() and "id" in name.lower()
        )
        self.assertIs(inputs[artifact_id].get("required"), True)
        for platform, job_name, artifact_name in (
            ("mac", "mac-native", "macos-arm64-release"),
            ("web", "web-journey", "linux-x64-release"),
        ):
            job = document["jobs"][job_name]
            downloads = [
                step for step in job["steps"]
                if "gh run download" in str(step.get("run", ""))
                or "actions/download-artifact" in str(step.get("uses", ""))
            ]
            build_downloads = [
                step for step in downloads
                if "artifact" in step_payload(step).lower()
                and "provenance" not in step_payload(step).lower()
            ]
            self.assertTrue(build_downloads, f"{platform} build artifact must be downloaded")
            step = build_downloads[0]
            self.assertIn(artifact_name, step_payload(step))
            run = str(step.get("run", ""))
            if "gh run download" in run:
                self.assertTrue(explicit_repository(run), "artifact download must name repository explicitly")
                self.assertRegex(run, r"(?:-n|--name)\s+['\"]?[a-z0-9-]+['\"]?")
                self.assertRegex(run, r"(?:-D|--dir)\s+['\"]?\$RUNNER_TEMP/artifact['\"]?(?:\s|$)")
                variable = shell_variable(run, "gh run download")
                self.assertIsNotNone(variable)
                value = (step.get("env") or {}).get(variable)
                repository_var = shell_variable(run, "--repo") or shell_variable(run, "-R")
                self.assertIsNotNone(repository_var)
                repository_value = (step.get("env") or {}).get(repository_var)
                self.assertTrue(repository_value, "explicit repository must be bound via step environment")
                self.assertEqual(expression_input(value), artifact_id)
            else:
                with_values = step.get("with") or {}
                self.assertTrue(with_values.get("repository"), "cross-run artifact download needs repository")
                self.assertEqual(expression_input(with_values.get("run-id")), artifact_id)

    def test_each_runtime_gate_downloads_external_provenance_before_run(self) -> None:
        document = self.load_workflow()
        inputs = workflow_on(document)["workflow_dispatch"]["inputs"]
        provenance_id = next(
            name for name in inputs
            if "provenance" in name.lower() and "run" in name.lower() and "id" in name.lower()
        )
        for platform, job_name in (("mac", "mac-native"), ("web", "web-journey")):
            steps = document["jobs"][job_name]["steps"]
            self.assertEqual(
                checkout_source_input(steps[0].get("with", {}).get("ref")), "source_sha",
                f"{platform} gates must check out exact dispatched SHA",
            )
            run_indexes = [
                index for index, step in enumerate(steps)
                if "--run" in str(step.get("run", "")).split()
            ]
            self.assertEqual(len(run_indexes), 1, f"{platform} must have one runtime gate step")
            run_index = run_indexes[0]
            run_step = steps[run_index]
            run = str(run_step.get("run", ""))
            provenance_var = shell_variable(run, "--trusted-provenance")
            digest_var = shell_variable(run, "--expected-receipt-digest")
            self.assertIsNotNone(provenance_var)
            self.assertIsNotNone(digest_var)
            self.assertNotEqual(provenance_var, digest_var)
            env = run_step.get("env") or {}
            self.assertEqual(env.get(provenance_var), "${{ runner.temp }}/trusted-provenance.json")
            digest_input = expression_input(env.get(digest_var))
            self.assertIsNotNone(digest_input, "digest must come directly from controller dispatch input")
            self.assertIn(platform, digest_input.lower())
            self.assertEqual(
                shell_variable(run, "--platform"), None,
                "platform selection must be a literal trusted gate value, not shell input",
            )
            self.assertIn(f"--platform {platform}", run)

            prior = steps[:run_index]
            build_downloads = [
                step for step in prior
                if ("gh run download" in str(step.get("run", ""))
                    or "actions/download-artifact" in str(step.get("uses", "")))
                and "artifact" in step_payload(step).lower()
                and "provenance" not in step_payload(step).lower()
            ]
            self.assertTrue(build_downloads, f"{platform} build artifact download must precede --run")
            provenance_downloads = [
                step for step in prior
                if ("gh run download" in str(step.get("run", ""))
                    or "actions/download-artifact" in str(step.get("uses", "")))
                and "provenance" in step_payload(step).lower()
            ]
            self.assertTrue(provenance_downloads, f"{platform} provenance must be fetched before --run")
            self.assertLess(
                steps.index(build_downloads[0]), steps.index(provenance_downloads[0]),
                f"{platform} build artifact must precede provenance download",
            )
            download = provenance_downloads[0]
            self.assertIn("trusted-provenance.json", step_payload(download))
            if "gh run download" in str(download.get("run", "")):
                download_variable = shell_variable(str(download.get("run", "")), "gh run download")
                self.assertIsNotNone(download_variable)
                self.assertNotEqual(download_variable, "ARTIFACT_RUN_ID", "provenance run must be independent from build run")
            download_run = str(download.get("run", ""))
            if "gh run download" in download_run:
                self.assertTrue(explicit_repository(download_run), "provenance download must name repository explicitly")
                self.assertRegex(download_run, r"(?:-D|--dir)\s+['\"]?\$RUNNER_TEMP['\"]?(?:\s|$)")
                variable = shell_variable(download_run, "gh run download")
                self.assertIsNotNone(variable)
                env = download.get("env") or {}
                self.assertEqual(expression_input(env.get(variable)), provenance_id)
                repository_var = shell_variable(download_run, "--repo") or shell_variable(download_run, "-R")
                self.assertIsNotNone(repository_var)
                self.assertTrue(env.get(repository_var), "explicit provenance repository must be environment-bound")
            else:
                values = download.get("with") or {}
                self.assertTrue(values.get("repository"), "cross-run provenance download needs repository")
                self.assertEqual(expression_input(values.get("run-id")), provenance_id)
                self.assertEqual(values.get("path"), "${{ runner.temp }}")
                pattern = values.get("pattern")
                self.assertIsInstance(pattern, str)
                self.assertIn("trusted-provenance.json", pattern)


if __name__ == "__main__":
    unittest.main()
