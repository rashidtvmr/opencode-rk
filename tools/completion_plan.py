#!/usr/bin/env python3
"""Mandatory additive scope reader and structural evidence gate (stdlib only).

This does not run product tests, launch agents, or certify the trustworthiness of
an operator-supplied verifier. --check validates specifications, not completion.
The legacy plan is retained verbatim; historical accepted flags are not evidence.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import sys
from collections import Counter

ROOT = pathlib.Path(__file__).resolve().parents[1]
ID = re.compile(r"[A-Z]+-[0-9]{3}\Z")
SHA = re.compile(r"[0-9a-f]{40}\Z")
HASH = re.compile(r"[0-9a-f]{64}\Z")
RELEASE_GATES = (
    "installed-local", "native-tui", "provider-canary", "full-parity",
    "platform-security", "resources", "hosted-remote", "ios-device", "android-device",
)


class InvalidPlan(ValueError):
    pass


def safe_path(root: pathlib.Path, relative: str) -> pathlib.Path:
    if not isinstance(relative, str) or not relative or "\\" in relative:
        raise InvalidPlan("invalid relative path")
    rel = pathlib.PurePosixPath(relative)
    if rel.is_absolute() or any(p in {".", ".."} for p in relative.split("/")):
        raise InvalidPlan(f"unsafe relative path: {relative}")
    root = root.resolve()
    result = (root / relative).resolve()
    if result == root or root not in result.parents:
        raise InvalidPlan(f"path escapes root: {relative}")
    for part in [root / pathlib.Path(*rel.parts[:i]) for i in range(1, len(rel.parts) + 1)]:
        if part.is_symlink():
            raise InvalidPlan(f"symlink is not an evidence/include file: {relative}")
    return result


def read_json(path: pathlib.Path) -> dict:
    if path.stat().st_size > 4 * 1024 * 1024:
        raise InvalidPlan(f"JSON exceeds 4 MiB: {path}")

    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise InvalidPlan(f"duplicate JSON key {key!r} in {path}")
            result[key] = value
        return result

    value = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique)
    if not isinstance(value, dict):
        raise InvalidPlan(f"expected JSON object: {path}")
    return value


def nonempty_strings(value, label: str, *, empty: bool = False) -> list[str]:
    if not isinstance(value, list) or (not value and not empty):
        raise InvalidPlan(f"{label}: expected {'possibly empty ' if empty else ''}list")
    if any(not isinstance(v, str) or not v.strip() for v in value):
        raise InvalidPlan(f"{label}: expected nonempty strings")
    if len(value) != len(set(value)):
        raise InvalidPlan(f"{label}: duplicates")
    return value


def graph_errors(stories: dict[str, dict]) -> list[str]:
    errors = []
    remaining = {}
    for tid, story in stories.items():
        deps = set(story["deps"])
        for dep in deps - stories.keys():
            errors.append(f"{tid}: unknown dependency {dep}")
        remaining[tid] = deps & stories.keys()
    resolved = set()
    while remaining:
        ready = {tid for tid, deps in remaining.items() if deps <= resolved}
        if not ready:
            errors.append("dependency cycle: " + ", ".join(sorted(remaining)))
            break
        resolved.update(ready)
        for tid in ready:
            remaining.pop(tid)
    return errors


def load(root: pathlib.Path = ROOT, *, require_legacy: bool = True) -> dict:
    manifest = read_json(root / "ralph.completion.json")
    if type(manifest.get("schemaVersion")) is not int or manifest["schemaVersion"] != 1:
        raise InvalidPlan("unsupported completion schema")
    if manifest.get("legacyPlan") != "ralph.json" or manifest.get("legacyRequirements") != "requirements/user-requirements.json":
        raise InvalidPlan("legacy scope paths may not be silently replaced")
    requirements = manifest.get("requirements", {})
    if not isinstance(requirements, dict) or not requirements:
        raise InvalidPlan("missing completion requirements")
    contract = manifest.get("contract", {})
    if contract.get("mandatory") is not True or contract.get("legacyAcceptedIsReleaseEvidence") is not False:
        raise InvalidPlan("mandatory scope and independent evidence policy required")
    includes = nonempty_strings(manifest.get("includes"), "includes")
    audit_tests = nonempty_strings(manifest.get("auditTests"), "auditTests")
    groups = {path: read_json(safe_path(root, path)) for path in includes}
    rows = []
    audits = manifest.get("auditShards", [])
    if not isinstance(audits, list) or not audits:
        raise InvalidPlan("missing audit shards")
    for shard in audits:
        tid = shard["id"]
        rows.append({"id": tid, "title": shard["title"], "deps": [],
                     "paths": [f"sources/completion/audits/{tid}.json"],
                     "journey": f"Reconcile {shard['title']} using pinned source, runtime evidence and required repair children.",
                     "tests": list(audit_tests), "requirementIds": ["COMP-003"], "source": shard})
    for path, group in groups.items():
        for story in group.get("stories", []):
            rows.append({**story, "requirementIds": story.get("requirementIds", group.get("requirementIds")), "cardSource": path})
    stories = {}
    for story in rows:
        tid = story.get("id", "")
        if not isinstance(tid, str) or not ID.fullmatch(tid) or tid in stories:
            raise InvalidPlan(f"invalid/duplicate task ID: {tid}")
        for key in ("title", "journey"):
            if not isinstance(story.get(key), str) or not story[key].strip():
                raise InvalidPlan(f"{tid}: missing {key}")
        nonempty_strings(story.get("deps"), f"{tid}.deps", empty=True)
        nonempty_strings(story.get("paths"), f"{tid}.paths")
        tests = nonempty_strings(story.get("tests"), f"{tid}.tests")
        if len(tests) < 5:
            raise InvalidPlan(f"{tid}: fewer than five concrete test scenarios")
        rids = nonempty_strings(story.get("requirementIds"), f"{tid}.requirementIds")
        if not set(rids) <= requirements.keys():
            raise InvalidPlan(f"{tid}: unknown requirement")
        for path in story["paths"]:
            # Fork namespace denotes a separate repository, not a local path.
            safe_path(root, path)
        story["status"] = "not-started"
        story["mandatory"] = True
        story["testObligations"] = [f"{tid}-T{i:02}" for i in range(1, len(tests) + 1)]
        stories[tid] = story
    errors = graph_errors(stories)
    if errors:
        raise InvalidPlan("; ".join(errors))
    cfg = read_json(root / "config/completion-controller.json")
    for key in ("targetWorkers", "maxWorkers", "maxUnverifiedCandidates", "maxHeavyValidations", "maxAttemptsPerTask"):
        if type(cfg.get(key)) is not int or cfg[key] <= 0:
            raise InvalidPlan(f"invalid controller setting {key}")
    if not cfg["targetWorkers"] <= cfg["maxWorkers"] <= 20 or cfg["maxHeavyValidations"] != 1 or cfg["maxAttemptsPerTask"] > 3:
        raise InvalidPlan("worker, validation or retry safety cap exceeded")
    if cfg.get("allowBudgetIncrease") is not False:
        raise InvalidPlan("implicit provider budget increase forbidden")
    legacy = read_json(root / manifest["legacyPlan"]) if require_legacy else {"userStories": []}
    old_requirements = read_json(root / manifest["legacyRequirements"]) if require_legacy else {"requirements": []}
    old_rows = legacy.get("userStories", [])
    old_ids = [row["id"] for row in old_rows]
    if require_legacy and not old_ids:
        raise InvalidPlan("legacy plan is empty")
    if len(old_ids) != len(set(old_ids)) or set(old_ids) & stories.keys():
        raise InvalidPlan("legacy task collision/duplicate")
    for req in old_requirements.get("requirements", []):
        if not set(req.get("tasks", [])) <= set(old_ids):
            raise InvalidPlan(f"legacy requirement has missing tasks: {req.get('id')}")
    for row in old_rows:
        nonempty_strings(row.get("testObligations"), f"legacy {row['id']} obligations")
    payload = {"manifest": manifest, "groups": groups, "config": cfg, "legacy": legacy, "legacyRequirements": old_requirements}
    digest = hashlib.sha256(json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    return {**payload, "stories": stories, "legacyStories": {r["id"]: r for r in old_rows}, "digest": digest}


def legacy_report(plan: dict) -> dict:
    assignments = {s["id"]: [] for s in plan["manifest"]["auditShards"]}
    owners = {}
    for shard in plan["manifest"]["auditShards"]:
        for prefix in shard["legacyPrefixes"]:
            if prefix in owners:
                raise InvalidPlan(f"duplicate legacy prefix owner: {prefix}")
            owners[prefix] = shard["id"]
    for tid in plan["legacyStories"]:
        assignments[owners.get(tid.split("-", 1)[0], "AUD-020")].append(tid)
    rows = list(plan["legacyStories"].values())
    return {"legacyCount": len(rows), "recordedStatusesNotEvidence": dict(Counter(r.get("status", "unknown") for r in rows)),
            "requiresRevalidation": sorted(plan["legacyStories"]),
            "tbdStories": [r["id"] for r in rows if "TBD" in r.get("userStory", "")],
            "emptyDependencies": [r["id"] for r in rows if not r.get("dependencyIds")], "auditAssignments": assignments}


def file_hash(path: pathlib.Path) -> str:
    if path.stat().st_size > 128 * 1024 * 1024:
        raise InvalidPlan("evidence artifact exceeds 128 MiB")
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def proof_errors(proof: dict, root: pathlib.Path, revision: str, required_tests: list[str]) -> list[str]:
    if not isinstance(proof, dict):
        return ["missing proof"]
    errors = []
    if proof.get("status") != "accepted" or proof.get("testedCommit") != revision:
        errors.append("missing accepted proof on exact release revision")
    count = proof.get("testCount")
    if type(count) is not int or count < max(1, len(required_tests)):
        errors.append("zero/insufficient real test count")
    if not set(required_tests) <= set(proof.get("testObligations", [])):
        errors.append("missing test obligations")
    if not HASH.fullmatch(str(proof.get("frozenTestsSha256", ""))):
        errors.append("missing frozen test hash")
    if not proof.get("verifier") or not proof.get("implementer") or proof["verifier"] == proof["implementer"]:
        errors.append("independent verifier identity required")
    artifacts = proof.get("artifacts", [])
    if not isinstance(artifacts, list) or not 1 <= len(artifacts) <= 64:
        return errors + ["missing/bounded artifact list required"]
    if required_tests and not {"red", "green", "integrated-green"} <= {a.get("kind") for a in artifacts if isinstance(a, dict)}:
        errors.append("missing RED/GREEN/integrated evidence")
    for artifact in artifacts:
        try:
            path = safe_path(root, artifact["path"])
            if not HASH.fullmatch(str(artifact.get("sha256", ""))) or file_hash(path) != artifact["sha256"]:
                errors.append("artifact hash mismatch")
        except (OSError, KeyError, TypeError, InvalidPlan) as exc:
            errors.append(f"invalid artifact: {exc}")
    return errors


def release_errors(plan: dict, evidence_root: pathlib.Path, revision: str, repo_root: pathlib.Path = ROOT) -> list[str]:
    if not SHA.fullmatch(revision):
        return ["exact 40-character release commit required"]
    evidence_root, repo_root = evidence_root.resolve(), repo_root.resolve()
    if evidence_root == repo_root or repo_root in evidence_root.parents:
        return ["trusted evidence must be outside the worker repository"]
    receipt = read_json(evidence_root / "release.json")
    errors = []
    if receipt.get("planSha256") != plan["digest"] or receipt.get("commit") != revision:
        errors.append("stale plan/revision evidence")
    all_tasks = {**plan["legacyStories"], **plan["stories"]}
    for tid, story in all_tasks.items():
        for error in proof_errors(receipt.get("tasks", {}).get(tid), evidence_root, revision, story["testObligations"]):
            errors.append(f"{tid}: {error}")
    for gate in RELEASE_GATES:
        for error in proof_errors(receipt.get("gates", {}).get(gate), evidence_root, revision, []):
            errors.append(f"{gate}: {error}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--check-spec", action="store_true", help="additions only; not full repository validation")
    mode.add_argument("--ready", action="store_true", help="initial audit roots, not live leases or accepted state")
    mode.add_argument("--audit-legacy", action="store_true")
    mode.add_argument("--card")
    mode.add_argument("--export", action="store_true")
    mode.add_argument("--release", action="store_true")
    parser.add_argument("--evidence-root", type=pathlib.Path)
    args = parser.parse_args()
    try:
        plan = load(require_legacy=not args.check_spec)
        if args.release:
            if not args.evidence_root:
                raise InvalidPlan("--release requires --evidence-root outside the repository")
            revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True, timeout=10).strip()
            errors = release_errors(plan, args.evidence_root, revision)
            if errors:
                print("RELEASE BLOCKED:\n" + "\n".join(errors[:100]))
                print(f"total errors={len(errors)}")
                return 1
            print(f"Evidence structure validated for {revision}; trusted execution and platform gates remain the verifier's responsibility.")
        elif args.ready:
            print("\n".join(tid for tid, s in plan["stories"].items() if not s["deps"]))
        elif args.audit_legacy:
            print(json.dumps(legacy_report(plan), indent=2))
        elif args.card:
            story = plan["stories"].get(args.card)
            if story is None:
                raise InvalidPlan("unknown completion task")
            print(json.dumps({"contract": plan["manifest"]["contract"], "task": story}, indent=2))
        elif args.export:
            print(json.dumps({"schemaVersion": 1, "note": "Legacy statuses are historical, not release proof.",
                              "legacyStories": list(plan["legacyStories"].values()), "completionStories": list(plan["stories"].values()), "planSha256": plan["digest"]}, indent=2))
        else:
            print(f"SPEC OK: additions={len(plan['stories'])}, legacy={len(plan['legacyStories'])}, scenarios={sum(len(s['tests']) for s in plan['stories'].values())}; NOT product acceptance")
        return 0
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as exc:
        print(f"completion_plan: FAIL: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
