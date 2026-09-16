# GUARD-TRIAGE-4 (2026-09-16)

## Validate
- `python3 tools/validate_repository.py` → log `/tmp/opencode/validate4.log`
- backlog exhaustion: **122 error(s)**, `validate_repository: FAIL backlog exhaustion exit=1` (shell EXIT:0 = tee mask; inner exit=1)
- protection policy: OK (owner @rashidtvmr, 56 paths, platform_verified=false)
- ruleset import: OK (planning, enforcement=disabled); fixture self-test passed (not platform evidence)
- `python3 tools/validate_plan.py` → `/tmp/opencode/plan4.log`, **133 error(s)**, EXIT:1 (extra 11 vs repo guard: `Unknown task prefix SYNC-001/SYNC-002` + related)
- `git diff --check`: clean (EXIT:0)
- `git status --porcelain`: **306 entries = 202 modified + 104 untracked** (snapshot /tmp/opencode/status5.txt; counts drift ±3 between runs — lanes active concurrently)
  - untracked split: **36 product + 68 worklog**

## Triage: pre-existing stale vs today's new drift
Pre-existing stale FEATURES (controller=accepted, FEATURES.md behind):
- ROUTE-001..012 (12), REL-004, UI-001..018 (18), AUTO-003/007, EXT-003/007/013, INT-004/008, OPS-010, PROV-014, SESS-019/020, TOOL-006/009/015. Fix = FEATURES.md status sync only, no code.
- Stale ownership-gap ledger entries (semantics changed, need deliberate review not auto-flip): OPS-001..006/008, REL-001..003, SHARE-001..005, EXT-004/006/009..012, INT-001/003/005..007/009, ROUTE-009/010.

Today's new drift (code/worklogs landed, controller NOT flipped — do NOT edit ralph.json here):
- ACP new files (NOT wired): `crates/server/src/acp_bridge.rs`, `acp_files.rs` + tests — grep `acp_bridge|sdk_client|remote_sync` in `crates/server/src/lib.rs` = 0 hits. Needs lib.rs wiring lane + ACP-001/002 status flip (currently not-started).
- SDK new files (NOT wired): `crates/server/src/sdk_client.rs`, `sdk_spawns.rs` + tests. Same wiring gap. SDK-001/002 currently not-started.
- WSX/SYNC/HEAD new files: `remote_sync.rs`, `sync_log.rs` + tests; `part_events.rs`, `runner.rs`, `tui_info_panel.rs` + tests; SYNC-001/002 unknown prefix in plan validator — needs validator prefix registration before flip.
- PROV-017..024 fix: 8 new test files (`prov_017_claude_oauth` … `prov_024_provider_contracts`) + `auth_profile.rs`, `codex_oauth.rs`; PROV-015..024 currently not-started, PROV-015 has no validator classification.
- AUTO delegation +78: `crates/agents/src/delegation_lane.rs` (+83/-20, BrokerDenied + reflow), tests touched; AUTO-004/005/006 in-progress — flip needs lane-gate GREEN.
- ACP wiring lib.rs: confirmed MISSING (see grep above).
- WEB live lib.rs+cli: `crates/sessions/src/lib.rs` +69/-? (ForkProvenance re-export, reflow), `crates/cli/src/session_export.rs` + test untracked NOT wired (no `session_export` ref in `crates/cli/src/`); WEB-001..005 in-progress, WEB-006..008 IMPLEMENTED-but-ledger-stale.

## New-file list (36 product, non-worklog untracked)
cli: session_export.rs + test. providers/tests: auth_profile, codex_oauth, prov_017..024 (8). server/src: acp_bridge, acp_files, remote_sync, sdk_client, sdk_spawns, sync_log. server/tests: same 6. sessions/src: part_events, runner, tui_info_panel. sessions/tests: mcp_status_panel, part_events, runner, tui_info_panel. tools/tests: mcp_bulk_actions, mcp_catalog_search, mcp_lifecycle, mcp_payload_filter.

## Controller flip needs (proposal only, no edit)
- FEATURES.md-only sync: ROUTE-001..012, REL-004, UI-001..018, AUTO-003/007, EXT-003/007/013, INT-004/008, OPS-010, PROV-014, SESS-019/020, TOOL-006/009/015.
- Code-landed, needs gate GREEN then flip in-progress→accepted: AUTO-004/005/006, EXT-001/002/004..006/008..012, INT-001..003/005..007/009/010, OPS-001..009, REL-001..003, SHARE-001..005, WEB-001..006.
- Code-landed but not-started + missing wiring/classification — wire/register/gate first: ACP-001/002, SDK-001/002, SYNC-001/002 (prefix), PROV-015..024, WSX-001/002, HEAD-001/002, RUN-001, TOOL-016..020, UI-019, WEB-007..017.
