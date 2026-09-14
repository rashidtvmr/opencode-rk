#!/usr/bin/env python3
"""Render/check the inert GitHub ruleset import artifact from desired policy.

The checked-in import recipe is intentionally disabled. Importing it must not be
treated as proof that GitHub protection is active; activation and verification
remain hosting-platform administrator actions.
"""
from __future__ import annotations

import argparse
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
POLICY_PATH = ROOT / ".github/protection-policy.json"
IMPORT_PATH = ROOT / ".github/rulesets/main.disabled.json"


def build_import_artifact(policy: dict) -> dict:
    desired = policy["desiredExternalRuleset"]
    if desired.get("allowBypass") is not False:
        raise ValueError("ruleset import renderer only supports the no-bypass desired policy")
    checks = desired.get("requiredStatusChecks")
    if not isinstance(checks, list) or not checks:
        raise ValueError("ruleset import renderer requires at least one status check")

    rules: list[dict] = []
    if desired.get("blockDeletions"):
        rules.append({"type": "deletion"})
    if desired.get("blockForcePushes"):
        rules.append({"type": "non_fast_forward"})
    if desired.get("requireLinearHistory"):
        rules.append({"type": "required_linear_history"})
    if desired.get("requirePullRequest"):
        rules.append({
            "type": "pull_request",
            "parameters": {
                "require_code_owner_review": bool(desired.get("requireCodeOwnerReview")),
                "require_last_push_approval": bool(desired.get("requireLastPushApproval")),
                "dismiss_stale_reviews_on_push": bool(desired.get("dismissStaleApprovals")),
                "required_approving_review_count": int(desired.get("requiredApprovingReviewCount", 0)),
                "required_review_thread_resolution": bool(desired.get("requireConversationResolution")),
            },
        })
    rules.append({
        "type": "required_status_checks",
        "parameters": {
            "required_status_checks": [{"context": str(context)} for context in checks],
            "strict_required_status_checks_policy": bool(desired.get("requireBranchUpToDate")),
            "do_not_enforce_on_create": False,
        },
    })

    return {
        "name": "opencode-rk main protection (IMPORT DISABLED)",
        "target": "branch",
        "source_type": "Repository",
        # Deliberately inert. An administrator must review and activate on GitHub.
        "enforcement": "disabled",
        "conditions": {
            "ref_name": {
                "exclude": [],
                "include": [f"refs/heads/{desired['targetBranch']}"],
            }
        },
        "rules": rules,
        "bypass_actors": [],
    }


def canonical_text(document: dict) -> str:
    return json.dumps(document, indent=2, sort_keys=False) + "\n"


def import_artifact_errors(root: pathlib.Path = ROOT) -> list[str]:
    policy_path = root / ".github/protection-policy.json"
    artifact_path = root / ".github/rulesets/main.disabled.json"
    if not policy_path.is_file():
        return ["missing desired protection policy: .github/protection-policy.json"]
    if not artifact_path.is_file():
        return ["missing GitHub ruleset import artifact: .github/rulesets/main.disabled.json"]
    try:
        policy = json.loads(policy_path.read_text(encoding="utf-8"))
        actual = json.loads(artifact_path.read_text(encoding="utf-8"))
        expected = build_import_artifact(policy)
    except (json.JSONDecodeError, KeyError, TypeError, ValueError, OSError) as exc:
        return [f"invalid GitHub ruleset import projection: {exc}"]
    errors: list[str] = []
    if actual != expected:
        errors.append("GitHub ruleset import artifact drifted from desired protection policy")
    if actual.get("enforcement") != "disabled":
        errors.append("checked-in GitHub ruleset import artifact must remain disabled/non-applying")
    if actual.get("bypass_actors") != []:
        errors.append("checked-in GitHub ruleset import artifact must not declare bypass actors")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--write", action="store_true")
    args = parser.parse_args()

    policy = json.loads(POLICY_PATH.read_text(encoding="utf-8"))
    expected = build_import_artifact(policy)
    if args.write:
        IMPORT_PATH.parent.mkdir(parents=True, exist_ok=True)
        IMPORT_PATH.write_text(canonical_text(expected), encoding="utf-8")
        print(f"render_ruleset_import: wrote {IMPORT_PATH.relative_to(ROOT)} (enforcement=disabled)")
        return 0

    errors = import_artifact_errors(ROOT)
    if errors:
        print(f"render_ruleset_import: {len(errors)} error(s)")
        for error in errors:
            print(f"  - {error}")
        return 1
    print("render_ruleset_import: OK  target=main required_check=planning enforcement=disabled")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
