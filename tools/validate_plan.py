#!/usr/bin/env python3
"""Structural validator for the Lean Harness plan, requirements and config.

Referenced by README.md step 3. Fails closed on:
  - structurally invalid ralph.json / user-requirements.json
  - stories that collide on a proposed module path (feature-ledger.json)
  - missing mandatory config files
  - requirement ids that reference unknown tasks and vice versa
  - duplicate test-obligation ids

Exit code 0 only when every check passes. Stdlib only, no network.
"""
from __future__ import annotations

import json
import pathlib
import sys
from collections import Counter

ROOT = pathlib.Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tools.plan_model import load_plan  # noqa: E402

REQUIRED_FILES = [
    "PLAN.md",
    "AGENTS.md",
    "ralph.json",
    "requirements/user-requirements.json",
    "config/resource-targets.json",
    "config/controller.settings.json",
    "sources/upstream.lock.json",
]


def _require_files() -> list[str]:
    errors: list[str] = []
    for rel in REQUIRED_FILES:
        if not (ROOT / rel).is_file():
            errors.append(f"missing required file: {rel}")
    return errors


def _check_obligations(root: pathlib.Path) -> list[str]:
    errors: list[str] = []
    ralph = json.loads((root / "ralph.json").read_text(encoding="utf-8"))
    seen: Counter[str] = Counter()
    for story in ralph.get("userStories", []):
        tid = story.get("id")
        obligations = story.get("testObligations", [])
        if len(set(obligations)) != len(obligations):
            errors.append(f"{tid}: duplicate test obligation ids")
        for obligation in obligations:
            if not isinstance(obligation, str) or not obligation.startswith(f"{tid}-T"):
                errors.append(f"{tid}: malformed obligation {obligation!r}")
        seen.update(obligations)
    for obligation, count in seen.items():
        if count > 1:
            errors.append(f"obligation {obligation} appears {count} times across stories")
    return errors


def _check_ledger(root: pathlib.Path) -> list[str]:
    """If feature-ledger.json exists, proposed module paths must not collide."""
    errors: list[str] = []
    ledger_path = root / "feature-ledger.json"
    if not ledger_path.is_file():
        return errors
    ledger = json.loads(ledger_path.read_text(encoding="utf-8"))
    owners: Counter[str] = Counter()
    for entry in ledger.get("entries", []):
        path = entry.get("suggestedModule")
        if path:
            owners[path] += 1
    for path, count in owners.items():
        if count > 1:
            errors.append(f"module path claimed by {count} slices: {path}")
    return errors


def _check_requirements(root: pathlib.Path) -> list[str]:
    errors: list[str] = []
    reqs = json.loads((root / "requirements/user-requirements.json").read_text(encoding="utf-8"))
    ralph = json.loads((root / "ralph.json").read_text(encoding="utf-8"))
    story_ids = {s["id"] for s in ralph.get("userStories", [])}
    for req in reqs.get("requirements", []):
        if req.get("mandatory") is not True:
            errors.append(f"{req.get('id')}: every seeded requirement must be mandatory")
        for task in req.get("tasks", []):
            if task not in story_ids:
                errors.append(f"{req.get('id')}: unknown task {task}")
    return errors


def main() -> int:
    errors = _require_files()
    if not errors:
        plan = load_plan(ROOT)
        errors.extend(plan.errors)
        errors.extend(_check_obligations(ROOT))
        errors.extend(_check_ledger(ROOT))
        errors.extend(_check_requirements(ROOT))

    if errors:
        print(f"validate_plan: {len(errors)} error(s)")
        for err in errors:
            print(f"  - {err}")
        return 1

    plan = load_plan(ROOT)
    total_obligations = sum(len(s.test_obligations) for s in plan.stories.values())
    print(
        f"validate_plan: OK  stories={len(plan.stories)} "
        f"requirements={len(plan.requirements)} obligations={total_obligations} "
        f"deps_synthesized={plan.deps_synthesized}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())