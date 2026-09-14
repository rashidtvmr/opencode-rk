#!/usr/bin/env python3
"""Canonical fail-closed repository validation entrypoint.

CI and autonomous controller verification use this command so repository
protection policy, backlog exhaustion, DISC-003 reconciliation/manifest integrity,
and plan validation cannot drift into separate optional paths. Bootstrap unit
tests remain a distinct CI step so this script can itself be imported and tested
without recursion.
"""
from __future__ import annotations

import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]

# Keep these independent even though validate_plan also validates the exhaustion
# ledger. The duplication is deliberate defense in depth against accidentally
# removing a check from one entrypoint.
REQUIRED_CHECKS = (
    ("ruleset import", ("tools/render_ruleset_import.py", "--check")),
    ("ruleset readback verifier fixtures", ("tools/verify_ruleset_readback.py", "--self-test")),
    ("repository protection", ("tools/validate_protection_policy.py",)),
    ("backlog exhaustion", ("tools/validate_backlog_exhaustion.py",)),
    ("DISC-003 reconciliation", ("tools/reconcile_surfaces.py", "--check-manifest")),
    ("plan", ("tools/validate_plan.py",)),
)

REQUIRED_WORKFLOW_LINES = (
    "name: ci",
    "  pull_request:",
    "    name: planning",
    "        run: /usr/bin/python3 tools/validate_repository.py",
    "        run: /usr/bin/python3 -m unittest discover -s tests/bootstrap -p 'test_*.py' -v",
)

EXPECTED_WORKFLOW_PREFIX = (
    "name: ci",
    "",
    "on:",
    "  push:",
    "    branches: [main]",
    "  pull_request:",
    "  workflow_dispatch:",
    "",
    "permissions:",
    "  contents: read",
    "",
    "env:",
    "  CARGO_TERM_COLOR: always",
    "",
    "jobs:",
)

EXPECTED_PLANNING_BLOCK = (
    "  planning:",
    "    name: planning",
    "    runs-on: ubuntu-latest",
    "    timeout-minutes: 10",
    "    steps:",
    "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0",
    "        with:",
    "          persist-credentials: false",
    "      - name: Validate repository contracts",
    "        run: /usr/bin/python3 tools/validate_repository.py",
    "      - name: Bootstrap controller tests",
    "        run: /usr/bin/python3 -m unittest discover -s tests/bootstrap -p 'test_*.py' -v",
    "",
)


def workflow_contract_errors(root: pathlib.Path = ROOT) -> list[str]:
    path = root / ".github/workflows/ci.yml"
    if not path.is_file():
        return ["missing required CI workflow: .github/workflows/ci.yml"]
    text = path.read_text(encoding="utf-8")
    lines = text.splitlines()
    errors = [f"CI workflow missing exact enforcement line: {line.strip()}" for line in REQUIRED_WORKFLOW_LINES if line not in lines]
    if tuple(lines[: len(EXPECTED_WORKFLOW_PREFIX)]) != EXPECTED_WORKFLOW_PREFIX:
        errors.append("CI workflow trigger/permission prefix drifted from the canonical fail-closed form")

    # The planning job is the stable required-check boundary. Do not allow its
    # repository/bootstrap enforcement to be conditionally skipped or softened.
    try:
        start = lines.index("  planning:")
    except ValueError:
        return [*errors, "CI workflow is missing the planning job"]
    end = next((index for index in range(start + 1, len(lines)) if lines[index].startswith("  ") and not lines[index].startswith("    ") and lines[index].endswith(":")), len(lines))
    planning = lines[start:end]
    if tuple(planning) != EXPECTED_PLANNING_BLOCK:
        errors.append("CI planning job drifted from the canonical fail-closed block")
    for forbidden in ("if:", "continue-on-error:", "needs:"):
        if any(line.strip().startswith(forbidden) for line in planning):
            errors.append(f"CI planning job/steps must not use {forbidden[:-1]}")
    return errors


def main() -> int:
    workflow_errors = workflow_contract_errors(ROOT)
    if workflow_errors:
        print(f"validate_repository: {len(workflow_errors)} CI enforcement error(s)")
        for error in workflow_errors:
            print(f"  - {error}")
        return 1
    for label, args in REQUIRED_CHECKS:
        command = [sys.executable, *args]
        result = subprocess.run(command, cwd=ROOT, check=False)
        if result.returncode != 0:
            print(f"validate_repository: FAIL {label} exit={result.returncode}")
            return result.returncode
    print("validate_repository: OK  ruleset-import + ruleset-readback + protection + exhaustion + DISC-003 + plan")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
