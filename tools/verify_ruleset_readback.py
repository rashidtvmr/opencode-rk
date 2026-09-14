#!/usr/bin/env python3
"""Offline verification of an administrator-supplied GitHub ruleset readback.

This tool performs no network calls and no mutations. It compares a supplied
single-ruleset JSON object with the checked-in desired policy/import artifact.
Only a complete active readback can be reported as verified-matching. Missing
platform evidence remains unverified, and explicit semantic drift is a mismatch.
"""
from __future__ import annotations

import argparse
import copy
import json
import pathlib
import sys
from collections.abc import Mapping

ROOT = pathlib.Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tools.render_ruleset_import import build_import_artifact

POLICY_PATH = ROOT / ".github/protection-policy.json"
IMPORT_PATH = ROOT / ".github/rulesets/main.disabled.json"
MATCH_FIXTURE = ROOT / "tests/fixtures/rulesets/github-active-matching.json"
MISMATCH_FIXTURE = ROOT / "tests/fixtures/rulesets/github-active-mismatching.json"
INCOMPLETE_FIXTURE = ROOT / "tests/fixtures/rulesets/github-active-incomplete-export.json"

STATUS_VERIFIED = "verified-matching"
STATUS_UNVERIFIED = "unverified"
STATUS_MISMATCH = "mismatch"

SEMANTIC_TOP_LEVEL_KEYS = {
    "name",
    "target",
    "source_type",
    "enforcement",
    "conditions",
    "rules",
    "bypass_actors",
}

# GitHub REST responses may add these server-owned fields to a ruleset object.
# They are intentionally the only top-level fields ignored for semantic
# comparison. `source`, when present, is server-owned metadata but is still
# validated against the canonical repository before being ignored.
SERVER_METADATA_KEYS = {
    "id",
    "node_id",
    "source",
    "created_at",
    "updated_at",
    "_links",
}


def _load_json(path: pathlib.Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def expected_platform_ruleset(root: pathlib.Path = ROOT) -> tuple[dict, str]:
    policy = _load_json(root / ".github/protection-policy.json")
    artifact = _load_json(root / ".github/rulesets/main.disabled.json")
    if not isinstance(policy, Mapping) or not isinstance(artifact, Mapping):
        raise ValueError("checked-in protection policy/import artifact must be JSON objects")
    derived = build_import_artifact(dict(policy))
    if dict(artifact) != derived:
        raise ValueError("checked-in ruleset import artifact drifted from desired policy")
    desired = policy.get("desiredExternalRuleset")
    if not isinstance(desired, Mapping):
        raise ValueError("desiredExternalRuleset must be an object")
    enforcement = desired.get("desiredEnforcement")
    if enforcement != "active":
        raise ValueError("offline platform verifier currently requires desired enforcement=active")
    expected = copy.deepcopy(derived)
    expected["enforcement"] = enforcement
    repository = policy.get("repository")
    if not isinstance(repository, str) or not repository:
        raise ValueError("repository identity is missing from protection policy")
    return expected, repository


def _normalize_rules(rules: object, *, errors: list[str]) -> list[dict]:
    if not isinstance(rules, list):
        errors.append("rules must be a list")
        return []
    normalized: list[dict] = []
    seen: set[str] = set()
    for index, rule in enumerate(rules):
        if not isinstance(rule, Mapping):
            errors.append(f"rules[{index}] must be an object")
            continue
        rule_type = rule.get("type")
        if not isinstance(rule_type, str) or not rule_type:
            errors.append(f"rules[{index}] is missing a rule type")
            continue
        if rule_type in seen:
            errors.append(f"duplicate ruleset rule type: {rule_type}")
            continue
        seen.add(rule_type)

        if rule_type in {"deletion", "non_fast_forward", "required_linear_history"}:
            if set(rule) != {"type"}:
                errors.append(f"{rule_type} contains unowned semantic fields")
            normalized.append({"type": rule_type})
            continue

        if rule_type == "pull_request":
            if set(rule) != {"type", "parameters"} or not isinstance(rule.get("parameters"), Mapping):
                errors.append("pull_request rule shape drifted")
                continue
            params = dict(rule["parameters"])
            expected_keys = {
                "require_code_owner_review",
                "require_last_push_approval",
                "dismiss_stale_reviews_on_push",
                "required_approving_review_count",
                "required_review_thread_resolution",
            }
            extras = sorted(set(params) - expected_keys)
            missing = sorted(expected_keys - set(params))
            if extras:
                errors.append(f"pull_request contains unowned semantic fields: {extras}")
            if missing:
                errors.append(f"pull_request is missing required fields: {missing}")
            normalized.append({
                "type": rule_type,
                "parameters": {key: params.get(key) for key in sorted(expected_keys)},
            })
            continue

        if rule_type == "required_status_checks":
            if set(rule) != {"type", "parameters"} or not isinstance(rule.get("parameters"), Mapping):
                errors.append("required_status_checks rule shape drifted")
                continue
            params = dict(rule["parameters"])
            expected_keys = {
                "required_status_checks",
                "strict_required_status_checks_policy",
                "do_not_enforce_on_create",
            }
            extras = sorted(set(params) - expected_keys)
            missing = sorted(expected_keys - set(params))
            if extras:
                errors.append(f"required_status_checks contains unowned semantic fields: {extras}")
            if missing:
                errors.append(f"required_status_checks is missing required fields: {missing}")
            raw_checks = params.get("required_status_checks")
            checks: list[dict[str, str]] = []
            if not isinstance(raw_checks, list):
                errors.append("required_status_checks.required_status_checks must be a list")
            else:
                for check_index, check in enumerate(raw_checks):
                    if not isinstance(check, Mapping):
                        errors.append(f"required status check {check_index} must be an object")
                        continue
                    check_keys = set(check)
                    if not check_keys <= {"context", "integration_id"}:
                        errors.append(
                            f"required status check {check_index} contains unowned fields: "
                            f"{sorted(check_keys - {'context', 'integration_id'})}"
                        )
                    integration_id = check.get("integration_id")
                    if integration_id is not None:
                        errors.append(
                            "required status check has non-null integration_id, but source policy does not own a check-app identity"
                        )
                    context = check.get("context")
                    if not isinstance(context, str) or not context:
                        errors.append(f"required status check {check_index} is missing context")
                        continue
                    checks.append({"context": context})
            checks.sort(key=lambda item: item["context"])
            normalized.append({
                "type": rule_type,
                "parameters": {
                    "do_not_enforce_on_create": params.get("do_not_enforce_on_create"),
                    "required_status_checks": checks,
                    "strict_required_status_checks_policy": params.get("strict_required_status_checks_policy"),
                },
            })
            continue

        errors.append(f"unexpected ruleset rule type: {rule_type}")
        normalized.append(dict(rule))

    return sorted(normalized, key=lambda item: str(item.get("type", "")))


def _normalize_semantics(document: Mapping[str, object], *, repository: str, errors: list[str]) -> dict:
    unknown = sorted(set(document) - SEMANTIC_TOP_LEVEL_KEYS - SERVER_METADATA_KEYS)
    if unknown:
        errors.append(f"ruleset readback contains unsupported top-level fields: {unknown}")

    if "source" in document and document.get("source") != repository:
        errors.append(f"ruleset readback source drifted from {repository}")

    for key in ("id", "node_id", "created_at", "updated_at", "_links"):
        if key not in document:
            continue
        value = document[key]
        valid = {
            "id": isinstance(value, int) and not isinstance(value, bool),
            "node_id": isinstance(value, str),
            "created_at": isinstance(value, str),
            "updated_at": isinstance(value, str),
            "_links": isinstance(value, Mapping),
        }[key]
        if not valid:
            errors.append(f"server metadata field {key} has an unexpected type")

    conditions = document.get("conditions")
    normalized_conditions: dict = {}
    if not isinstance(conditions, Mapping):
        errors.append("conditions must be an object")
    else:
        if set(conditions) != {"ref_name"} or not isinstance(conditions.get("ref_name"), Mapping):
            errors.append("ruleset conditions drifted from exact ref_name targeting")
        ref_name = conditions.get("ref_name")
        if isinstance(ref_name, Mapping):
            if set(ref_name) != {"include", "exclude"}:
                errors.append("ref_name conditions contain missing or unowned fields")
            include = ref_name.get("include")
            exclude = ref_name.get("exclude")
            if not isinstance(include, list) or not all(isinstance(item, str) for item in include):
                errors.append("ref_name.include must be a list of strings")
                include = []
            if not isinstance(exclude, list) or not all(isinstance(item, str) for item in exclude):
                errors.append("ref_name.exclude must be a list of strings")
                exclude = []
            normalized_conditions = {"ref_name": {"exclude": sorted(exclude), "include": sorted(include)}}

    return {
        "name": document.get("name"),
        "target": document.get("target"),
        "source_type": document.get("source_type"),
        "enforcement": document.get("enforcement"),
        "conditions": normalized_conditions,
        "rules": _normalize_rules(document.get("rules"), errors=errors),
        "bypass_actors": document.get("bypass_actors"),
    }


def _policy_underspecification_errors(document: Mapping[str, object]) -> list[str]:
    """Return semantics present in platform data that desired policy does not own."""
    errors: list[str] = []
    rules = document.get("rules")
    if not isinstance(rules, list):
        return errors
    for rule in rules:
        if not isinstance(rule, Mapping) or not isinstance(rule.get("parameters"), Mapping):
            continue
        params = rule["parameters"]
        if rule.get("type") == "pull_request" and "allowed_merge_methods" in params:
            errors.append(
                "platform readback contains allowed_merge_methods, but source policy intentionally does not own a merge-method policy"
            )
        if rule.get("type") != "required_status_checks":
            continue
        checks = params.get("required_status_checks")
        if not isinstance(checks, list):
            continue
        for check in checks:
            if isinstance(check, Mapping) and check.get("integration_id") is not None:
                errors.append(
                    "platform readback has a non-null integration_id, but source policy does not own a check-app identity"
                )
    return errors


def verify_document(document: object, root: pathlib.Path = ROOT) -> dict[str, object]:
    try:
        expected, repository = expected_platform_ruleset(root)
    except (json.JSONDecodeError, OSError, TypeError, KeyError, ValueError) as exc:
        return {
            "status": STATUS_UNVERIFIED,
            "verified": False,
            "errors": [f"local desired-state inputs are invalid: {exc}"],
        }

    if not isinstance(document, Mapping):
        return {
            "status": STATUS_UNVERIFIED,
            "verified": False,
            "errors": ["administrator-supplied ruleset evidence must be a JSON object"],
        }

    missing = sorted(SEMANTIC_TOP_LEVEL_KEYS - set(document))
    if missing:
        reason = f"ruleset readback is incomplete; missing semantic fields: {missing}"
        if "bypass_actors" in missing:
            reason += "; GitHub ruleset-history exports and non-write API reads cannot prove the no-bypass requirement"
        return {"status": STATUS_UNVERIFIED, "verified": False, "errors": [reason]}

    if "source" not in document:
        return {
            "status": STATUS_UNVERIFIED,
            "verified": False,
            "errors": [
                "ruleset readback does not identify its repository source; verified-matching requires source=rashidtvmr/opencode-rk"
            ],
        }

    underspecified = _policy_underspecification_errors(document)
    if underspecified:
        return {
            "status": STATUS_UNVERIFIED,
            "verified": False,
            "errors": underspecified,
        }

    actual_errors: list[str] = []
    actual = _normalize_semantics(document, repository=repository, errors=actual_errors)
    expected_errors: list[str] = []
    normalized_expected = _normalize_semantics(expected, repository=repository, errors=expected_errors)
    if expected_errors:
        return {
            "status": STATUS_UNVERIFIED,
            "verified": False,
            "errors": [f"local expected ruleset failed normalization: {error}" for error in expected_errors],
        }

    if actual != normalized_expected:
        if actual.get("enforcement") != normalized_expected.get("enforcement"):
            actual_errors.append(
                f"enforcement drifted: expected {normalized_expected.get('enforcement')}, got {actual.get('enforcement')}"
            )
        if actual.get("conditions") != normalized_expected.get("conditions"):
            actual_errors.append("branch/ref conditions drifted from refs/heads/main")
        if actual.get("bypass_actors") != normalized_expected.get("bypass_actors"):
            actual_errors.append("bypass actor list drifted from the required empty list")
        if actual.get("rules") != normalized_expected.get("rules"):
            actual_errors.append("rules/check/review semantics drifted from desired policy")
        if actual.get("target") != normalized_expected.get("target"):
            actual_errors.append("ruleset target drifted")
        if actual.get("source_type") != normalized_expected.get("source_type"):
            actual_errors.append("ruleset source_type drifted")
        if actual.get("name") != normalized_expected.get("name"):
            actual_errors.append("ruleset name drifted from the checked-in import artifact")

    if actual_errors:
        return {
            "status": STATUS_MISMATCH,
            "verified": False,
            "errors": list(dict.fromkeys(actual_errors)),
        }
    return {
        "status": STATUS_VERIFIED,
        "verified": True,
        "errors": [],
        "repository": repository,
        "branch": "main",
        "requiredCheck": "planning",
        "note": "Point-in-time supplied readback matches desired active ruleset; repository platformState.verified remains false.",
    }


def fixture_self_test(root: pathlib.Path = ROOT) -> list[str]:
    errors: list[str] = []
    cases = (
        (root / MATCH_FIXTURE.relative_to(ROOT), STATUS_VERIFIED),
        (root / MISMATCH_FIXTURE.relative_to(ROOT), STATUS_MISMATCH),
        (root / INCOMPLETE_FIXTURE.relative_to(ROOT), STATUS_UNVERIFIED),
    )
    for path, expected_status in cases:
        if not path.is_file():
            errors.append(f"missing ruleset verifier fixture: {path.relative_to(root)}")
            continue
        try:
            result = verify_document(_load_json(path), root)
        except (json.JSONDecodeError, OSError) as exc:
            errors.append(f"invalid ruleset verifier fixture {path.relative_to(root)}: {exc}")
            continue
        if result.get("status") != expected_status:
            errors.append(
                f"ruleset verifier fixture {path.relative_to(root)} expected {expected_status}, got {result.get('status')}"
            )
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=pathlib.Path, help="administrator-supplied single-ruleset API/export JSON")
    parser.add_argument("--self-test", action="store_true", help="validate checked-in verifier fixtures only")
    args = parser.parse_args()

    if args.self_test:
        if args.input is not None:
            parser.error("--self-test cannot be combined with --input")
        errors = fixture_self_test(ROOT)
        if errors:
            print(json.dumps({"status": STATUS_MISMATCH, "verified": False, "errors": errors}, indent=2))
            return 1
        print(
            json.dumps(
                {
                    "status": "fixture-self-test-passed",
                    "verified": False,
                    "platformStateVerified": False,
                    "note": "Fixtures exercise the verifier only; they are not hosting-platform evidence.",
                },
                indent=2,
            )
        )
        return 0

    if args.input is None:
        print(
            json.dumps(
                {
                    "status": STATUS_UNVERIFIED,
                    "verified": False,
                    "errors": ["no administrator-supplied ruleset readback was provided"],
                },
                indent=2,
            )
        )
        return 2

    try:
        document = _load_json(args.input)
    except (json.JSONDecodeError, OSError) as exc:
        print(json.dumps({"status": STATUS_UNVERIFIED, "verified": False, "errors": [str(exc)]}, indent=2))
        return 2
    result = verify_document(document, ROOT)
    print(json.dumps(result, indent=2))
    return 0 if result["status"] == STATUS_VERIFIED else (2 if result["status"] == STATUS_UNVERIFIED else 1)


if __name__ == "__main__":
    raise SystemExit(main())
