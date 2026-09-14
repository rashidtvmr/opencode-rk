#!/usr/bin/env python3
"""Fail-closed validator for the post-DISC backlog exhaustion ledger.

The ledger is local reconciliation evidence only. It must never mutate or imply
controller acceptance. Its purpose is to make the current blocked/exhausted
state mechanically reviewable and to force any future category change to be an
explicit source-backed edit.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import sys
from collections import Counter, defaultdict
from collections.abc import Mapping

ROOT = pathlib.Path(__file__).resolve().parents[1]

LEDGER_PATH = ROOT / "sources/backlog-exhaustion.json"
DISC_EVIDENCE_KINDS = ("source", "caller", "test", "spec")
ENTERPRISE_REMOTE_GAP_PATH = ROOT / "sources/enterprise-remote-spec-gap.json"
ENTERPRISE_REMOTE_SURFACE = "opencode.enterprise-remote"
ENTERPRISE_REMOTE_GAP_STORIES = ("INT-010", "SHARE-003", "WEB-004")
ENTERPRISE_REMOTE_REQUIRED_PARTITIONS = (
    "enterprise-share-http",
    "function-syncserver-websocket-r2",
    "function-support-relay",
    "function-github-token-exchange-installation",
    "deployment-resource-lifecycle",
)
ROUTING_OWNERSHIP_GAP_PATH = ROOT / "sources/routing-ownership-gap.json"
ROUTING_GAP_STORIES = ("ROUTE-009", "ROUTE-010")
ROUTING_SIGNATURE_STORIES = ("ROUTE-008", "ROUTE-009", "ROUTE-010")
ROUTING_SUBTRACTED_OWNERS = (
    "ROUTE-001", "ROUTE-002", "ROUTE-003", "ROUTE-004", "ROUTE-005", "ROUTE-007", "ROUTE-011",
)
ROUTING_FROZEN_OWNERS = ("ROUTE-006", "ROUTE-008")
ROUTING_UNRESOLVED_PARTITIONS = (
    "account-storage-schema-credential-lifecycle",
    "dashboard-provider-status-and-secret-redaction",
    "dashboard-settings-security-and-runtime-side-effects",
    "routing-provider-specific-quota-proxy-and-credential-runtime",
    "combo-stream-provider-execution-and-token-refresh",
)
OPERATIONS_OWNERSHIP_GAP_PATH = ROOT / "sources/operations-ownership-gap.json"
OPERATIONS_GAP_STORIES = ("OPS-001", "OPS-002", "OPS-003", "OPS-004", "OPS-005", "OPS-006", "OPS-008")
OPERATIONS_EQUIVALENCE_GROUPS = (
    ("configuration-only", ("OPS-001", "OPS-005"), ("opencode.configuration-runtime",)),
    ("repository-only", ("OPS-002", "OPS-003", "OPS-006"), ("opencode.repository-operations",)),
    (
        "configuration-and-repository",
        ("OPS-004", "OPS-008"),
        ("opencode.configuration-runtime", "opencode.repository-operations"),
    ),
)
OPERATIONS_ACCEPTED_OWNERS = ("BASE-004", "BASE-005", "BASE-006", "BASE-007", "BASE-008")
OPERATIONS_FROZEN_OWNERS = ("INT-002", "OPS-007", "OPS-009")
OPERATIONS_UNRESOLVED_PARTITIONS = (
    "configuration-discovery-policy-and-location-lifetime",
    "scoped-state-transform-registration-disposal-and-reload",
    "location-service-map-cache-and-idle-eviction",
    "global-npm-installation-process-side-effects",
    "repository-reference-normalization-and-cache-identity",
    "repository-cache-git-filesystem-lock-lifecycle",
    "repository-observability-installation-and-container-remainder",
)
RELEASE_ASSURANCE_GAP_PATH = ROOT / "sources/release-assurance-gap.json"
RELEASE_ASSURANCE_STORIES = ("REL-001", "REL-002", "REL-003")
RELEASE_ASSURANCE_PARTITIONS = (
    "release-feature-accounting-validator",
    "strict-tdd-independent-verification-validator",
    "safety-resource-correctness-release-validator",
)

CATEGORY_IDS = {
    "local-implemented-stale": {
        "AUTO-003", "AUTO-007", "EXT-003", "EXT-007", "EXT-013", "INT-004", "INT-008",
        "OPS-010", "PROV-014", "REL-004", "ROUTE-001", "ROUTE-002", "ROUTE-003",
        "ROUTE-004", "ROUTE-005", "ROUTE-007", "ROUTE-011", "ROUTE-012", "SESS-019",
        "SESS-020", "TOOL-015",
    },
    "explicit-blocker": {
        "AUTO-004", "AUTO-005", "AUTO-006", "EXT-008", "INT-002", "OPS-007", "OPS-009",
        "ROUTE-006", "ROUTE-008",
    },
    "dependency-constrained": {
        *(f"UI-{number:03d}" for number in range(1, 19)),
        "WEB-001", "WEB-002", "WEB-003", "WEB-005",
    },
    "unresolved-decomposition": {
        "EXT-001", "EXT-002", "EXT-004", "EXT-005", "EXT-006", "EXT-009", "EXT-010",
        "EXT-011", "EXT-012", "INT-001", "INT-003", "INT-005", "INT-006", "INT-007",
        "INT-009", "INT-010", "OPS-001", "OPS-002", "OPS-003", "OPS-004", "OPS-005",
        "OPS-006", "OPS-008", "REL-001", "REL-002", "REL-003", "ROUTE-009", "ROUTE-010",
        "SHARE-001", "SHARE-002", "SHARE-003", "SHARE-004", "SHARE-005", "WEB-004",
    },
}

STALE_IMPLEMENTATION_COMMITS = {
    "AUTO-003": ["d8436e62b0fc18712cb803fa440dd7ae2084cecc"],
    "AUTO-007": ["3be4d3c7329da6870fcc979c078de4b780f6c75d"],
    "EXT-003": ["31a5adec20d0e70a2e02f4fdc1d626c14c5098b6"],
    "EXT-007": ["b76d6126c97f928b5145dd19b6bdec1c4e70aa83"],
    "EXT-013": ["accb2e919fe3e02ac34aa4b42851ec710f8ca282"],
    "INT-004": ["d8fa4854fcd680e5fe3dfe2b05c6a2151a340056"],
    "INT-008": [
        "b496878d8192dc3b10d56c00e68597354f99ea32",
        "4e779bedd3018b97ccb981c2edeb6163b0181f83",
        "39887dda97eb7163c8d8725a5a9ecfddc00a9234",
    ],
    "OPS-010": ["01e22f7f73599e4b487a273cd58eba206f38eb09"],
    "PROV-014": ["9800d0bc1a339d5a8fa1b2a60c53ae4a5f67d058"],
    "REL-004": ["ba57eafd21facf9df2cb85300d8db87da62a1b75"],
    "ROUTE-001": ["5e1c98ef6133f414dd9311a831dd20126cdbe3bf"],
    "ROUTE-002": ["2d1a47d7caad2b1ef044cb4f963a112c969623ea"],
    "ROUTE-003": ["dabf09d16e3989ac1835b736a8a684d226d4bb84"],
    "ROUTE-004": ["47d749cfd62c66718d0ad58e8512c871bc91a121"],
    "ROUTE-005": ["116ef477b4489a22aa4edd9ec6525bcea2b9c3d5"],
    "ROUTE-007": ["79448b709f32939f76a7f2b8b64d439ff0325625"],
    "ROUTE-011": ["20155fed4de0bf97853e7a022a121672f8d8dd2f"],
    "ROUTE-012": ["caf050398a84a6aced49b72c1c2bba9b5bb5555e"],
    "SESS-019": ["29bfeb7086d88092cf2ccf5337da97b5e6d9b9e8"],
    "SESS-020": ["d9a771492c085bed0b3a1e404abccee6a65553c6"],
    "TOOL-015": ["2780889f1182ea65bb61e18e939a6c2ab037caca"],
}

REASON_BY_ID = {}
for _id in CATEGORY_IDS["local-implemented-stale"]:
    REASON_BY_ID[_id] = "local-implemented-controller-stale"
for _id in {"AUTO-004", "AUTO-005", "AUTO-006"}:
    REASON_BY_ID[_id] = "automation-ownership-undefined"
REASON_BY_ID.update({
    "EXT-008": "plugin-hook-contract-mismatch",
    "INT-002": "repository-contract-mixes-side-effects",
    "OPS-007": "effect-runtime-cross-surface-ownership",
    "OPS-009": "recorder-cross-surface-side-effects",
    "ROUTE-006": "proxy-authority-and-platform-boundary",
    "ROUTE-008": "routing-dashboard-ownership-overlap",
})
for _id in CATEGORY_IDS["dependency-constrained"]:
    REASON_BY_ID[_id] = "client-architecture-dependency"
for _id in CATEGORY_IDS["unresolved-decomposition"]:
    prefix = _id.split("-", 1)[0]
    REASON_BY_ID[_id] = {
        "EXT": "extensibility-family-not-decomposed",
        "INT": "integration-family-not-decomposed",
        "OPS": "operations-family-not-decomposed",
        "REL": "release-assurance-not-product-owner",
        "ROUTE": "routing-family-not-decomposed",
        "SHARE": "sharing-family-not-decomposed",
        "WEB": "hosted-remote-family-not-decomposed",
    }[prefix]


def _load(path: pathlib.Path):
    return json.loads(path.read_text(encoding="utf-8"))


def _task_status(path: pathlib.Path) -> str | None:
    if not path.is_file():
        return None
    match = re.search(r"^Status:\s*(.+?)\.\s*Kind:", path.read_text(encoding="utf-8"), re.MULTILINE)
    return match.group(1).strip() if match else None


def _reason_policy(category: str) -> str:
    return {
        "local-implemented-stale": "controller-verifier-acceptance-external",
        "explicit-blocker": "material-new-pinned-evidence-required",
        "dependency-constrained": "approved-client-architecture-and-dependencies-required",
        "unresolved-decomposition": "source-grounded-task-decomposition-required",
    }[category]


def _local_evidence_paths(root: pathlib.Path, story_id: str, category: str, surface_ids: list[str]) -> list[str]:
    paths: list[str] = []
    task = root / "tasks" / f"{story_id}.md"
    worklog = root / "worklog" / f"{story_id}.md"
    if task.is_file():
        paths.append(f"tasks/{story_id}.md")
    if worklog.is_file():
        paths.append(f"worklog/{story_id}.md")
    if surface_ids:
        return paths
    if story_id in {"AUTO-004", "AUTO-005", "AUTO-006"}:
        paths.extend(["requirements/user-requirements.json", "FEATURES.md", "worklog/AUTO-003.md"])
    elif story_id.startswith("REL-"):
        paths.extend(["requirements/user-requirements.json", "FEATURES.md", "PLAN.md", "docs/TDD.md", "docs/SECURITY.md"])
    elif story_id.startswith("UI-"):
        paths.extend(["requirements/user-requirements.json", "FEATURES.md", "PLAN.md"])
    elif category == "local-implemented-stale":
        # Stale local implementations without a DISC surface are fully evidenced
        # by their task/worklog plus the implementation commit(s).
        pass
    else:
        paths.extend(["ralph.json", "FEATURES.md"])
    return list(dict.fromkeys(paths))


def surface_evidence_gaps(
    surface_ids: list[str],
    reviewed: Mapping[str, object],
) -> list[dict[str, object]]:
    """Project each mapped surface's still-missing evidence classes explicitly."""
    gaps: list[dict[str, object]] = []
    for surface_id in surface_ids:
        surface = reviewed.get(surface_id)
        if not isinstance(surface, Mapping):
            gaps.append({"surfaceId": surface_id, "missingKinds": list(DISC_EVIDENCE_KINDS)})
            continue
        evidence = surface.get("evidence", {})
        missing = [
            kind
            for kind in DISC_EVIDENCE_KINDS
            if not isinstance(evidence, Mapping)
            or not isinstance(evidence.get(kind), list)
            or not evidence.get(kind)
        ]
        if missing:
            gaps.append({"surfaceId": surface_id, "missingKinds": missing})
    return gaps


def build_expected_ledger(root: pathlib.Path = ROOT) -> dict[str, object]:
    plan = _load(root / "ralph.json")
    rules = _load(root / "sources/behavior-surface-rules.json")
    reconciliation = _load(root / "sources/disc-003-reconciliation.json")
    reviewed = {row["id"]: row for row in reconciliation.get("reviewedSurfaces", [])}

    surfaces_by_feature: dict[str, list[str]] = defaultdict(list)
    for rule in rules.get("rules", []):
        for feature_id in rule.get("featureIds", []):
            surfaces_by_feature[str(feature_id)].append(str(rule["id"]))

    category_for: dict[str, str] = {}
    for category, ids in CATEGORY_IDS.items():
        for story_id in ids:
            if story_id in category_for:
                raise ValueError(f"story is classified twice in validator policy: {story_id}")
            category_for[story_id] = category

    stories = []
    accepted = 0
    ralph_status_counts = Counter()
    for story in plan.get("userStories", []):
        story_id = str(story["id"])
        status = str(story["status"])
        ralph_status_counts[status] += 1
        if status == "accepted":
            accepted += 1
            continue
        category = category_for.get(story_id)
        if category is None:
            raise ValueError(f"non-accepted story has no validator classification: {story_id}")
        surface_ids = sorted(surfaces_by_feature.get(story_id, []))
        evidence_ids: list[str] = []
        for surface_id in surface_ids:
            row = reviewed.get(surface_id)
            if not row:
                continue
            for kind in ("source", "caller", "test", "spec"):
                for ref in (row.get("evidence") or {}).get(kind, []):
                    evidence_id = str(ref.get("evidenceId", ""))
                    if evidence_id and evidence_id not in evidence_ids:
                        evidence_ids.append(evidence_id)
        task_path = root / "tasks" / f"{story_id}.md"
        worklog_path = root / "worklog" / f"{story_id}.md"
        task_status = _task_status(task_path)
        stories.append({
            "id": story_id,
            "category": category,
            "controllerStatus": status,
            "requirementIds": list(story.get("requirementIds", [])),
            "reasonKey": REASON_BY_ID[story_id],
            "reopenPolicy": _reason_policy(category),
            "surfaceIds": surface_ids,
            "evidenceIds": evidence_ids,
            "surfaceEvidenceGaps": surface_evidence_gaps(surface_ids, reviewed),
            "localEvidencePaths": _local_evidence_paths(root, story_id, category, surface_ids),
            "taskCard": f"tasks/{story_id}.md" if task_path.is_file() else None,
            "taskStatus": task_status,
            "worklog": f"worklog/{story_id}.md" if worklog_path.is_file() else None,
            "implementationCommits": STALE_IMPLEMENTATION_COMMITS.get(story_id, []),
        })

    counts = Counter(row["category"] for row in stories)
    return {
        "schemaVersion": 1,
        "status": "in-progress-not-release-evidence",
        "baseline": "post-efad109-exhaustion-normalization",
        "summary": {
            "storyCount": len(plan.get("userStories", [])),
            "controllerAccepted": accepted,
            "nonAccepted": len(stories),
            "ralphStatusCounts": dict(sorted(ralph_status_counts.items())),
            "classificationCounts": dict(sorted(counts.items())),
        },
        "stories": stories,
    }


def _features_status_errors(root: pathlib.Path, plan: Mapping[str, object]) -> list[str]:
    text = (root / "FEATURES.md").read_text(encoding="utf-8")
    expected = {str(row["id"]): str(row["status"]) for row in plan.get("userStories", [])}
    errors: list[str] = []
    for story_id, status in expected.items():
        occurrences = re.findall(rf"`{re.escape(story_id)}`\s+\((accepted|in-progress|not-started)\)", text)
        occurrences += re.findall(rf"\|\s*`{re.escape(story_id)}`\s*\|[^\n]*?\|\s*(accepted|in-progress|not-started)\s*\|", text)
        if not occurrences:
            errors.append(f"FEATURES.md: missing status occurrence for {story_id}")
            continue
        wrong = sorted({value for value in occurrences if value != status})
        if wrong:
            errors.append(f"FEATURES.md: {story_id} has stale statuses {wrong}, expected {status}")
    return errors


def disc_status_errors(
    reconciliation: Mapping[str, object],
    manifest: Mapping[str, object],
    source_map: Mapping[str, object],
    task_text: str,
    progress_text: str,
) -> list[str]:
    errors: list[str] = []
    if reconciliation.get("status") != "in-progress-not-release-evidence":
        errors.append("DISC-003 reconciliation status changed from in-progress-not-release-evidence")
    if manifest.get("status") != "in-progress-not-release-evidence":
        errors.append("DISC-003 manifest status changed from in-progress-not-release-evidence")
    if source_map.get("status") != "in-progress":
        errors.append("DISC-003 source-map status changed from in-progress")
    if reconciliation.get("queuedSurfaceIds") != []:
        errors.append("DISC-003 exhaustion ledger expects zero queued surface families")
    if any(row.get("reviewState") != "partial" for row in reconciliation.get("reviewedSurfaces", [])):
        errors.append("DISC-003 exhaustion ledger requires all reviewed surfaces to remain partial")
    if "Status: IN PROGRESS." not in task_text or "not accepted" not in task_text.lower():
        errors.append("tasks/DISC-003.md no longer states IN PROGRESS / not accepted")
    if "Status: **IN PROGRESS / NOT ACCEPTED**." not in progress_text:
        errors.append("workspaces/DISC-003/progress.md no longer states IN PROGRESS / NOT ACCEPTED")
    return errors


def residual_evidence_kind_errors(rows, reconciliation: Mapping[str, object]) -> list[str]:
    """Require every surface-backed residual row to retain all four evidence classes."""
    reviewed = {
        str(row.get("id", "")): row
        for row in reconciliation.get("reviewedSurfaces", [])
        if isinstance(row, Mapping)
    }
    errors: list[str] = []
    for row in rows:
        if not isinstance(row, Mapping):
            continue
        story_id = str(row.get("id", ""))
        surface_ids = row.get("surfaceIds", [])
        if not isinstance(surface_ids, list) or not surface_ids:
            continue
        present: set[str] = set()
        for surface_id in surface_ids:
            surface = reviewed.get(str(surface_id))
            if surface is None:
                errors.append(f"{story_id}: mapped DISC surface is missing from reconciliation: {surface_id}")
                continue
            evidence = surface.get("evidence", {})
            if not isinstance(evidence, Mapping):
                continue
            for kind in DISC_EVIDENCE_KINDS:
                refs = evidence.get(kind)
                if isinstance(refs, list) and refs:
                    present.add(kind)
        missing = [kind for kind in DISC_EVIDENCE_KINDS if kind not in present]
        if missing:
            errors.append(
                f"{story_id}: surface-backed exhaustion evidence lost classes: {', '.join(missing)}"
            )
    return errors


def _surface_path_matches(path: str, patterns: list[str]) -> bool:
    for pattern in patterns:
        if pattern.endswith("/**"):
            prefix = pattern[:-3]
            if path == prefix or path.startswith(prefix + "/"):
                return True
        elif path == pattern:
            return True
    return False


def enterprise_remote_gap_errors(
    rows: list[object],
    reconciliation: Mapping[str, object],
    root: pathlib.Path = ROOT,
) -> list[str]:
    """Lock the truthful enterprise-remote negative-spec result fail closed."""
    errors: list[str] = []
    path = root / "sources/enterprise-remote-spec-gap.json"
    if not path.is_file():
        return ["missing machine-checkable enterprise-remote spec-gap record"]
    try:
        gap = _load(path)
    except (json.JSONDecodeError, OSError) as exc:
        return [f"invalid enterprise-remote spec-gap record: {exc}"]

    if gap.get("schemaVersion") != 1:
        errors.append("enterprise-remote spec-gap schemaVersion must be 1")
    if gap.get("status") != "searched-no-qualifying-in-surface-spec":
        errors.append("enterprise-remote spec-gap status drifted")
    if gap.get("surfaceId") != ENTERPRISE_REMOTE_SURFACE:
        errors.append("enterprise-remote spec-gap surfaceId drifted")
    if gap.get("repositoryId") != "opencode" or gap.get("repository") != "anomalyco/opencode":
        errors.append("enterprise-remote spec-gap repository identity drifted")
    if gap.get("residualStoryIds") != list(ENTERPRISE_REMOTE_GAP_STORIES):
        errors.append("enterprise-remote residual story set drifted")
    if gap.get("missingKinds") != ["spec"]:
        errors.append("enterprise-remote expected missing evidence kind must remain spec")
    if gap.get("requiredDecompositionPartitions") != list(ENTERPRISE_REMOTE_REQUIRED_PARTITIONS):
        errors.append("enterprise-remote required decomposition partitions drifted")

    lock = _load(root / "sources/upstream.lock.json")
    locked = next((item for item in lock.get("repositories", []) if item.get("id") == "opencode"), None)
    locked_commit = str(locked.get("commit", "")) if isinstance(locked, Mapping) else ""
    if not locked_commit or gap.get("commit") != locked_commit:
        errors.append("enterprise-remote negative evidence is not bound to the locked OpenCode commit")

    rules = _load(root / "sources/behavior-surface-rules.json")
    surface_rule = next((item for item in rules.get("rules", []) if item.get("id") == ENTERPRISE_REMOTE_SURFACE), None)
    expected_patterns = list(surface_rule.get("patterns", [])) if isinstance(surface_rule, Mapping) else []
    if not expected_patterns or gap.get("rulePatterns") != expected_patterns:
        errors.append("enterprise-remote negative evidence rule patterns drifted from behavior-surface rules")

    searched_trees = gap.get("searchedTrees")
    expected_trees = [
        {"path": "packages/enterprise", "treeSha": "de3cbb958ad0ef982f5922c8d9ac7ea7100ff4aa"},
        {"path": "packages/function", "treeSha": "19db2fcad6271bb67d6331a3c2069b416ea636af"},
    ]
    if searched_trees != expected_trees:
        errors.append("enterprise-remote searched subtree pins drifted")

    reviewed = {
        str(item.get("id", "")): item
        for item in reconciliation.get("reviewedSurfaces", [])
        if isinstance(item, Mapping)
    }
    surface = reviewed.get(ENTERPRISE_REMOTE_SURFACE)
    if surface is None:
        errors.append("enterprise-remote reconciliation row is missing")
    else:
        evidence = surface.get("evidence")
        if not isinstance(evidence, Mapping):
            errors.append("enterprise-remote reconciliation evidence is missing")
        else:
            if evidence.get("spec") != []:
                errors.append(
                    "enterprise-remote spec evidence must remain empty until the negative-spec record is intentionally resolved"
                )
            live_runtime_ids = [
                str(ref.get("evidenceId", ""))
                for kind in ("source", "caller", "test")
                for ref in evidence.get(kind, []) if isinstance(ref, Mapping)
            ]
            if gap.get("reviewedRuntimeEvidenceIds") != live_runtime_ids:
                errors.append("enterprise-remote reviewed runtime evidence ids drifted")

    expected_gap = [{"surfaceId": ENTERPRISE_REMOTE_SURFACE, "missingKinds": ["spec"]}]
    actual_gap_rows = {
        str(row.get("id", "")): row.get("surfaceEvidenceGaps")
        for row in rows
        if isinstance(row, Mapping) and row.get("surfaceEvidenceGaps")
    }
    wanted_gap_rows = {story_id: expected_gap for story_id in ENTERPRISE_REMOTE_GAP_STORIES}
    if actual_gap_rows != wanted_gap_rows:
        errors.append(
            "residual per-surface evidence gaps must remain exactly INT-010/SHARE-003/WEB-004 -> enterprise-remote spec"
        )

    candidates = gap.get("rejectedCandidates")
    expected_candidates = {
        "packages/enterprise/README.md": (
            "9337430cfd31be33675b8e7336b9260906e98440", 1, 32,
        ),
        "packages/enterprise/src/routes/api/[...path].ts": (
            "4677d68d33c48c6a030ae66100d59823fc241aa9", 16, 155,
        ),
        "packages/enterprise/package.json": (
            "9d61ae5eacfd8702f446da9fe8b33d384d60e906", 1, 46,
        ),
        "packages/function/package.json": (
            "7d9bc6548b66027e1c5fc17f0067175d8b208a1c", 1, 23,
        ),
        "packages/enterprise/sst-env.d.ts": (
            "64441936d7a02748b870556190c893d73e560948", 1, 10,
        ),
        "packages/function/sst-env.d.ts": (
            "64441936d7a02748b870556190c893d73e560948", 1, 10,
        ),
    }
    if not isinstance(candidates, list) or not candidates:
        errors.append("enterprise-remote spec-gap record must retain rejected in-surface candidates")
    else:
        seen_paths: set[str] = set()
        for candidate in candidates:
            if not isinstance(candidate, Mapping):
                errors.append("enterprise-remote rejected candidate must be an object")
                continue
            candidate_path = candidate.get("path")
            if not isinstance(candidate_path, str) or not candidate_path:
                errors.append("enterprise-remote rejected candidate is missing path")
                continue
            if candidate_path in seen_paths:
                errors.append(f"enterprise-remote rejected candidate is duplicated: {candidate_path}")
            seen_paths.add(candidate_path)
            if not _surface_path_matches(candidate_path, expected_patterns):
                errors.append(f"enterprise-remote rejected candidate escaped surface rule: {candidate_path}")
            if not re.fullmatch(r"[0-9a-f]{40}", str(candidate.get("blobSha", ""))):
                errors.append(f"enterprise-remote rejected candidate lacks pinned blob SHA: {candidate_path}")
            reviewed_lines = candidate.get("reviewedLines")
            if (
                not isinstance(reviewed_lines, Mapping)
                or not isinstance(reviewed_lines.get("from"), int)
                or not isinstance(reviewed_lines.get("to"), int)
                or reviewed_lines["from"] < 1
                or reviewed_lines["to"] < reviewed_lines["from"]
            ):
                errors.append(f"enterprise-remote rejected candidate lacks reviewed line range: {candidate_path}")
            if not str(candidate.get("reason", "")).strip():
                errors.append(f"enterprise-remote rejected candidate lacks insufficiency reason: {candidate_path}")
        if seen_paths != set(expected_candidates):
            errors.append("enterprise-remote rejected candidate inventory drifted")
        for candidate in candidates:
            if not isinstance(candidate, Mapping):
                continue
            candidate_path = candidate.get("path")
            expected = expected_candidates.get(str(candidate_path))
            if expected is None:
                continue
            blob_sha, first_line, last_line = expected
            reviewed_lines = candidate.get("reviewedLines")
            if candidate.get("blobSha") != blob_sha:
                errors.append(f"enterprise-remote rejected candidate blob drifted: {candidate_path}")
            if not isinstance(reviewed_lines, Mapping) or reviewed_lines.get("from") != first_line or reviewed_lines.get("to") != last_line:
                errors.append(f"enterprise-remote rejected candidate reviewed range drifted: {candidate_path}")

    history = gap.get("historyReview")
    if not isinstance(history, Mapping) or history.get("state") != "locked-checkout-grafted-at-pinned-commit":
        errors.append("enterprise-remote history-review limitation drifted")
    elif not str(history.get("limitation", "")).strip():
        errors.append("enterprise-remote history-review limitation is missing")
    if "qualifying specification" not in str(gap.get("closure", "")):
        errors.append("enterprise-remote spec-gap closure condition drifted")
    return errors


def _inventory_index(root: pathlib.Path, repository_id: str) -> tuple[dict[str, Mapping[str, object]], list[str]]:
    path = root / "sources/inventory" / f"{repository_id}.jsonl"
    rows: dict[str, Mapping[str, object]] = {}
    errors: list[str] = []
    if not path.is_file():
        return rows, [f"missing pinned inventory for ownership evidence: {path.relative_to(root)}"]
    try:
        for line_number, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if not raw.strip():
                continue
            value = json.loads(raw)
            if not isinstance(value, Mapping) or not isinstance(value.get("path"), str):
                errors.append(f"invalid {repository_id} inventory row at line {line_number}")
                continue
            rows[str(value["path"])] = value
    except (json.JSONDecodeError, OSError) as exc:
        errors.append(f"invalid {repository_id} inventory: {exc}")
    return rows, errors


def routing_ownership_gap_errors(
    rows: list[object],
    reconciliation: Mapping[str, object],
    root: pathlib.Path = ROOT,
) -> list[str]:
    """Keep ROUTE-009/010 and adjacent singleton arithmetic from becoming ownership proof."""
    errors: list[str] = []
    gap_path = root / "sources/routing-ownership-gap.json"
    if not gap_path.is_file():
        return ["missing machine-checkable routing ownership-gap record"]
    try:
        gap = _load(gap_path)
    except (json.JSONDecodeError, OSError) as exc:
        return [f"invalid routing ownership-gap record: {exc}"]

    if gap.get("schemaVersion") != 1:
        errors.append("routing ownership-gap schemaVersion must be 1")
    if gap.get("status") != "source-reviewed-no-exact-task-owner":
        errors.append("routing ownership-gap status drifted")
    if gap.get("repositoryId") != "9router" or gap.get("repository") != "decolua/9router":
        errors.append("routing ownership-gap repository identity drifted")
    if gap.get("storyIds") != list(ROUTING_GAP_STORIES):
        errors.append("routing ownership-gap story set drifted")
    if gap.get("ownershipDecision") != {story_id: None for story_id in ROUTING_GAP_STORIES}:
        errors.append("routing ownership must remain unresolved until source-grounded task evidence changes")
    if gap.get("unresolvedPartitions") != list(ROUTING_UNRESOLVED_PARTITIONS):
        errors.append("routing unresolved decomposition partitions drifted")

    lock = _load(root / "sources/upstream.lock.json")
    locked = next((item for item in lock.get("repositories", []) if item.get("id") == "9router"), None)
    locked_commit = str(locked.get("commit", "")) if isinstance(locked, Mapping) else ""
    locked_tree = str(locked.get("treeSha", "")) if isinstance(locked, Mapping) else ""
    if not locked_commit or gap.get("commit") != locked_commit:
        errors.append("routing ownership-gap is not bound to the locked 9router commit")
    if not locked_tree or gap.get("treeSha") != locked_tree:
        errors.append("routing ownership-gap is not bound to the locked 9router tree")

    plan = _load(root / "ralph.json")
    plan_by_id = {
        str(item.get("id", "")): item
        for item in plan.get("userStories", [])
        if isinstance(item, Mapping)
    }
    generic_story = "Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json"
    for story_id in ROUTING_GAP_STORIES:
        story = plan_by_id.get(story_id)
        if story is None:
            errors.append(f"routing ownership-gap story disappeared from Ralph: {story_id}")
            continue
        if story.get("status") != "not-started" or story.get("userStory") != generic_story:
            errors.append(f"{story_id}: routing ownership-gap is stale after Ralph task semantics changed")
        if story.get("requirementIds") != []:
            errors.append(f"{story_id}: routing ownership-gap expects no task-level requirement binding")

    rules = _load(root / "sources/behavior-surface-rules.json")
    rule_rows = [item for item in rules.get("rules", []) if isinstance(item, Mapping)]
    live_signatures = {
        story_id: sorted(
            str(item.get("id"))
            for item in rule_rows
            if story_id in item.get("featureIds", [])
        )
        for story_id in ROUTING_SIGNATURE_STORIES
    }
    recorded_signatures = gap.get("storySurfaceSignatures")
    if not isinstance(recorded_signatures, Mapping):
        errors.append("routing ownership-gap storySurfaceSignatures must be an object")
    else:
        for story_id, signature in live_signatures.items():
            if recorded_signatures.get(story_id) != signature:
                errors.append(f"{story_id}: routing ownership-gap surface signature drifted from live rules")
    if live_signatures["ROUTE-008"] != live_signatures["ROUTE-009"]:
        errors.append("ROUTE-008/ROUTE-009 are no longer surface-indistinguishable; routing ownership gap needs review")
    ambiguity = gap.get("surfaceAmbiguity")
    if not isinstance(ambiguity, Mapping) or ambiguity.get("indistinguishablePair") != ["ROUTE-008", "ROUTE-009"]:
        errors.append("routing ownership-gap lost the ROUTE-008/ROUTE-009 indistinguishable-pair guard")
    if not isinstance(ambiguity, Mapping) or "singleton membership" not in str(ambiguity.get("singletonCaveat", "")):
        errors.append("routing ownership-gap must explicitly reject ROUTE-010 singleton arithmetic")

    row_by_id = {
        str(item.get("id", "")): item
        for item in rows
        if isinstance(item, Mapping)
    }
    for story_id in ROUTING_GAP_STORIES:
        row = row_by_id.get(story_id)
        if row is None:
            errors.append(f"routing ownership-gap story missing from exhaustion ledger: {story_id}")
            continue
        if row.get("category") != "unresolved-decomposition" or row.get("reasonKey") != "routing-family-not-decomposed":
            errors.append(f"{story_id}: routing ownership-gap classification drifted")
        if row.get("taskCard") is not None or row.get("worklog") is not None or row.get("implementationCommits") != []:
            errors.append(f"{story_id}: task/worklog/implementation appeared; routing ownership gap must be deliberately reconciled")
        if sorted(row.get("surfaceIds", [])) != live_signatures[story_id]:
            errors.append(f"{story_id}: ledger surface projection drifted from routing ownership-gap rules")

    subtracted = gap.get("subtractedOwners")
    expected_subtracted_commits = {
        story_id: STALE_IMPLEMENTATION_COMMITS[story_id][0]
        for story_id in ROUTING_SUBTRACTED_OWNERS
    }
    if not isinstance(subtracted, list):
        errors.append("routing ownership-gap subtractedOwners must be a list")
    else:
        ids = [str(item.get("id", "")) for item in subtracted if isinstance(item, Mapping)]
        if ids != list(ROUTING_SUBTRACTED_OWNERS):
            errors.append("routing ownership-gap implemented-owner subtraction drifted")
        for item in subtracted:
            if not isinstance(item, Mapping):
                errors.append("routing ownership-gap subtracted owner must be an object")
                continue
            story_id = str(item.get("id", ""))
            expected_commit = expected_subtracted_commits.get(story_id)
            if expected_commit is None:
                continue
            if item.get("implementationCommit") != expected_commit:
                errors.append(f"{story_id}: routing ownership-gap implementation receipt drifted")
            task = root / str(item.get("task", ""))
            worklog = root / str(item.get("worklog", ""))
            if not task.is_file() or not worklog.is_file():
                errors.append(f"{story_id}: routing ownership-gap implemented task/worklog receipt missing")
            elif not str(_task_status(task) or "").startswith("IMPLEMENTED"):
                errors.append(f"{story_id}: routing ownership-gap subtracted task is no longer IMPLEMENTED")
            if not str(item.get("ownedPartition", "")).strip():
                errors.append(f"{story_id}: routing ownership-gap lacks the already-owned partition description")
            result = subprocess.run(
                ["git", "merge-base", "--is-ancestor", expected_commit, "HEAD"], cwd=root,
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False,
            )
            if result.returncode != 0:
                errors.append(f"{story_id}: routing ownership-gap implementation receipt is not in live history")

    frozen = gap.get("frozenOwners")
    expected_frozen = [
        {"id": story_id, "reasonKey": REASON_BY_ID[story_id]}
        for story_id in ROUTING_FROZEN_OWNERS
    ]
    if frozen != expected_frozen:
        errors.append("routing ownership-gap frozen-owner subtraction drifted")
    for story_id in ROUTING_FROZEN_OWNERS:
        row = row_by_id.get(story_id)
        if not isinstance(row, Mapping) or row.get("category") != "explicit-blocker":
            errors.append(f"{story_id}: routing ownership-gap frozen blocker classification drifted")

    inventory, inventory_errors = _inventory_index(root, "9router")
    errors.extend(inventory_errors)
    if inventory:
        path_patterns = []
        generated = gap.get("generatedContractSearch")
        if not isinstance(generated, Mapping):
            errors.append("routing ownership-gap generated-contract search record is missing")
        else:
            if generated.get("inventory") != "sources/inventory/9router.jsonl" or generated.get("matches") != []:
                errors.append("routing ownership-gap generated-contract search result drifted")
            for pattern in generated.get("pathPatterns", []) if isinstance(generated.get("pathPatterns"), list) else []:
                try:
                    path_patterns.append(re.compile(str(pattern), re.IGNORECASE))
                except re.error as exc:
                    errors.append(f"routing ownership-gap invalid generated-contract path pattern {pattern!r}: {exc}")
            found = sorted(path for path in inventory if any(pattern.search(path) for pattern in path_patterns))
            if found:
                errors.append(f"routing ownership-gap generated-contract absence is stale; matching paths now exist: {found}")

        expected_partition_ids = [
            "account-storage-schema-and-credentials",
            "dashboard-status-and-secret-safe-projection",
            "dashboard-settings-and-runtime-side-effects",
            "routing-runtime-remainder",
        ]
        partitions = gap.get("reviewedPartitions")
        if not isinstance(partitions, list) or [str(item.get("id", "")) for item in partitions if isinstance(item, Mapping)] != expected_partition_ids:
            errors.append("routing ownership-gap reviewed partition inventory drifted")
        else:
            for partition in partitions:
                evidence = partition.get("evidence") if isinstance(partition, Mapping) else None
                if not isinstance(evidence, list) or not evidence:
                    errors.append(f"routing ownership-gap partition lacks pinned evidence: {partition.get('id')}")
                    continue
                if not partition.get("sideEffects") or not str(partition.get("ownershipBlocker", "")).strip():
                    errors.append(f"routing ownership-gap partition lacks side-effect/ownership blocker detail: {partition.get('id')}")
                for item in evidence:
                    if not isinstance(item, Mapping):
                        errors.append("routing ownership-gap partition evidence must be an object")
                        continue
                    source_path = str(item.get("path", ""))
                    inventory_row = inventory.get(source_path)
                    if inventory_row is None:
                        errors.append(f"routing ownership-gap evidence escaped pinned inventory: {source_path}")
                        continue
                    if inventory_row.get("commit") != locked_commit or inventory_row.get("blob") != item.get("blobSha"):
                        errors.append(f"routing ownership-gap evidence pin drifted: {source_path}")
                    reviewed_lines = item.get("reviewedLines")
                    if (
                        not isinstance(reviewed_lines, Mapping)
                        or not isinstance(reviewed_lines.get("from"), int)
                        or not isinstance(reviewed_lines.get("to"), int)
                        or reviewed_lines["from"] < 1
                        or reviewed_lines["to"] < reviewed_lines["from"]
                    ):
                        errors.append(f"routing ownership-gap evidence lacks reviewed line range: {source_path}")

    candidates = gap.get("candidateFragments")
    expected_candidate_ids = ["provider-status-classification", "routing-runtime-remainder"]
    if not isinstance(candidates, list) or [str(item.get("id", "")) for item in candidates if isinstance(item, Mapping)] != expected_candidate_ids:
        errors.append("routing ownership-gap candidate fragment set drifted")
    else:
        for candidate in candidates:
            if candidate.get("ownershipEstablished") is not False:
                errors.append(f"routing candidate cannot become owned from singleton arithmetic: {candidate.get('id')}")
            for key in ("inputs", "outputs", "failureSemantics", "stateLifetime", "resourceLifetime"):
                if not str(candidate.get(key, "")).strip():
                    errors.append(f"routing candidate {candidate.get('id')} lacks explicit {key}")
            if not candidate.get("disqualifiers"):
                errors.append(f"routing candidate {candidate.get('id')} lacks ownership disqualifiers")

    adjacent = gap.get("adjacentSingletonChecks")
    if not isinstance(adjacent, list) or [str(item.get("storyId", "")) for item in adjacent if isinstance(item, Mapping)] != ["EXT-005", "WEB-004"]:
        errors.append("routing ownership-gap adjacent singleton check set drifted")
    else:
        opencode_inventory, opencode_inventory_errors = _inventory_index(root, "opencode")
        errors.extend(opencode_inventory_errors)
        requirements = _load(root / "requirements/user-requirements.json")
        requirement_by_id = {
            str(item.get("id", "")): item
            for item in requirements.get("requirements", [])
            if isinstance(item, Mapping)
        }
        evidence_rows = []
        for evidence_path in (root / "sources/evidence.json", root / "sources/disc-003-evidence.json"):
            evidence_rows.extend(_load(evidence_path).get("sources", []))
        evidence_index = {str(item.get("id", "")): item for item in evidence_rows if isinstance(item, Mapping)}
        for item in adjacent:
            story_id = str(item.get("storyId", ""))
            if item.get("ownershipEstablished") is not False or not str(item.get("closure", "")).strip():
                errors.append(f"{story_id}: adjacent singleton ownership must remain unresolved with explicit closure criteria")
            live_surface_ids = sorted(
                str(rule.get("id"))
                for rule in rule_rows
                if story_id in rule.get("featureIds", [])
            )
            if item.get("surfaceIds") != live_surface_ids:
                errors.append(f"{story_id}: adjacent singleton surface signature drifted")
            ledger_row = row_by_id.get(story_id)
            if not isinstance(ledger_row, Mapping) or ledger_row.get("category") != "unresolved-decomposition":
                errors.append(f"{story_id}: adjacent singleton classification drifted")
                continue
            if ledger_row.get("taskCard") is not None or ledger_row.get("worklog") is not None:
                errors.append(f"{story_id}: adjacent singleton gained task evidence and must be deliberately reconciled")

            if story_id == "EXT-005":
                requirement_ids = item.get("requirementIds")
                if requirement_ids != ["REQ-005"] or ledger_row.get("requirementIds") != ["REQ-005"]:
                    errors.append("EXT-005 adjacent singleton requirement binding drifted")
                req = requirement_by_id.get("REQ-005")
                if not isinstance(req, Mapping) or item.get("requirementTaskSplit") != req.get("tasks"):
                    errors.append("EXT-005 adjacent singleton lost the live REQ-005 multi-task split")
                refs = item.get("pinnedEvidence")
                if not isinstance(refs, list) or not refs:
                    errors.append("EXT-005 adjacent singleton lacks pinned decomposition evidence")
                else:
                    for ref in refs:
                        source_path = str(ref.get("path", "")) if isinstance(ref, Mapping) else ""
                        inventory_row = opencode_inventory.get(source_path)
                        if inventory_row is None or inventory_row.get("commit") != "95daf90670b7c039c436c85537da5fbfe2205b41" or inventory_row.get("blob") != ref.get("blobSha"):
                            errors.append(f"EXT-005 adjacent singleton evidence pin drifted: {source_path}")
            elif story_id == "WEB-004":
                expected_ids = [
                    "OC-SERVER-API", "OC-CONTROL-PLANE-MOVE", "OC-CONTROL-PLANE-HANDLER",
                    "OC-CONTROL-PLANE-TEST", "OC-CLIENT-CONTRACT-TEST", "OC-HTTPAPI-ROUTE-SPEC",
                    "OC-ENTERPRISE-SHARE", "OC-FUNCTION-REMOTE",
                ]
                if item.get("pinnedEvidenceIds") != expected_ids:
                    errors.append("WEB-004 adjacent singleton evidence set drifted")
                for evidence_id in expected_ids:
                    source = evidence_index.get(evidence_id)
                    if source is None or source.get("commit") != "95daf90670b7c039c436c85537da5fbfe2205b41":
                        errors.append(f"WEB-004 adjacent singleton lost pinned evidence: {evidence_id}")
                enterprise_gap = root / "sources/enterprise-remote-spec-gap.json"
                if not enterprise_gap.is_file() or _load(enterprise_gap).get("status") != "searched-no-qualifying-in-surface-spec":
                    errors.append("WEB-004 adjacent singleton must retain the enterprise-remote missing-spec guard")

    history = gap.get("historyReview")
    if not isinstance(history, Mapping) or history.get("state") != "locked-checkout-grafted-at-pinned-commit":
        errors.append("routing ownership-gap history-review limitation drifted")
    if not isinstance(gap.get("closureCriteria"), list) or len(gap.get("closureCriteria", [])) != 4:
        errors.append("routing ownership-gap closure criteria drifted")
    return errors


def operations_ownership_gap_errors(rows: list[object], root: pathlib.Path = ROOT) -> list[str]:
    """Keep broad OpenCode operations surfaces from becoming task ownership by subtraction."""
    errors: list[str] = []
    gap_path = root / "sources/operations-ownership-gap.json"
    if not gap_path.is_file():
        return ["missing machine-checkable operations ownership-gap record"]
    try:
        gap = _load(gap_path)
    except (json.JSONDecodeError, OSError) as exc:
        return [f"invalid operations ownership-gap record: {exc}"]

    if gap.get("schemaVersion") != 1:
        errors.append("operations ownership-gap schemaVersion must be 1")
    if gap.get("status") != "source-reviewed-no-exact-task-owner":
        errors.append("operations ownership-gap status drifted")
    if gap.get("repositoryId") != "opencode" or gap.get("repository") != "anomalyco/opencode":
        errors.append("operations ownership-gap repository identity drifted")
    if gap.get("storyIds") != list(OPERATIONS_GAP_STORIES):
        errors.append("operations ownership-gap story set drifted")
    if gap.get("ownershipDecision") != {story_id: None for story_id in OPERATIONS_GAP_STORIES}:
        errors.append("operations ownership must remain unresolved until task-specific source evidence changes")
    if gap.get("unresolvedPartitions") != list(OPERATIONS_UNRESOLVED_PARTITIONS):
        errors.append("operations unresolved decomposition partitions drifted")

    lock = _load(root / "sources/upstream.lock.json")
    locked = next((item for item in lock.get("repositories", []) if item.get("id") == "opencode"), None)
    locked_commit = str(locked.get("commit", "")) if isinstance(locked, Mapping) else ""
    locked_tree = str(locked.get("treeSha", "")) if isinstance(locked, Mapping) else ""
    if not locked_commit or gap.get("commit") != locked_commit:
        errors.append("operations ownership-gap is not bound to the locked OpenCode commit")
    if not locked_tree or gap.get("treeSha") != locked_tree:
        errors.append("operations ownership-gap is not bound to the locked OpenCode tree")

    plan = _load(root / "ralph.json")
    plan_by_id = {
        str(item.get("id", "")): item
        for item in plan.get("userStories", [])
        if isinstance(item, Mapping)
    }
    generic_story = "Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json"
    expected_requirements = {
        "OPS-001": ["REQ-001", "REQ-024", "REQ-032"],
        "OPS-002": [],
        "OPS-003": [],
        "OPS-004": [],
        "OPS-005": [],
        "OPS-006": [],
        "OPS-008": [],
    }
    binding = gap.get("taskBindingState")
    if not isinstance(binding, Mapping) or list(binding) != list(OPERATIONS_GAP_STORIES):
        errors.append("operations ownership-gap task binding set drifted")
        binding = {}
    for story_id in OPERATIONS_GAP_STORIES:
        story = plan_by_id.get(story_id)
        if story is None:
            errors.append(f"operations ownership-gap story disappeared from Ralph: {story_id}")
            continue
        expected_story = "TBD - see source audit" if story_id == "OPS-001" else generic_story
        if story.get("status") != "not-started" or story.get("userStory") != expected_story:
            errors.append(f"{story_id}: operations ownership-gap is stale after Ralph task semantics changed")
        if story.get("requirementIds") != expected_requirements[story_id]:
            errors.append(f"{story_id}: operations ownership-gap requirement binding drifted")
        recorded = binding.get(story_id)
        if not isinstance(recorded, Mapping):
            errors.append(f"{story_id}: operations ownership-gap task binding is missing")
        elif (
            recorded.get("ralphStory") != expected_story
            or recorded.get("requirementIds") != expected_requirements[story_id]
            or recorded.get("taskCard") is not None
            or recorded.get("worklog") is not None
        ):
            errors.append(f"{story_id}: operations ownership-gap task binding drifted")
        if (root / "tasks" / f"{story_id}.md").exists() or (root / "worklog" / f"{story_id}.md").exists():
            errors.append(f"{story_id}: task/worklog appeared; operations ownership gap needs deliberate review")

    rules = _load(root / "sources/behavior-surface-rules.json")
    rule_rows = [item for item in rules.get("rules", []) if isinstance(item, Mapping)]
    rule_by_id = {str(item.get("id", "")): item for item in rule_rows}
    live_signatures = {
        story_id: sorted(
            str(item.get("id"))
            for item in rule_rows
            if story_id in item.get("featureIds", [])
        )
        for story_id in OPERATIONS_GAP_STORIES
    }
    recorded_signatures = gap.get("storySurfaceSignatures")
    if not isinstance(recorded_signatures, Mapping):
        errors.append("operations ownership-gap storySurfaceSignatures must be an object")
    else:
        for story_id, signature in live_signatures.items():
            if recorded_signatures.get(story_id) != signature:
                errors.append(f"{story_id}: operations ownership-gap surface signature drifted from live rules")

    expected_groups = [
        {"id": group_id, "storyIds": list(story_ids), "surfaceSignature": list(signature)}
        for group_id, story_ids, signature in OPERATIONS_EQUIVALENCE_GROUPS
    ]
    if gap.get("equivalenceGroups") != expected_groups:
        errors.append("operations ownership-gap equivalence groups drifted")
    for group_id, story_ids, signature in OPERATIONS_EQUIVALENCE_GROUPS:
        for story_id in story_ids:
            if live_signatures.get(story_id) != list(signature):
                errors.append(f"{group_id}: operations equivalence no longer matches live surface signatures")
                break

    expected_patterns = {
        surface_id: list(rule_by_id.get(surface_id, {}).get("patterns", []))
        for surface_id in ("opencode.configuration-runtime", "opencode.repository-operations")
    }
    if gap.get("surfaceRulePatterns") != expected_patterns:
        errors.append("operations ownership-gap surface rule patterns drifted")

    row_by_id = {
        str(item.get("id", "")): item
        for item in rows
        if isinstance(item, Mapping)
    }
    for story_id in OPERATIONS_GAP_STORIES:
        row = row_by_id.get(story_id)
        if row is None:
            errors.append(f"operations ownership-gap story missing from exhaustion ledger: {story_id}")
            continue
        if row.get("category") != "unresolved-decomposition" or row.get("reasonKey") != "operations-family-not-decomposed":
            errors.append(f"{story_id}: operations ownership-gap classification drifted")
        if row.get("taskCard") is not None or row.get("worklog") is not None or row.get("implementationCommits") != []:
            errors.append(f"{story_id}: local task ownership appeared; operations ownership gap needs deliberate review")
        if row.get("requirementIds") != expected_requirements[story_id]:
            errors.append(f"{story_id}: exhaustion requirement projection drifted from operations ownership gap")
        if sorted(row.get("surfaceIds", [])) != live_signatures[story_id]:
            errors.append(f"{story_id}: exhaustion surface projection drifted from operations ownership gap")

    accepted = gap.get("acceptedSurfaceOwners")
    if not isinstance(accepted, list) or [str(item.get("id", "")) for item in accepted if isinstance(item, Mapping)] != list(OPERATIONS_ACCEPTED_OWNERS):
        errors.append("operations ownership-gap accepted-owner exclusion set drifted")
    else:
        for item in accepted:
            story_id = str(item.get("id", ""))
            story = plan_by_id.get(story_id)
            if not isinstance(story, Mapping) or story.get("status") != "accepted" or item.get("controllerStatus") != "accepted":
                errors.append(f"{story_id}: operations accepted-owner exclusion is no longer controller accepted")
            if not str(item.get("note", "")).strip():
                errors.append(f"{story_id}: operations accepted-owner exclusion lacks disposition")

    frozen = gap.get("frozenCrossSurfaceOwners")
    expected_frozen = [
        {"id": story_id, "reasonKey": REASON_BY_ID[story_id]}
        for story_id in OPERATIONS_FROZEN_OWNERS
    ]
    if frozen != expected_frozen:
        errors.append("operations ownership-gap frozen cross-surface exclusion drifted")
    for story_id in OPERATIONS_FROZEN_OWNERS:
        row = row_by_id.get(story_id)
        if not isinstance(row, Mapping) or row.get("category") != "explicit-blocker" or row.get("reasonKey") != REASON_BY_ID[story_id]:
            errors.append(f"{story_id}: operations frozen cross-surface blocker drifted")

    inventory, inventory_errors = _inventory_index(root, "opencode")
    errors.extend(inventory_errors)
    expected_partition_ids = [
        "configuration-discovery-and-policy",
        "scoped-replayable-state-transforms",
        "location-service-map-cache",
        "global-npm-installation-bootstrap",
        "repository-reference-normalization",
        "repository-cache-materialization",
    ]
    partitions = gap.get("reviewedPartitions")
    if not isinstance(partitions, list) or [str(item.get("id", "")) for item in partitions if isinstance(item, Mapping)] != expected_partition_ids:
        errors.append("operations ownership-gap reviewed partition inventory drifted")
    elif inventory:
        for partition in partitions:
            evidence = partition.get("evidence") if isinstance(partition, Mapping) else None
            if not isinstance(evidence, list) or not evidence:
                errors.append(f"operations ownership-gap partition lacks pinned evidence: {partition.get('id')}")
                continue
            if not partition.get("sideEffects") or not str(partition.get("ownershipBlocker", "")).strip():
                errors.append(f"operations ownership-gap partition lacks side-effect/ownership blocker detail: {partition.get('id')}")
            for item in evidence:
                if not isinstance(item, Mapping):
                    errors.append("operations ownership-gap partition evidence must be an object")
                    continue
                source_path = str(item.get("path", ""))
                inventory_row = inventory.get(source_path)
                if inventory_row is None:
                    errors.append(f"operations ownership-gap evidence escaped pinned inventory: {source_path}")
                    continue
                if inventory_row.get("commit") != locked_commit or inventory_row.get("blob") != item.get("blobSha"):
                    errors.append(f"operations ownership-gap evidence pin drifted: {source_path}")
                if item.get("kind") not in DISC_EVIDENCE_KINDS:
                    errors.append(f"operations ownership-gap evidence has invalid kind: {source_path}")
                reviewed_lines = item.get("reviewedLines")
                if (
                    not isinstance(reviewed_lines, Mapping)
                    or not isinstance(reviewed_lines.get("from"), int)
                    or not isinstance(reviewed_lines.get("to"), int)
                    or reviewed_lines["from"] < 1
                    or reviewed_lines["to"] < reviewed_lines["from"]
                ):
                    errors.append(f"operations ownership-gap evidence lacks reviewed line range: {source_path}")

    remainder = gap.get("unresolvedRulePartitions")
    if not isinstance(remainder, Mapping) or set(remainder) != {"opencode.configuration-runtime", "opencode.repository-operations"}:
        errors.append("operations ownership-gap unresolved rule remainder drifted")
    elif any(not isinstance(items, list) or not items for items in remainder.values()):
        errors.append("operations ownership-gap unresolved rule remainder must stay explicit")

    candidates = gap.get("candidateFragments")
    expected_candidate_ids = ["repository-reference-normalization", "scoped-replayable-state-transforms"]
    if not isinstance(candidates, list) or [str(item.get("id", "")) for item in candidates if isinstance(item, Mapping)] != expected_candidate_ids:
        errors.append("operations ownership-gap candidate fragment set drifted")
    else:
        for candidate in candidates:
            if candidate.get("ownershipEstablished") is not False:
                errors.append(f"operations candidate cannot become owned from residual arithmetic: {candidate.get('id')}")
            for key in ("inputs", "outputs", "failureSemantics", "stateLifetime", "resourceLifetime"):
                if not str(candidate.get(key, "")).strip():
                    errors.append(f"operations candidate {candidate.get('id')} lacks explicit {key}")
            if not candidate.get("disqualifiers"):
                errors.append(f"operations candidate {candidate.get('id')} lacks ownership disqualifiers")

    history = gap.get("historyReview")
    if not isinstance(history, Mapping) or history.get("state") != "locked-checkout-grafted-at-pinned-commit" or not str(history.get("limitation", "")).strip():
        errors.append("operations ownership-gap history-review limitation drifted")
    if not isinstance(gap.get("closureCriteria"), list) or len(gap.get("closureCriteria", [])) != 4:
        errors.append("operations ownership-gap closure criteria drifted")
    return errors


def release_assurance_gap_errors(rows: list[object], root: pathlib.Path = ROOT) -> list[str]:
    """Keep declarative REL rows from becoming invented Rust product ownership."""
    errors: list[str] = []
    gap_path = root / "sources/release-assurance-gap.json"
    if not gap_path.is_file():
        return ["missing machine-checkable release assurance-gap record"]
    try:
        gap = _load(gap_path)
    except (json.JSONDecodeError, OSError) as exc:
        return [f"invalid release assurance-gap record: {exc}"]

    if gap.get("schemaVersion") != 1:
        errors.append("release assurance-gap schemaVersion must be 1")
    if gap.get("status") != "declarative-assurance-no-product-owner" or gap.get("nativeProductOwner") is not False:
        errors.append("release assurance-gap must remain declarative with no native product owner")
    if gap.get("storyIds") != list(RELEASE_ASSURANCE_STORIES):
        errors.append("release assurance-gap story set drifted")
    if gap.get("ownershipDecision") != {story_id: None for story_id in RELEASE_ASSURANCE_STORIES}:
        errors.append("release assurance ownership must remain unresolved until an executable task-specific validator contract exists")
    if gap.get("storySurfaceSignatures") != {story_id: [] for story_id in RELEASE_ASSURANCE_STORIES}:
        errors.append("release assurance-gap must retain empty behavior-surface signatures")
    if gap.get("requiredDeclarativePartitions") != list(RELEASE_ASSURANCE_PARTITIONS):
        errors.append("release assurance-gap declarative partition set drifted")

    plan = _load(root / "ralph.json")
    plan_by_id = {
        str(item.get("id", "")): item
        for item in plan.get("userStories", [])
        if isinstance(item, Mapping)
    }
    expected_requirements = {
        "REL-001": ["REQ-003"],
        "REL-002": ["REQ-004", "REQ-035"],
        "REL-003": ["REQ-035"],
    }
    binding = gap.get("taskBindingState")
    if not isinstance(binding, Mapping) or list(binding) != list(RELEASE_ASSURANCE_STORIES):
        errors.append("release assurance-gap task binding set drifted")
        binding = {}
    row_by_id = {
        str(item.get("id", "")): item
        for item in rows
        if isinstance(item, Mapping)
    }
    for story_id in RELEASE_ASSURANCE_STORIES:
        story = plan_by_id.get(story_id)
        if not isinstance(story, Mapping):
            errors.append(f"release assurance story disappeared from Ralph: {story_id}")
            continue
        if story.get("status") != "not-started" or story.get("userStory") != "TBD - see source audit":
            errors.append(f"{story_id}: release assurance gap is stale after Ralph semantics changed")
        if story.get("requirementIds") != expected_requirements[story_id]:
            errors.append(f"{story_id}: release assurance requirement binding drifted")
        recorded = binding.get(story_id)
        if not isinstance(recorded, Mapping) or (
            recorded.get("ralphStory") != "TBD - see source audit"
            or recorded.get("requirementIds") != expected_requirements[story_id]
            or recorded.get("taskCard") is not None
            or recorded.get("worklog") is not None
        ):
            errors.append(f"{story_id}: release assurance task binding drifted")
        if (root / "tasks" / f"{story_id}.md").exists() or (root / "worklog" / f"{story_id}.md").exists():
            errors.append(f"{story_id}: task/worklog appeared; release assurance gap needs deliberate review")
        row = row_by_id.get(story_id)
        if not isinstance(row, Mapping):
            errors.append(f"release assurance story missing from exhaustion ledger: {story_id}")
            continue
        if row.get("category") != "unresolved-decomposition" or row.get("reasonKey") != "release-assurance-not-product-owner":
            errors.append(f"{story_id}: release assurance classification drifted")
        if row.get("surfaceIds") != [] or row.get("evidenceIds") != []:
            errors.append(f"{story_id}: release assurance unexpectedly gained product-surface evidence")
        if row.get("requirementIds") != expected_requirements[story_id]:
            errors.append(f"{story_id}: exhaustion requirement projection drifted from release assurance gap")

    requirements = _load(root / "requirements/user-requirements.json")
    requirement_by_id = {
        str(item.get("id", "")): item
        for item in requirements.get("requirements", [])
        if isinstance(item, Mapping)
    }
    topology = gap.get("requirementTopology")
    expected_requirement_ids = ("REQ-003", "REQ-004", "REQ-035")
    if not isinstance(topology, Mapping) or list(topology) != list(expected_requirement_ids):
        errors.append("release assurance-gap requirement topology set drifted")
    else:
        for requirement_id in expected_requirement_ids:
            live = requirement_by_id.get(requirement_id)
            recorded = topology.get(requirement_id)
            if not isinstance(live, Mapping) or not isinstance(recorded, Mapping):
                errors.append(f"release assurance-gap missing requirement topology: {requirement_id}")
                continue
            if recorded.get("requirement") != live.get("requirement") or recorded.get("tasks") != live.get("tasks"):
                errors.append(f"release assurance-gap requirement topology drifted: {requirement_id}")

    expected_policy_paths = ["PLAN.md", "docs/TDD.md", "docs/SECURITY.md"]
    policy = gap.get("localPolicyEvidence")
    if not isinstance(policy, list) or [str(item.get("path", "")) for item in policy if isinstance(item, Mapping)] != expected_policy_paths:
        errors.append("release assurance-gap local policy evidence set drifted")
    else:
        for item in policy:
            path = root / str(item.get("path", ""))
            if not path.is_file():
                errors.append(f"release assurance-gap policy evidence missing: {item.get('path')}")
                continue
            actual_hash = hashlib.sha256(path.read_bytes()).hexdigest()
            if item.get("sha256") != actual_hash:
                errors.append(f"release assurance-gap policy evidence hash drifted: {item.get('path')}")
            reviewed_lines = item.get("reviewedLines")
            if (
                not isinstance(reviewed_lines, Mapping)
                or not isinstance(reviewed_lines.get("from"), int)
                or not isinstance(reviewed_lines.get("to"), int)
                or reviewed_lines["from"] < 1
                or reviewed_lines["to"] < reviewed_lines["from"]
                or reviewed_lines["to"] > len(path.read_text(encoding="utf-8").splitlines())
            ):
                errors.append(f"release assurance-gap policy evidence line range drifted: {item.get('path')}")
            if not str(item.get("conclusion", "")).strip():
                errors.append(f"release assurance-gap policy evidence lacks conclusion: {item.get('path')}")

    owned = gap.get("alreadyOwnedRequirementPartitions")
    if not isinstance(owned, list) or [item.get("requirementId") for item in owned if isinstance(item, Mapping)] != list(expected_requirement_ids):
        errors.append("release assurance-gap already-owned requirement partitions drifted")
    elif any(not str(item.get("disposition", "")).strip() for item in owned if isinstance(item, Mapping)):
        errors.append("release assurance-gap already-owned requirement partition lacks disposition")

    candidates = gap.get("candidateValidatorContracts")
    expected_candidate_ids = ["feature-accounting-release-check", "release-safety-and-tdd-check"]
    if not isinstance(candidates, list) or [str(item.get("id", "")) for item in candidates if isinstance(item, Mapping)] != expected_candidate_ids:
        errors.append("release assurance-gap candidate validator set drifted")
    else:
        for candidate in candidates:
            if candidate.get("ownershipEstablished") is not False:
                errors.append(f"release assurance candidate cannot become owned from requirement arithmetic: {candidate.get('id')}")
            for key in ("inputs", "outputs", "failureSemantics", "stateLifetime", "resourceLifetime"):
                if not str(candidate.get(key, "")).strip():
                    errors.append(f"release assurance candidate {candidate.get('id')} lacks explicit {key}")
            if not candidate.get("disqualifiers"):
                errors.append(f"release assurance candidate {candidate.get('id')} lacks ownership disqualifiers")

    if not isinstance(gap.get("closureCriteria"), list) or len(gap.get("closureCriteria", [])) != 4:
        errors.append("release assurance-gap closure criteria drifted")
    return errors


def sync_features_status(root: pathlib.Path = ROOT) -> None:
    """Synchronize only the generated status mirrors in FEATURES.md from Ralph."""
    plan = _load(root / "ralph.json")
    path = root / "FEATURES.md"
    text = path.read_text(encoding="utf-8")
    for row in plan.get("userStories", []):
        story_id = str(row["id"])
        status = str(row["status"])
        text = re.sub(
            rf"(`{re.escape(story_id)}`\s+\()(?:accepted|in-progress|not-started)(\):)",
            rf"\g<1>{status}\g<2>",
            text,
        )
        text = re.sub(
            rf"(\|\s*`{re.escape(story_id)}`\s*\|[^\n]*?\|\s*)(?:accepted|in-progress|not-started)(\s*\|)",
            rf"\g<1>{status}\g<2>",
            text,
        )
    path.write_text(text, encoding="utf-8")


def validate_ledger(document: Mapping[str, object], root: pathlib.Path = ROOT) -> list[str]:
    errors: list[str] = []
    if document.get("schemaVersion") != 1:
        errors.append("backlog exhaustion ledger schemaVersion must be 1")
    if document.get("status") != "in-progress-not-release-evidence":
        errors.append("backlog exhaustion ledger must remain in-progress-not-release-evidence")

    plan = _load(root / "ralph.json")
    plan_by_id = {str(row["id"]): row for row in plan.get("userStories", [])}
    nonaccepted = {story_id for story_id, row in plan_by_id.items() if row.get("status") != "accepted"}
    accepted = set(plan_by_id) - nonaccepted

    rows = document.get("stories")
    if not isinstance(rows, list):
        return [*errors, "backlog exhaustion stories must be a list"]

    ids = [str(row.get("id", "")) for row in rows if isinstance(row, Mapping)]
    duplicates = sorted(story_id for story_id, count in Counter(ids).items() if count > 1)
    if duplicates:
        errors.append(f"backlog exhaustion duplicate classifications: {duplicates}")
    missing = sorted(nonaccepted - set(ids))
    if missing:
        errors.append(f"backlog exhaustion missing non-accepted stories: {missing}")
    extras = sorted(set(ids) - nonaccepted)
    if extras:
        errors.append(f"backlog exhaustion classifies accepted/unknown stories: {extras}")
    if accepted & set(ids):
        errors.append("controller-accepted stories must never appear in the exhaustion ledger")

    expected_sets = {category: set(ids) for category, ids in CATEGORY_IDS.items()}
    actual_sets: dict[str, set[str]] = defaultdict(set)
    for row in rows:
        if isinstance(row, Mapping):
            actual_sets[str(row.get("category", ""))].add(str(row.get("id", "")))
    for category, expected_ids in expected_sets.items():
        if actual_sets.get(category, set()) != expected_ids:
            errors.append(
                f"backlog exhaustion category drift for {category}: "
                f"expected {sorted(expected_ids)}, got {sorted(actual_sets.get(category, set()))}"
            )

    # Validate exact live projection fields without trusting the checked-in ledger.
    try:
        expected = build_expected_ledger(root)
    except ValueError as exc:
        errors.append(str(exc))
        expected = None
    if expected is not None:
        expected_rows = {row["id"]: row for row in expected["stories"]}
        for row in rows:
            if not isinstance(row, Mapping):
                errors.append("backlog exhaustion story row must be an object")
                continue
            story_id = str(row.get("id", ""))
            wanted = expected_rows.get(story_id)
            if wanted is None:
                continue
            for field in (
                "category", "controllerStatus", "requirementIds", "reasonKey", "reopenPolicy",
                "surfaceIds", "evidenceIds", "surfaceEvidenceGaps", "localEvidencePaths", "taskCard", "worklog",
                "taskStatus", "implementationCommits",
            ):
                if row.get(field) != wanted.get(field):
                    errors.append(f"{story_id}: {field} drifted from live source-grounded accounting")
        if document.get("summary") != expected.get("summary"):
            errors.append("backlog exhaustion summary drifted from live Ralph/classification accounting")

    for story_id in CATEGORY_IDS["local-implemented-stale"]:
        task_path = root / "tasks" / f"{story_id}.md"
        worklog_path = root / "worklog" / f"{story_id}.md"
        status = _task_status(task_path)
        if status is None or not status.startswith("IMPLEMENTED"):
            errors.append(f"{story_id}: stale local implementation must have an IMPLEMENTED task card")
        if not worklog_path.is_file():
            errors.append(f"{story_id}: stale local implementation must have a worklog")
        for commit in STALE_IMPLEMENTATION_COMMITS[story_id]:
            result = subprocess.run(
                ["git", "cat-file", "-e", f"{commit}^{{commit}}"], cwd=root,
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False,
            )
            if result.returncode != 0:
                errors.append(f"{story_id}: implementation commit is missing from history: {commit}")

    for story_id in nonaccepted:
        task_path = root / "tasks" / f"{story_id}.md"
        status = _task_status(task_path)
        if status and status.startswith("IMPLEMENTED") and story_id not in CATEGORY_IDS["local-implemented-stale"]:
            errors.append(f"{story_id}: task card says IMPLEMENTED but stale-local accounting was not updated")

    # Every referenced local evidence path must actually exist.
    for row in rows:
        if not isinstance(row, Mapping):
            continue
        story_id = str(row.get("id", ""))
        for rel in row.get("localEvidencePaths", []) if isinstance(row.get("localEvidencePaths"), list) else []:
            if not (root / str(rel)).is_file():
                errors.append(f"{story_id}: local evidence path is missing: {rel}")

    lock = _load(root / "sources/upstream.lock.json")
    repositories = {
        str(row["url"]).removeprefix("https://github.com/").removesuffix(".git"): str(row["commit"])
        for row in lock.get("repositories", [])
    }
    evidence_rows = []
    for evidence_path in (root / "sources/evidence.json", root / "sources/disc-003-evidence.json"):
        evidence_rows.extend(_load(evidence_path).get("sources", []))
    evidence_index = {str(row["id"]): row for row in evidence_rows}
    referenced_evidence = {
        str(evidence_id)
        for row in rows if isinstance(row, Mapping)
        for evidence_id in row.get("evidenceIds", []) if isinstance(row.get("evidenceIds"), list)
    }
    for evidence_id in sorted(referenced_evidence):
        evidence_row = evidence_index.get(evidence_id)
        if evidence_row is None:
            errors.append(f"backlog exhaustion references unknown pinned evidence: {evidence_id}")
            continue
        repository = str(evidence_row.get("repository", ""))
        if repository not in repositories:
            errors.append(f"backlog exhaustion evidence {evidence_id} is outside locked repositories")
            continue
        if evidence_row.get("commit") != repositories[repository]:
            errors.append(f"backlog exhaustion evidence {evidence_id} is not at the locked commit")
        if not re.fullmatch(r"[0-9a-f]{40}", str(evidence_row.get("blobSha", ""))):
            errors.append(f"backlog exhaustion evidence {evidence_id} lacks a pinned blob SHA")
        path = evidence_row.get("path")
        if not isinstance(path, str) or not path or pathlib.PurePosixPath(path).is_absolute() or ".." in pathlib.PurePosixPath(path).parts:
            errors.append(f"backlog exhaustion evidence {evidence_id} has unsafe/missing source path")

    # Fail closed if DISC-003 drifts toward acceptance.
    reconciliation = _load(root / "sources/disc-003-reconciliation.json")
    manifest = _load(root / "sources/disc-003-reconciliation.manifest.json")
    source_map = _load(root / "workspaces/DISC-003/source-map.json")
    task_text = (root / "tasks/DISC-003.md").read_text(encoding="utf-8")
    progress_text = (root / "workspaces/DISC-003/progress.md").read_text(encoding="utf-8")
    errors.extend(disc_status_errors(reconciliation, manifest, source_map, task_text, progress_text))
    errors.extend(residual_evidence_kind_errors(rows, reconciliation))
    errors.extend(enterprise_remote_gap_errors(rows, reconciliation, root))
    errors.extend(routing_ownership_gap_errors(rows, reconciliation, root))
    errors.extend(operations_ownership_gap_errors(rows, root))
    errors.extend(release_assurance_gap_errors(rows, root))

    errors.extend(_features_status_errors(root, plan))
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="rewrite the canonical ledger from live repository state")
    parser.add_argument("--sync-features", action="store_true", help="synchronize FEATURES.md status mirrors from ralph.json")
    parser.add_argument("--ledger", type=pathlib.Path, default=LEDGER_PATH)
    args = parser.parse_args()
    if args.write:
        document = build_expected_ledger(ROOT)
        args.ledger.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if args.sync_features:
        sync_features_status(ROOT)
    if not args.ledger.is_file():
        print(f"validate_backlog_exhaustion: missing ledger: {args.ledger}")
        return 1
    document = _load(args.ledger)
    errors = validate_ledger(document, ROOT)
    if errors:
        print(f"validate_backlog_exhaustion: {len(errors)} error(s)")
        for error in errors:
            print(f"  - {error}")
        return 1
    summary = document["summary"]
    counts = summary["classificationCounts"]
    print(
        "validate_backlog_exhaustion: OK  "
        f"stories={summary['storyCount']} accepted={summary['controllerAccepted']} "
        f"stale={counts['local-implemented-stale']} blockers={counts['explicit-blocker']} "
        f"dependency={counts['dependency-constrained']} unresolved={counts['unresolved-decomposition']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
