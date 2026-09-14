#!/usr/bin/env python3
"""Canonical fail-closed repository validation entrypoint.

CI and autonomous controller verification use this command so backlog exhaustion,
DISC-003 reconciliation/manifest integrity, and plan validation cannot drift into
separate optional paths. Bootstrap unit tests remain a distinct CI step so this
script can itself be imported and tested without recursion.
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
    ("backlog exhaustion", ("tools/validate_backlog_exhaustion.py",)),
    ("DISC-003 reconciliation", ("tools/reconcile_surfaces.py", "--check-manifest")),
    ("plan", ("tools/validate_plan.py",)),
)

REQUIRED_WORKFLOW_LINES = (
    "  pull_request:",
    "        run: /usr/bin/python3 tools/validate_repository.py",
    "        run: /usr/bin/python3 -m unittest discover -s tests/bootstrap -p 'test_*.py' -v",
)


def workflow_contract_errors(root: pathlib.Path = ROOT) -> list[str]:
    path = root / ".github/workflows/ci.yml"
    if not path.is_file():
        return ["missing required CI workflow: .github/workflows/ci.yml"]
    text = path.read_text(encoding="utf-8")
    lines = text.splitlines()
    errors = [f"CI workflow missing exact enforcement line: {line.strip()}" for line in REQUIRED_WORKFLOW_LINES if line not in lines]

    # The planning job is the stable required-check boundary. Do not allow its
    # repository/bootstrap enforcement to be conditionally skipped or softened.
    try:
        start = lines.index("  planning:")
    except ValueError:
        return [*errors, "CI workflow is missing the planning job"]
    end = next((index for index in range(start + 1, len(lines)) if lines[index].startswith("  ") and not lines[index].startswith("    ") and lines[index].endswith(":")), len(lines))
    planning = lines[start:end]
    for line in planning:
        stripped = line.strip()
        if stripped.startswith("if:"):
            errors.append("CI planning job/steps must not be conditionally skipped")
        if stripped == "continue-on-error: true":
            errors.append("CI planning job/steps must not use continue-on-error")
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
    print("validate_repository: OK  exhaustion + DISC-003 + plan")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
