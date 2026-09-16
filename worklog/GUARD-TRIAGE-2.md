# GUARD-TRIAGE-2 — 2026-09-16

## Validate results
- `tools/validate_repository.py` exit=1, `validate_backlog_exhaustion: 122 error(s)` (log: /tmp/opencode/validate2.log)
- `tools/validate_plan.py` exit=1, `133 error(s)` (11 extra = unknown task prefixes SYNC/RUN/ACP/WSX/SDK/HEAD; log: /tmp/opencode/plan2.log)
- `git diff --check` exit=0 (clean)
- `git status --porcelain=v1`: 262 entries = 181 modified + 81 untracked

## Triage: pre-existing stale vs new drift
- Pre-existing: all `FEATURES.md stale statuses` lines (both logs, ~60-70 lines) + `backlog exhaustion missing non-accepted` 27 IDs + `classifies accepted/unknown` 44 IDs. Controller `ralph.json` still holds those stories at old flags; only controller+verifier flip fixes them. Do NOT edit ralph.json from lanes.
- New drift from today's lanes (task/worklog appeared, ownership-gap needs deliberate review): OPS-001..006,008; REL-001..003; SHARE-001..005; EXT-001,002,004,006,009,010,011,012; INT-001,003,005,006,007 (+INT-009 stale, ROUTE-009/010 stale, WEB-006/007/008 IMPLEMENTED-but-accounting-stale). These are expected lane outputs awaiting verifier accept, not regressions.
- PROV-015: `non-accepted story has no validator classification` — validator gap, needs verifier/controller classification, not lane fix.

## Dirty list (condensed)
- Modified 181: agents 6, catalog 3, foundation 25, providers 16, security 3, server 35, sessions 37, storage 3, tools 51, worklog 2 (INTEGRATION-PROPOSAL.md, WEB-006.md). Full list: /tmp/opencode/raw_status.txt lines 1-181.
- Untracked 81: crates/providers/tests 10 (auth_profile, codex_oauth, prov_017..024), crates/server 6 (acp_files, sdk_client, sync_log + tests), crates/sessions 7 (part_events, runner, tui_info_panel, mcp_status_panel + tests), crates/tools/tests 4 (mcp_bulk_actions, mcp_catalog_search, mcp_lifecycle, mcp_payload_filter), worklog 54 (AUTO-004..006, COMPLIANCE-SWEEP, EXT-*, INT-*, OPS-*, PROV-017-024, REL-*, SDK-001, SHARE-001, UI019-TOOL016-020-SYNC-RUN, WEB-*, etc.). Full list: /tmp/opencode/raw_status.txt lines 182-262.

## What controller must flip (verifier-gated, do NOT edit ralph.json from lanes)
- After lane_gate GREEN per lane, controller flips ralph.json `in-progress/not-started → accepted` for: WEB-001..006 (+007..013 as verified), OPS-001..009, REL-001..003, SHARE-001..005, EXT-001,002,004..006,008..012, INT-001..003,005..007,009,010, AUTO-004..006, PROV-017..024 (+PROV-016), SDK-001, TOOL-016..020, UI-019, SYNC-001/002, RUN-001, ACP-001/002, WSX-001/002, HEAD-001/002.
- Controller also: classify PROV-015; add unknown prefixes (SYNC/RUN/ACP/WSX/SDK/HEAD) to plan task-prefix allowlist or map to crates; reconcile FEATURES.md stale statuses + WEB-006/007/008 accounting + ROUTE-009/010 ownership gaps.
