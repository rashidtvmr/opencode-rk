# GUARD-TRIAGE-3 — 2026-09-16

## Validate results
- `tools/validate_repository.py` exit=1, `validate_backlog_exhaustion: 122 error(s)` (log: /tmp/opencode/validate3.log)
- `tools/validate_plan.py` exit=1, `133 error(s)` (log: /tmp/opencode/plan3.log; 11 extra = unknown task prefixes SYNC/RUN/ACP/WSX/SDK/HEAD)
- `git diff --check` exit=0 (clean)
- `git status --porcelain=v1`: 274 entries = 183 modified + 91 untracked (raw: /tmp/opencode/raw_status3.txt)

## Diff vs TRIAGE-2 (no new guard drift)
- `diff validate2.log validate3.log`: identical (exit 0).
- `diff plan2.log plan3.log`: only line-order swap WEB-007/WEB-008, same 133 errors.
- `diff raw_status.txt raw_status3.txt`: +12 entries only, all expected lane output (see below). No new validator error classes.

## Triage: pre-existing stale vs new drift
- Pre-existing (unchanged): all `FEATURES.md stale statuses` lines (~90 lines) + `backlog exhaustion missing non-accepted` 27 IDs + `classifies accepted/unknown` 44 IDs. Controller `ralph.json` still holds old flags; only controller+verifier flip fixes them. Do NOT edit ralph.json from lanes.
- Carried lane drift (awaiting verifier accept, not regressions): OPS-001..006,008; REL-001..003; SHARE-001..005; EXT-001,002,004,006,009,010,011,012; INT-001,003,005,006,007 (+INT-009 stale, ROUTE-009/010 stale, WEB-006/007/008 IMPLEMENTED-but-accounting-stale).
- PROV-015: `non-accepted story has no validator classification` — validator gap, needs verifier/controller classification, not lane fix.
- Today's fixes (new since TRIAGE-2, +12): ACP bridge + remote sync lanes — `M crates/cli/src/main.rs`, `M crates/tools/src/plugin_builtins.rs`, `?? crates/server/src/{acp_bridge,remote_sync}.rs` + tests, `?? worklog/{ACP-001,ACP-002,EXT-TYPE-UNIFY,WEB-LIVE,WSX-002}.md` (+GUARD-TRIAGE-2.md itself). Consistent with ACP-001/002 + WSX-002 lane work; validator already lists ACP/WSX prefixes as unknown, so no surprise.

## Dirty list (condensed)
- Modified 183: tools 52, sessions 37, server 35, foundation 25, providers 16, agents 6, storage 3, security 3, catalog 3, cli 1, worklog 2 (INTEGRATION-PROPOSAL.md, WEB-006.md).
- Untracked 91: crates/server 10 (acp_bridge, acp_files, remote_sync, sdk_client, sync_log + tests), crates/providers 10 (auth_profile, codex_oauth, prov_017..024), crates/sessions 7, crates/tools 4, worklog 60. Full list: /tmp/opencode/raw_status3.txt.

## What controller must flip (verifier-gated, do NOT edit ralph.json from lanes)
- After lane_gate GREEN per lane, controller flips ralph.json `in-progress/not-started → accepted` for: WEB-001..006 (+007..013 as verified), OPS-001..009, REL-001..003, SHARE-001..005, EXT-001,002,004..006,008..012, INT-001..003,005..007,009,010, AUTO-004..006, PROV-017..024 (+PROV-016), SDK-001, TOOL-016..020, UI-019, SYNC-001/002, RUN-001, ACP-001/002, WSX-001/002, HEAD-001/002.
- Controller also: classify PROV-015; add unknown prefixes (SYNC/RUN/ACP/WSX/SDK/HEAD) to plan task-prefix allowlist or map to crates; reconcile FEATURES.md stale statuses + WEB-006/007/008 accounting + ROUTE-009/010 ownership gaps.
