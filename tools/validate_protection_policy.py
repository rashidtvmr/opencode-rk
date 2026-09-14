#!/usr/bin/env python3
"""Validate source-controlled repository ownership/protection desired state.

This validates what the Git tree can guarantee. It deliberately does not claim
that hosting-platform branch protection or rulesets are currently enabled.
"""
from __future__ import annotations

import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tools.render_ruleset_import import import_artifact_errors  # noqa: E402

POLICY_PATH = ROOT / ".github/protection-policy.json"
CODEOWNERS_PATH = ROOT / ".github/CODEOWNERS"

OWNER = "@rashidtvmr"
REPOSITORY = "rashidtvmr/opencode-rk"

EXPECTED_CODEOWNER_PATTERNS = (
    "*",
    "/.github/",
    "/.github/rulesets/main.disabled.json",
    "/AGENTS.md",
    "/PLAN.md",
    "/README.md",
    "/docs/ADAPTER_PROTOCOL.md",
    "/docs/AUTONOMOUS_EXECUTION.md",
    "/docs/REPOSITORY_PROTECTION.md",
    "/docs/SECURITY.md",
    "/docs/TDD.md",
    "/config/controller.settings.json",
    "/config/resource-targets.json",
    "/config/security-policy.json",
    "/requirements/user-requirements.json",
    "/tools/auto_drive.py",
    "/tools/lane_gate.py",
    "/tools/plan_model.py",
    "/tools/ralph_loop.py",
    "/tools/validate_repository.py",
    "/tools/validate_protection_policy.py",
    "/tools/validate_backlog_exhaustion.py",
    "/tools/reconcile_surfaces.py",
    "/tools/render_ruleset_import.py",
    "/tools/validate_plan.py",
    "/tests/bootstrap/test_ci_enforcement.py",
    "/tests/bootstrap/test_protection_policy.py",
    "/tests/bootstrap/test_backlog_exhaustion.py",
    "/tests/bootstrap/test_disc003_reconciliation.py",
    "/tests/bootstrap/test_validate_plan.py",
    "/ralph.json",
    "/FEATURES.md",
    "/sources/backlog-exhaustion.json",
    "/sources/behavior-surface-rules.json",
    "/sources/disc-003-evidence.json",
    "/sources/disc-003-reconciliation.json",
    "/sources/disc-003-reconciliation.manifest.json",
    "/sources/evidence.json",
    "/sources/upstream.lock.json",
    "/tasks/DISC-003.md",
    "/workspaces/DISC-003/source-map.json",
    "/workspaces/DISC-003/progress.md",
)

PROTECTED_PATHS = tuple(sorted({
    ".github/CODEOWNERS",
    ".github/protection-policy.json",
    ".github/rulesets/main.disabled.json",
    ".github/workflows/ci.yml",
    "AGENTS.md",
    "PLAN.md",
    "README.md",
    "config/controller.settings.json",
    "config/resource-targets.json",
    "config/security-policy.json",
    "docs/ADAPTER_PROTOCOL.md",
    "docs/AUTONOMOUS_EXECUTION.md",
    "docs/REPOSITORY_PROTECTION.md",
    "docs/SECURITY.md",
    "docs/TDD.md",
    "FEATURES.md",
    "ralph.json",
    "requirements/user-requirements.json",
    "sources/backlog-exhaustion.json",
    "sources/behavior-surface-rules.json",
    "sources/disc-003-evidence.json",
    "sources/disc-003-reconciliation.json",
    "sources/disc-003-reconciliation.manifest.json",
    "sources/evidence.json",
    "sources/upstream.lock.json",
    "tasks/DISC-003.md",
    "tests/bootstrap/test_backlog_exhaustion.py",
    "tests/bootstrap/test_ci_enforcement.py",
    "tests/bootstrap/test_disc003_reconciliation.py",
    "tests/bootstrap/test_protection_policy.py",
    "tests/bootstrap/test_validate_plan.py",
    "tools/auto_drive.py",
    "tools/lane_gate.py",
    "tools/plan_model.py",
    "tools/ralph_loop.py",
    "tools/reconcile_surfaces.py",
    "tools/render_ruleset_import.py",
    "tools/validate_backlog_exhaustion.py",
    "tools/validate_plan.py",
    "tools/validate_protection_policy.py",
    "tools/validate_repository.py",
    "workspaces/DISC-003/progress.md",
    "workspaces/DISC-003/source-map.json",
}))

EXPECTED_RULESET = {
    "desiredEnforcement": "active",
    "targetBranch": "main",
    "requiredStatusChecks": ["planning"],
    "requireBranchUpToDate": True,
    "requirePullRequest": True,
    "requiredApprovingReviewCount": 1,
    "requireCodeOwnerReview": True,
    "dismissStaleApprovals": True,
    "requireLastPushApproval": True,
    "requireConversationResolution": True,
    "requireLinearHistory": True,
    "blockForcePushes": True,
    "blockDeletions": True,
    "allowBypass": False,
}


def _load_policy(root: pathlib.Path) -> dict:
    return json.loads((root / ".github/protection-policy.json").read_text(encoding="utf-8"))


def _codeowner_records(root: pathlib.Path) -> list[tuple[str, tuple[str, ...]]]:
    records = []
    for raw in (root / ".github/CODEOWNERS").read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split()
        records.append((parts[0], tuple(parts[1:])))
    return records


def protection_policy_errors(root: pathlib.Path = ROOT) -> list[str]:
    errors: list[str] = []
    policy_path = root / ".github/protection-policy.json"
    codeowners_path = root / ".github/CODEOWNERS"
    if not policy_path.is_file():
        errors.append("missing required protection policy: .github/protection-policy.json")
        return errors
    if not codeowners_path.is_file():
        errors.append("missing required ownership policy: .github/CODEOWNERS")
        return errors

    try:
        policy = _load_policy(root)
    except (json.JSONDecodeError, OSError) as exc:
        return [f"invalid repository protection policy: {exc}"]

    if policy.get("schemaVersion") != 1:
        errors.append("repository protection policy schemaVersion must be 1")
    if policy.get("status") != "desired-external-state-not-verified":
        errors.append("repository protection policy must remain desired-state-only")
    if policy.get("repository") != REPOSITORY:
        errors.append(f"repository protection owner/name drifted from {REPOSITORY}")
    if policy.get("codeOwner") != OWNER:
        errors.append(f"repository protection code owner drifted from {OWNER}")

    source = policy.get("sourceControlled")
    if not isinstance(source, dict):
        errors.append("repository protection sourceControlled must be an object")
    else:
        expected_scalar = {
            "workflow": ".github/workflows/ci.yml",
            "requiredJob": "planning",
            "canonicalValidator": "tools/validate_repository.py",
            "codeowners": ".github/CODEOWNERS",
            "rulesetImportArtifact": ".github/rulesets/main.disabled.json",
            "rulesetRenderer": "tools/render_ruleset_import.py",
            "defaultOwnerPattern": "*",
        }
        for key, expected in expected_scalar.items():
            if source.get(key) != expected:
                errors.append(f"repository protection sourceControlled.{key} drifted")
        if source.get("protectedPaths") != list(PROTECTED_PATHS):
            errors.append("repository protection protectedPaths drifted")

    if policy.get("desiredExternalRuleset") != EXPECTED_RULESET:
        errors.append("repository protection desired external ruleset drifted")
    platform = policy.get("platformState")
    if not isinstance(platform, dict) or platform.get("verified") is not False:
        errors.append("repository source must not claim hosting-platform protection is verified")
    if not isinstance(platform, dict) or "cannot attest" not in str(platform.get("reason", "")):
        errors.append("repository protection policy must state the platform-attestation limitation")

    records = _codeowner_records(root)
    patterns = [pattern for pattern, _owners in records]
    duplicates = sorted({pattern for pattern in patterns if patterns.count(pattern) > 1})
    if duplicates:
        errors.append(f"CODEOWNERS contains duplicate protected patterns: {duplicates}")
    if tuple(patterns) != EXPECTED_CODEOWNER_PATTERNS:
        errors.append("CODEOWNERS protected pattern set/order drifted")
    for pattern, owners in records:
        if owners != (OWNER,):
            errors.append(f"CODEOWNERS owner drift for {pattern}: {owners}")

    for rel in PROTECTED_PATHS:
        if not (root / rel).is_file():
            errors.append(f"protected repository path is missing or renamed: {rel}")
    errors.extend(import_artifact_errors(root))
    return errors


def main() -> int:
    errors = protection_policy_errors(ROOT)
    if errors:
        print(f"validate_protection_policy: {len(errors)} error(s)")
        for error in errors:
            print(f"  - {error}")
        return 1
    print(
        "validate_protection_policy: OK  "
        f"owner={OWNER} protected_paths={len(PROTECTED_PATHS)} required_check=planning platform_verified=false"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
