#!/usr/bin/env python3
"""Old-vs-new controller reconciler (COORD-008, stdlib only).

Detects the gaps named in tasks/completion/delivery.json COORD-008:
- old controller marks accepted before integration (tools/ralph_loop.py:601-608
  sets status accepted, then finalize_worktree, keeping accepted-but-not-
  integrated with only last_error). New scheduler (tools/completion_scheduler.py:
  189-203) accepts only after verify, integrate, verify_integrated.
- rank-gate synthesis (tools/plan_model.py:193-205 synthesizes rank closure
  only when zero authored dependencyIds exist): adding explicit deps must not
  silently drop that protection.
- rare-prefix coverage: legacy families SYNC/RUN/ACP/WSX/SDK/HEAD must load
  and stay audit-assigned, never fall to the AUD-020 catch-all or vanish.
- flat prd.json export (schemaVersion 1, tasks carry only id/status/userStory/
  testObligations, no deps/evidence) must never count as release evidence;
  contract legacyAcceptedIsReleaseEvidence false (ralph.completion.json).

Pure, bounded (10000 tasks), no processes/network/writes/import side effects.
"""
from __future__ import annotations

# Rank table mirrors tools/plan_model.py:27-49 (HEAD 5af7884). Local copy so
# this checker stays import-safe and cannot drift via shared mutable state.
PHASE_RANK = {
    "DISC": 0, "BASE": 1, "SEC": 1, "DB": 1, "CAT": 2, "PROV": 2,
    "TOOL": 2, "SESS": 2, "AGENT": 3, "ROUTE": 3, "UI": 3, "SHARE": 4,
    "EXT": 4, "INT": 4, "WEB": 5, "OPS": 6, "AUTO": 6, "REL": 6,
    "SYNC": 2, "RUN": 2, "ACP": 2, "WSX": 5, "SDK": 5, "HEAD": 1,
}
PHASE_OVERRIDE = {"AUTO-001": 0, "AUTO-002": 0}

REQUIRED_PREFIXES = ("SYNC", "RUN", "ACP", "WSX", "SDK", "HEAD")
CATCH_ALL_SHARD = "AUD-020"
MAX_TASKS = 10000


class ReconcileError(ValueError):
    """Old/new scope disagreement that must block release, not warn past."""


def prefix_of(task_id: str) -> str:
    if not isinstance(task_id, str) or "-" not in task_id:
        raise ReconcileError(f"invalid task id: {task_id!r}")
    prefix = task_id.split("-", 1)[0]
    if not prefix.isupper() or not prefix.isalpha():
        raise ReconcileError(f"invalid task id: {task_id!r}")
    return prefix


def rank_for(task_id: str) -> int:
    if task_id in PHASE_OVERRIDE:
        return PHASE_OVERRIDE[task_id]
    return PHASE_RANK[prefix_of(task_id)]


def accepted_before_integrated(records: dict[str, dict]) -> list[str]:
    """Task ids with accepted status but no proven integration.

    Mirrors the ralph_loop.py:601-608 gap: status accepted + integrated not
    True (False/None/missing) is the forbidden state the new pipeline
    (scheduler finalize: integrate, then verify_integrated, then accept)
    never produces.
    """
    if not isinstance(records, dict) or len(records) > MAX_TASKS:
        raise ReconcileError("bounded records mapping required")
    bad = []
    for tid, rec in records.items():
        if not isinstance(rec, dict):
            raise ReconcileError(f"{tid}: record must be a mapping")
        if rec.get("status") == "accepted" and rec.get("integrated") is not True:
            bad.append(tid)
    return sorted(bad)


def is_flat_prd_row(row: dict) -> bool:
    """True when a row is a flat prd.json export: no dep/evidence authority."""
    if not isinstance(row, dict):
        return True
    return not any(k in row for k in ("dependencyIds", "deps", "synthesized_deps",
                                      "proof", "artifacts", "testedCommit"))


def prd_bypass_errors(prd_rows: list[dict], *, treat_as_evidence: bool) -> list[str]:
    """Reject flat-prd or legacy-accepted flags posed as release evidence."""
    if not isinstance(prd_rows, list) or len(prd_rows) > MAX_TASKS:
        raise ReconcileError("bounded prd row list required")
    errors = []
    flat = sum(1 for row in prd_rows if is_flat_prd_row(row))
    if flat:
        errors.append(f"flat prd export carries no evidence authority ({flat} flat rows)")
    for row in prd_rows:
        if isinstance(row, dict) and row.get("status") == "accepted" and is_flat_prd_row(row):
            errors.append(f"{row.get('id', '?')}: flat accepted flag is not evidence")
            break
    if treat_as_evidence and prd_rows:
        errors.append("legacyAcceptedIsReleaseEvidence is false: prd/legacy flags cannot grant release")
    return errors


def graph_errors(stories: dict[str, set[str]]) -> list[str]:
    if not isinstance(stories, dict) or not stories or len(stories) > MAX_TASKS:
        raise ReconcileError("nonempty bounded task graph required")
    errors = []
    remaining = {}
    for tid, deps in stories.items():
        if not isinstance(deps, set):
            raise ReconcileError(f"{tid}: deps must be a set")
        for dep in deps - stories.keys():
            errors.append(f"{tid}: unknown dependency {dep}")
        remaining[tid] = deps & stories.keys()
    resolved: set[str] = set()
    while remaining:
        ready = {tid for tid, deps in remaining.items() if deps <= resolved}
        if not ready:
            return errors + ["dependency cycle: " + ", ".join(sorted(remaining))]
        resolved.update(ready)
        for tid in ready:
            remaining.pop(tid)
    return errors


def rank_closure(task_id: str, ranks: dict[str, int]) -> set[str]:
    mine = ranks[task_id]
    return {t for t, r in ranks.items() if r < mine}


def protection_errors(ranks: dict[str, int], authored: dict[str, set[str]]) -> list[str]:
    """Explicit deps must still cover the legacy rank-gate closure.

    plan_model.py:193 synthesizes the full lower-rank closure only when NO
    authored deps exist anywhere. Once any explicit dep appears, synthesis
    stops globally, so each rank>0 task must carry the closure explicitly;
    anything less silently removes protection.
    """
    if set(ranks) != set(authored):
        raise ReconcileError("ranks and authored deps must cover the same tasks")
    if len(ranks) > MAX_TASKS:
        raise ReconcileError("task graph exceeds bound")
    errors = []
    for tid in sorted(ranks):
        missing = rank_closure(tid, ranks) - authored[tid]
        if missing:
            errors.append(f"{tid}: explicit deps drop rank protection for {len(missing)} lower-rank task(s)")
    return errors


def prefix_coverage_errors(legacy_ids: list[str], owners: dict[str, str]) -> list[str]:
    """Every legacy family loads; rare prefixes keep a real audit owner."""
    if len(legacy_ids) > MAX_TASKS:
        raise ReconcileError("legacy id list exceeds bound")
    present = {prefix_of(t) for t in legacy_ids}
    errors = []
    for prefix in REQUIRED_PREFIXES:
        if prefix not in present:
            errors.append(f"legacy prefix {prefix} missing from scope")
        elif owners.get(prefix, CATCH_ALL_SHARD) == CATCH_ALL_SHARD:
            errors.append(f"legacy prefix {prefix} falls to catch-all {CATCH_ALL_SHARD}")
    dupes = {p for p, s in owners.items() if list(owners.values()).count(s) > 1 and p in REQUIRED_PREFIXES}
    _ = dupes  # shared shards allowed (e.g. AUD-010 owns SYNC/RUN/WSX/SDK); no error
    return errors


def entrypoint_verdict(*, old_accepts_before_integrate: bool) -> str:
    """Old loop either enforces integrate-then-accept or must refuse."""
    return "refuse-unsafe-execution" if old_accepts_before_integrate else "same-trusted-policy"


def reconcile(*, records: dict[str, dict], prd_rows: list[dict],
              treat_prd_as_evidence: bool, legacy_ids: list[str],
              owners: dict[str, str], ranks: dict[str, int],
              authored: dict[str, set[str]],
              old_accepts_before_integrate: bool) -> list[str]:
    errors = [f"accepted-before-integrated: {t}" for t in accepted_before_integrated(records)]
    errors += prd_bypass_errors(prd_rows, treat_as_evidence=treat_prd_as_evidence)
    errors += prefix_coverage_errors(legacy_ids, owners)
    errors += graph_errors(authored)
    if not graph_errors(authored):
        errors += protection_errors(ranks, authored)
    if entrypoint_verdict(old_accepts_before_integrate=old_accepts_before_integrate) != "same-trusted-policy":
        errors.append("old controller entrypoint must refuse: accepts before integration")
    return errors


__all__ = [
    "CATCH_ALL_SHARD", "MAX_TASKS", "PHASE_OVERRIDE", "PHASE_RANK",
    "REQUIRED_PREFIXES", "ReconcileError", "accepted_before_integrated",
    "entrypoint_verdict", "graph_errors", "is_flat_prd_row",
    "prefix_coverage_errors", "prefix_of", "protection_errors",
    "prd_bypass_errors", "rank_closure", "rank_for", "reconcile",
]


if __name__ == "__main__":
    # T01: accepted-before-integrated + flat-prd bypass rejected.
    assert accepted_before_integrated({"A": {"status": "accepted", "integrated": True}}) == []
    assert accepted_before_integrated({"A": {"status": "accepted", "integrated": False}}) == ["A"]
    assert accepted_before_integrated({"A": {"status": "accepted"}}) == ["A"]
    assert accepted_before_integrated({"A": {"status": "blocked", "integrated": False}}) == []
    flat = [{"id": "X-001", "status": "not-started", "userStory": "u",
             "testObligations": ["X-001-T01"]}]
    assert any("no evidence authority" in e for e in prd_bypass_errors(flat, treat_as_evidence=False))
    assert prd_bypass_errors([{"id": "X-001", "status": "accepted", "dependencyIds": [],
                               "proof": {}, "testedCommit": "c"}], treat_as_evidence=False) == []
    assert any("cannot grant release" in e
               for e in prd_bypass_errors(flat, treat_as_evidence=True))
    # T02: explicit deps must keep rank closure once synthesis stops.
    ranks = {"A-001": 0, "B-001": 1, "B-002": 1}
    full = {"A-001": set(), "B-001": {"A-001"}, "B-002": {"A-001"}}
    thin = {"A-001": set(), "B-001": {"A-001"}, "B-002": set()}
    assert protection_errors(ranks, full) == []
    assert len(protection_errors(ranks, thin)) == 1
    # T03: graph loads, cycles/missing fail; rare prefixes covered.
    assert graph_errors(full) == []
    assert any("unknown dependency" in e for e in graph_errors({"A": {"Z"}}))
    assert any("cycle" in e for e in graph_errors({"A": {"B"}, "B": {"A"}}))
    legacy = ["SYNC-001", "RUN-001", "ACP-001", "WSX-001", "SDK-001", "HEAD-001"]
    owners = {"SYNC": "AUD-010", "RUN": "AUD-010", "ACP": "AUD-009",
              "WSX": "AUD-010", "SDK": "AUD-010", "HEAD": "AUD-001"}
    assert prefix_coverage_errors(legacy, owners) == []
    assert len(prefix_coverage_errors(["SYNC-001"], owners)) == 5
    assert any("catch-all" in e for e in prefix_coverage_errors(legacy, {}))
    # T04: old entrypoint refuses until fixed.
    assert entrypoint_verdict(old_accepts_before_integrate=True) == "refuse-unsafe-execution"
    assert entrypoint_verdict(old_accepts_before_integrate=False) == "same-trusted-policy"
    assert reconcile(records={"A": {"status": "accepted", "integrated": True}},
                     prd_rows=flat, treat_prd_as_evidence=False, legacy_ids=legacy,
                     owners=owners, ranks=ranks, authored=full,
                     old_accepts_before_integrate=False) != []
    ok_prd = []  # empty export: nothing posed as evidence
    assert reconcile(records={"A": {"status": "accepted", "integrated": True}},
                     prd_rows=ok_prd, treat_prd_as_evidence=False, legacy_ids=legacy,
                     owners=owners, ranks=ranks, authored=full,
                     old_accepts_before_integrate=False) == []
    assert rank_for("AUTO-001") == 0 and rank_for("SYNC-001") == 2
    print("reconcile self-check: OK")
