"""Independent REL-002 verifier checks.

The release validator and fixtures predate this verifier lane. These tests keep
the observable receipt contract frozen while exposing the still-missing CI
caller as a RED check.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import shlex
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


_VALID_RELEASE_WORKFLOW = """\
name: ci
on:
  pull_request:
permissions:
  contents: read
jobs:
  planning:
    name: planning
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@pinned
      - name: Release TDD assurance
        run: |
          set -euo pipefail
          revision="$(git rev-parse HEAD)"
          /usr/bin/python3 tools/check_release_tdd.py \
            --revision "$revision" \
            --manifest fixtures/release-tdd/frozen.json \
            --receipts fixtures/release-tdd/receipts \
            --gates fixtures/release-tdd/gates \
            --out report.json
"""


def _planning_steps(workflow: str) -> list[dict[str, object]]:
    """Extract planning-job steps without treating YAML text as executable."""
    lines = workflow.splitlines()
    try:
        start = lines.index("  planning:")
    except ValueError as exc:
        raise AssertionError("CI workflow has no planning job") from exc
    end = next(
        (
            index
            for index in range(start + 1, len(lines))
            if re.match(r"^  [A-Za-z0-9_-]+:$", lines[index])
        ),
        len(lines),
    )
    planning = lines[start:end]
    steps: list[dict[str, object]] = []
    starts = [
        index
        for index, line in enumerate(planning)
        if re.match(r"^      - ", line)
    ]
    for relative, step_start in enumerate(starts):
        step_end = starts[relative + 1] if relative + 1 < len(starts) else len(planning)
        body = planning[step_start:step_end]
        first = body[0].strip()[2:].strip()
        name = ""
        if first.startswith("name:"):
            name = first.split(":", 1)[1].strip()
        run: str | None = None
        run_index = next(
            (
                index
                for index, line in enumerate(body)
                if line.startswith("        run:")
            ),
            None,
        )
        if run_index is not None:
            value = body[run_index].split(":", 1)[1].strip()
            if value == "|":
                script_lines: list[str] = []
                for line in body[run_index + 1 :]:
                    if line and len(line) - len(line.lstrip(" ")) < 10:
                        break
                    script_lines.append(line[10:] if line else "")
                run = "\n".join(script_lines)
            elif value and value not in {">", "|-", ">-"}:
                run = value
        continue_on_error: str | None = None
        for line in body:
            if line.startswith("        continue-on-error:"):
                continue_on_error = line.split(":", 1)[1].strip().lower()
        steps.append(
            {
                "name": name,
                "run": run,
                "continue-on-error": continue_on_error,
                "body": body,
            }
        )
    return steps


def _shell_commands(script: str) -> list[tuple[list[str], str | None, str | None]]:
    """Return shell command argv plus adjacent operators, ignoring comments."""
    script = script.replace("\\\n", " ").replace("\n", " ; ")
    lexer = shlex.shlex(script, posix=True, punctuation_chars=";&|")
    lexer.whitespace_split = True
    lexer.commenters = "#"
    tokens = list(lexer)
    commands: list[list[str]] = []
    operators: list[str] = []
    current: list[str] = []
    for token in tokens:
        if token in {";", "&&", "||", "&", "|"}:
            if current:
                commands.append(current)
                current = []
            operators.append(token)
        else:
            current.append(token)
    if current:
        commands.append(current)
    result: list[tuple[list[str], str | None, str | None]] = []
    for index, command in enumerate(commands):
        before = operators[index - 1] if index else None
        after = operators[index] if index < len(operators) else None
        result.append((command, before, after))
    return result


def _release_contract(workflow: str) -> dict[str, object]:
    """Validate and return one executable release-validator step."""
    lines = workflow.splitlines()
    try:
        planning_start = lines.index("  planning:")
    except ValueError as exc:
        raise AssertionError("CI workflow has no planning job") from exc
    planning_end = next(
        (
            index
            for index in range(planning_start + 1, len(lines))
            if re.match(r"^  [A-Za-z0-9_-]+:$", lines[index])
        ),
        len(lines),
    )
    planning = lines[planning_start:planning_end]
    timeout_lines = [
        line.split(":", 1)[1].strip()
        for line in planning
        if line.startswith("    timeout-minutes:")
    ]
    self_assert = timeout_lines and timeout_lines[0].isdigit() and int(timeout_lines[0]) > 0
    if not self_assert:
        raise AssertionError("planning job must have a positive timeout-minutes bound")
    if any(line.strip().startswith("continue-on-error: true") for line in planning):
        raise AssertionError("planning job cannot ignore release-validator failure")

    candidates: list[dict[str, object]] = []
    for step in _planning_steps(workflow):
        script = step["run"]
        if not isinstance(script, str):
            continue
        commands = _shell_commands(script)
        for command, before, after in commands:
            executable_index = next(
                (
                    index
                    for index, token in enumerate(command)
                    if token == "tools/check_release_tdd.py"
                ),
                None,
            )
            if executable_index is None:
                continue
            prefix = command[:executable_index]
            prefix = [token for token in prefix if "=" not in token or token.startswith("--")]
            if len(prefix) < 1 or Path(prefix[-1]).name != "python3":
                continue
            candidates.append(
                {
                    "step": step,
                    "script": script,
                    "command": command,
                    "before": before,
                    "after": after,
                }
            )
    if len(candidates) != 1:
        raise AssertionError(
            f"planning job must contain exactly one executable release-validator invocation; found {len(candidates)}"
        )
    candidate = candidates[0]
    step = candidate["step"]
    assert isinstance(step, dict)
    if step.get("continue-on-error") == "true":
        raise AssertionError("release-validator step cannot use continue-on-error")
    script = candidate["script"]
    assert isinstance(script, str)
    commands = _shell_commands(script)
    set_commands = [
        command for command, _, _ in commands if command and command[0] == "set"
    ]
    has_errexit = any(
        any(
            token in {"-e", "-eu", "-euo", "errexit"}
            or (token.startswith("-") and "e" in token[1:])
            for token in command[1:]
        )
        for command in set_commands
    )
    if not has_errexit:
        raise AssertionError("release-validator step must enable shell errexit")
    if not any("pipefail" in token for command, _, _ in commands for token in command):
        raise AssertionError("release-validator step must enable pipefail")
    if candidate["before"] == "||" or candidate["after"] == "||":
        raise AssertionError("release-validator failure must not be swallowed")

    command = candidate["command"]
    assert isinstance(command, list)
    required = {"--revision", "--manifest", "--receipts", "--gates", "--out"}
    positions = {flag: [index for index, token in enumerate(command) if token == flag] for flag in required}
    if any(len(indexes) != 1 for indexes in positions.values()):
        raise AssertionError("release-validator must receive each exact required flag once")
    values: dict[str, str] = {}
    for flag, indexes in positions.items():
        index = indexes[0]
        if index + 1 >= len(command) or not command[index + 1] or command[index + 1].startswith("-"):
            raise AssertionError(f"release-validator flag has no value: {flag}")
        values[flag] = command[index + 1]

    dynamic_assignments = {
        match.group(1)
        for command_tokens, _, _ in commands
        for token in command_tokens
        if (match := re.fullmatch(r"([A-Za-z_][A-Za-z0-9_]*)=\$\(git rev-parse HEAD\)", token))
    }
    revision_value = values["--revision"]
    dynamic_revision = revision_value in {
        f"${{{name}}}" for name in dynamic_assignments
    } | {f"${name}" for name in dynamic_assignments}
    dynamic_revision = dynamic_revision or bool(
        re.fullmatch(r"\$\(git rev-parse HEAD\)", revision_value)
    )
    if not dynamic_revision or VALID_REVISION in script:
        raise AssertionError("--revision must derive from git rev-parse HEAD, not a historical SHA")
    return {**candidate, "values": values}


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
        """RED until planning contains an executable release gate."""
        workflow = (ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        contract = _release_contract(workflow)
        self.assertEqual(
            contract["values"],
            {
                "--revision": "$revision",
                "--manifest": "fixtures/release-tdd/frozen.json",
                "--receipts": "fixtures/release-tdd/receipts",
                "--gates": "fixtures/release-tdd/gates",
                "--out": "report.json",
            },
        )

    def test_ci_release_contract_rejects_dead_strings_and_failure_bypasses(self) -> None:
        """Comments, YAML scalars, swallowed exits, and incomplete argv fail closed."""
        _release_contract(_VALID_RELEASE_WORKFLOW)
        cases = {
            "comment": _VALID_RELEASE_WORKFLOW.replace(
                "/usr/bin/python3 tools/check_release_tdd.py",
                "# /usr/bin/python3 tools/check_release_tdd.py",
            ),
            "dead scalar": _VALID_RELEASE_WORKFLOW.replace(
                "/usr/bin/python3 tools/check_release_tdd.py",
                "echo '/usr/bin/python3 tools/check_release_tdd.py'",
            ),
            "swallowed exit": _VALID_RELEASE_WORKFLOW.replace(
                "--out report.json", "--out report.json || true"
            ),
            "missing gate": _VALID_RELEASE_WORKFLOW.replace(
                "--gates fixtures/release-tdd/gates", "--gates"
            ),
            "ignored step": _VALID_RELEASE_WORKFLOW.replace(
                "        run: |\n", "        continue-on-error: true\n        run: |\n"
            ),
            "historical revision": _VALID_RELEASE_WORKFLOW.replace(
                'revision="$(git rev-parse HEAD)"',
                f'revision="{VALID_REVISION}"',
            ),
        }
        for label, workflow in cases.items():
            with self.subTest(label=label):
                with self.assertRaises(AssertionError):
                    _release_contract(workflow)

    def test_ci_release_command_uses_checked_out_revision_and_exact_argv(self) -> None:
        """Execute the extracted bounded step in a disposable git repository."""
        contract = _release_contract(_VALID_RELEASE_WORKFLOW)
        script = contract["script"]
        self.assertIsInstance(script, str)
        with tempfile.TemporaryDirectory(prefix="rel002-ci-command-") as tmp:
            repo = Path(tmp)
            (repo / "tools").mkdir()
            args_path = repo / "argv.json"
            (repo / "tools" / "check_release_tdd.py").write_text(
                "import json, os, sys\n"
                "path = os.environ['REL002_ARGV_PATH']\n"
                "with open(path, 'w', encoding='utf-8') as handle:\n"
                "    json.dump(sys.argv[1:], handle)\n",
                encoding="utf-8",
            )
            (repo / "seed").write_text("fixture\n", encoding="utf-8")
            subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
            subprocess.run(["git", "config", "user.email", "ci@example.invalid"], cwd=repo, check=True)
            subprocess.run(["git", "config", "user.name", "CI fixture"], cwd=repo, check=True)
            subprocess.run(["git", "add", "seed", "tools/check_release_tdd.py"], cwd=repo, check=True)
            subprocess.run(["git", "commit", "-qm", "fixture"], cwd=repo, check=True)
            expected_revision = subprocess.check_output(
                ["git", "rev-parse", "HEAD"], cwd=repo, text=True
            ).strip()
            environment = dict(os.environ)
            environment["REL002_ARGV_PATH"] = str(args_path)
            completed = subprocess.run(
                ["bash", "-euo", "pipefail", "-c", script],
                cwd=repo,
                env=environment,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertEqual(
                json.loads(args_path.read_text(encoding="utf-8")),
                [
                    "--revision",
                    expected_revision,
                    "--manifest",
                    "fixtures/release-tdd/frozen.json",
                    "--receipts",
                    "fixtures/release-tdd/receipts",
                    "--gates",
                    "fixtures/release-tdd/gates",
                    "--out",
                    "report.json",
                ],
            )
            (repo / "tools" / "check_release_tdd.py").write_text(
                "raise SystemExit(23)\n", encoding="utf-8"
            )
            failed = subprocess.run(
                ["bash", "-euo", "pipefail", "-c", script],
                cwd=repo,
                env=environment,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(failed.returncode, 23)

    def test_malformed_receipt_is_tool_error_without_report(self) -> None:
        """Malformed release evidence is a bounded tool error, not assurance pass."""
        with tempfile.TemporaryDirectory(prefix="rel002-malformed-") as tmp:
            candidate = Path(tmp) / "candidate"
            shutil.copytree(FIXTURES / "fail-malformed", candidate)
            output = Path(tmp) / "report.json"
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
            self.assertEqual(completed.returncode, 1)
            self.assertIn(b"malformed receipt", completed.stderr)
            self.assertFalse(output.exists())

    def test_timeout_is_tool_error_without_report(self) -> None:
        """A deadline expiry blocks assurance and does not emit a partial report."""
        with tempfile.TemporaryDirectory(prefix="rel002-timeout-") as tmp:
            candidate = Path(tmp) / "candidate"
            shutil.copytree(FIXTURES, candidate)
            output = Path(tmp) / "report.json"
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
                    "0.000001",
                ],
                cwd=Path(tmp),
                check=False,
                capture_output=True,
            )
            self.assertEqual(completed.returncode, 1)
            self.assertIn(b"timeout", completed.stderr)
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
