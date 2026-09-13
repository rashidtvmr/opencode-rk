#!/usr/bin/env python3
"""Fail-closed DISC-003 reconciliation validator."""
from __future__ import annotations
import argparse, hashlib, json, os, pathlib, re, sys
from collections import Counter
from collections.abc import Mapping

ROOT = pathlib.Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path: sys.path.insert(0, str(ROOT))
from tools.inventory import sha256_file  # noqa: E402

SHA = re.compile(r"[0-9a-f]{40}")
STATES = {"reconciled", "partial", "queued"}
IMPL = {"implemented-v2", "shared-legacy", "partial", "planned", "reference-implemented", "unresolved"}
KINDS = ("source", "caller", "test", "spec")


def _safe_path(value: object) -> bool:
    if not isinstance(value, str) or not value: return False
    p = pathlib.PurePosixPath(value)
    return not p.is_absolute() and ".." not in p.parts and "\\" not in value


def _index(rows, key="id"):
    out = {}
    for row in rows:
        if not isinstance(row, Mapping) or key not in row: continue
        rid = str(row[key])
        if rid in out: raise ValueError(f"Duplicate {key}: {rid}")
        out[rid] = row
    return out


def expand_reconciliation(document: Mapping[str, object], rules: Mapping[str, object]) -> dict[str, object]:
    by_rule = _index(rules.get("rules", []))
    reviewed = _index(document.get("reviewedSurfaces", []))
    surfaces = []
    for sid in document.get("queuedSurfaceIds", []):
        rule = by_rule.get(str(sid), {})
        surfaces.append({
            "id": str(sid), "kind": rule.get("kind"), "repository": (rule.get("repositories") or [None])[0],
            "featureIds": list(rule.get("featureIds", [])), "reviewState": "queued", "implementationStatus": "unresolved",
            "evidence": {k: [] for k in KINDS},
            "finding": "DISC-002 candidate retained without promotion; exact caller/test/spec reconciliation is still required.",
            "unresolved": ["Inspect exact pinned source and record source, caller, test, and spec evidence before assigning implementation status."],
        })
    for sid, row in reviewed.items():
        rule = by_rule.get(sid, {})
        surfaces.append({**row, "kind": rule.get("kind"), "repository": (rule.get("repositories") or [None])[0], "featureIds": list(rule.get("featureIds", []))})
    return {**document, "surfaces": surfaces}


def validate_reconciliation(document, rules, evidence, plan, repository_names=None):
    errors, repository_names = [], repository_names or {}
    if document.get("schemaVersion") != 1: errors.append("Unsupported DISC-003 reconciliation schema")
    if document.get("status") != "in-progress-not-release-evidence": errors.append("DISC-003 reconciliation must remain explicitly in-progress")
    document = expand_reconciliation(document, rules)
    by_rule, by_evidence = _index(rules.get("rules", [])), _index(evidence.get("sources", []))
    task_ids = {str(x["id"]) for x in plan.get("userStories", []) if isinstance(x, Mapping) and "id" in x}
    seen = set()
    for row in document.get("surfaces", []):
        sid = str(row.get("id", ""))
        if sid in seen: errors.append(f"Duplicate reconciled surface: {sid}"); continue
        seen.add(sid); rule = by_rule.get(sid)
        if not rule: errors.append(f"Reconciliation references unknown candidate surface: {sid}"); continue
        if row.get("kind") != rule.get("kind"): errors.append(f"{sid}: kind differs from DISC-002 candidate")
        if sorted(row.get("featureIds", [])) != sorted(rule.get("featureIds", [])): errors.append(f"{sid}: feature scope differs from DISC-002 candidate")
        repo = row.get("repository")
        if repo not in set(rule.get("repositories", [])): errors.append(f"{sid}: repository is not allowed by candidate rule")
        state, impl = row.get("reviewState"), row.get("implementationStatus")
        if state not in STATES: errors.append(f"{sid}: invalid review state {state!r}")
        if impl not in IMPL: errors.append(f"{sid}: invalid implementation status {impl!r}")
        if not str(row.get("finding", "")).strip(): errors.append(f"{sid}: missing reconciliation finding")
        emap = row.get("evidence")
        if not isinstance(emap, Mapping): errors.append(f"{sid}: evidence must be an object"); continue
        for kind in KINDS:
            refs = emap.get(kind)
            if not isinstance(refs, list): errors.append(f"{sid}: evidence.{kind} must be a list"); continue
            for ref in refs:
                item = by_evidence.get(str(ref.get("evidenceId", ""))) if isinstance(ref, Mapping) else None
                if not item: errors.append(f"{sid}: unknown evidence id {ref.get('evidenceId') if isinstance(ref, Mapping) else ''!r}"); continue
                eid = item["id"]
                if item.get("repository") != repository_names.get(str(repo), str(repo)): errors.append(f"{sid}: evidence {eid} belongs to another repository")
                if not _safe_path(item.get("path")): errors.append(f"{sid}: evidence {eid} has unsafe/missing path")
                if not isinstance(item.get("blobSha"), str) or not SHA.fullmatch(item["blobSha"]): errors.append(f"{sid}: evidence {eid} lacks pinned blob SHA")
        unresolved = row.get("unresolved")
        if not isinstance(unresolved, list): errors.append(f"{sid}: unresolved must be a list"); unresolved = []
        if state == "reconciled":
            for kind in KINDS:
                if not emap.get(kind): errors.append(f"{sid}: reconciled surface lacks {kind} evidence")
            if impl == "unresolved": errors.append(f"{sid}: reconciled surface cannot have unresolved implementation status")
        elif state == "partial":
            if not emap.get("source"): errors.append(f"{sid}: partial surface lacks pinned source evidence")
            if not unresolved: errors.append(f"{sid}: partial surface must state unresolved work")
        elif state == "queued":
            if impl != "unresolved": errors.append(f"{sid}: queued surface must keep implementation status unresolved")
            if not unresolved: errors.append(f"{sid}: queued surface must state missing evidence")
        for feature in row.get("featureIds", []):
            if feature not in task_ids: errors.append(f"{sid}: unknown target feature {feature}")
    if set(by_rule) - seen: errors.append(f"Missing DISC-002 candidate reconciliations: {sorted(set(by_rule)-seen)}")
    if seen - set(by_rule): errors.append(f"Unexpected DISC-003 reconciliations: {sorted(seen-set(by_rule))}")
    return errors


def summarize(document, rules=None):
    if rules is not None: document = expand_reconciliation(document, rules)
    rows = [x for x in document.get("surfaces", []) if isinstance(x, Mapping)]
    return {"surfaceFamilies": len(rows), "reviewStateCounts": dict(sorted(Counter(str(x.get("reviewState")) for x in rows).items())),
            "implementationStatusCounts": dict(sorted(Counter(str(x.get("implementationStatus")) for x in rows).items())),
            "evidenceReferences": sum(len(v) for x in rows for v in (x.get("evidence") or {}).values() if isinstance(v, list)),
            "unresolvedFindings": sum(len(x.get("unresolved", [])) for x in rows)}


def _json_digest(path):
    value = json.loads(path.read_text(encoding="utf-8"))
    data = json.dumps(value, separators=(",", ":"), sort_keys=True).encode()
    return hashlib.sha256(data).hexdigest()


def build_manifest(reconciliation_path, rules_path, evidence_path, supplemental_evidence_path, plan_path, document):
    rules = json.loads(rules_path.read_text(encoding="utf-8"))
    return {"schemaVersion": 1, "status": "in-progress-not-release-evidence", **summarize(document, rules), "inputs": {
        "reconciliationSha256": sha256_file(reconciliation_path), "surfaceRulesSha256": sha256_file(rules_path),
        "evidenceCatalogSha256": _json_digest(evidence_path), "supplementalEvidenceSha256": _json_digest(supplemental_evidence_path), "planSha256": sha256_file(plan_path)},
        "warning": "DISC-003 reconciliation is review evidence only; it does not replace full pinned-tree inventory, independent review receipts, or acceptance tests."}


def _write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True); tmp = path.with_name(path.name + ".tmp")
    try: tmp.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8"); os.replace(tmp, path)
    finally: tmp.unlink(missing_ok=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__); ap.add_argument("--reconciliation", type=pathlib.Path, default=ROOT/"sources/disc-003-reconciliation.json"); ap.add_argument("--manifest-output", type=pathlib.Path, default=ROOT/"sources/disc-003-reconciliation.manifest.json"); a = ap.parse_args()
    rp, ep, sp, pp, lp = ROOT/"sources/behavior-surface-rules.json", ROOT/"sources/evidence.json", ROOT/"sources/disc-003-evidence.json", ROOT/"ralph.json", ROOT/"sources/upstream.lock.json"
    document, rules, base, extra, plan, lock = [json.loads(p.read_text(encoding="utf-8")) for p in (a.reconciliation, rp, ep, sp, pp, lp)]
    evidence = {"sources": [*base.get("sources", []), *extra.get("sources", [])]}
    names = {str(x["id"]): str(x["url"]).removeprefix("https://github.com/").removesuffix(".git") for x in lock.get("repositories", [])}
    errors = validate_reconciliation(document, rules, evidence, plan, names); result = {"passed": not errors, **summarize(document, rules), "errors": errors}
    if errors: print(json.dumps(result, indent=2)); return 2
    _write(a.manifest_output, build_manifest(a.reconciliation, rp, ep, sp, pp, document)); print(json.dumps({**result, "manifest": str(a.manifest_output)}, indent=2)); return 0

if __name__ == "__main__": raise SystemExit(main())
