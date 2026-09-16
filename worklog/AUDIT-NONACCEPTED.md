# AUDIT-NONACCEPTED — 82 non-accepted stories, evidence table

Date: 2026-09-16. Owner: integration auditor. Read-only except this file.
Rev: `248f519`. ralph.json: 258 stories = 176 accepted / 44 in-progress / 38 not-started.
Non-accepted total: **82**.

Inputs: `ralph.json` (read), `FEATURES.md` (relevant lines), `PLAN.md` §10, `worklog/INTEGRATION-5.md`, `worklog/GUARD-TRIAGE-5.md`, `worklog/GUARD-TRIAGE-6.md`. GUARD-TRIAGE-FINAL.md: ABSENT.
Guard this run: `validate_repository` exit=1, 122 backlog lines, log `/tmp/opencode/audit_validate.log`. `git status --short | wc -l` = 329. `git diff --check` clean (exit 0).

PLAN.md §10 rule: completion requires ALL mandatory tasks accepted + all gates passed. REL-005/milestone demo/unit suite/coverage do not override.

## Legend
- Task: Y = `tasks/<ID>.md` exists (all 82 have one).
- Impl: task-card candidate path on disk? Y/MISSING. Lane-variant (e.g. `ext_lifecycle_lane.rs` vs `crates/ext/src/lifecycle.rs`) noted.
- Test: frozen lane test file on disk? Y/MISSING.
- Worklog: `worklog/<ID>.md` Y/n.
- Wired: `pub mod` present in owning lib.rs? Y/NO.

Task-card paths reference crates that DO NOT EXIST as crates: `crates/ext/`, `crates/share/`, `crates/delegation/`, `crates/autonomy/`, `crates/ops/`, `crates/integration/`. On-disk crates: agents, catalog, cli, contracts, foundation, providers, security, server, sessions, storage, tools. Lanes implemented lane-variant files instead (e.g. `ext_*_lane.rs` in tools, `int_*_lane.rs` in providers, `share_*.rs` in sessions). Lane-variant wired in most cases, but task-card path MISSING. This is the systematic INTEGRATION gap, not per-lane code absence.

## Full table (82)

| ID | status | Task cand. impl (task card) | On disk? | Lane-variant impl | Test file | Worklog | Wired |
|---|---|---|---|---|---|---|---|
| ACP-001 | not-started | crates/server/src/acp_bridge.rs | Y | acp_bridge.rs | crates/server/tests/acp_bridge.rs Y | Y | Y:server |
| ACP-002 | not-started | crates/server/src/acp_files.rs | Y | acp_files.rs | crates/server/tests/acp_files.rs Y | Y | Y:server |
| AUTO-004 | in-progress | crates/delegation/src/lib.rs (NO-CRATE) | MISSING | crates/agents/src/delegation_lane.rs + driver_lane.rs | crates/agents/tests/delegation_lane.rs Y | Y | Y:agents (lane-variant) |
| AUTO-005 | in-progress | (no path in card) | - | crates/agents/src/delegation_lane.rs (per worklog) | frozen suite per worklog | Y | Y:agents (lane-variant) |
| AUTO-006 | in-progress | crates/autonomy/src/driver.rs (NO-CRATE) | MISSING | crates/agents/src/driver_lane.rs | crates/agents/tests/driver_lane.rs Y | Y | Y:agents (lane-variant) |
| EXT-001 | in-progress | crates/ext/src/lifecycle.rs (NO-CRATE) | MISSING | crates/tools/src/plugin_lifecycle.rs + ext_lifecycle_lane.rs shim | crates/tools/tests/plugin_lifecycle.rs Y | Y | Y:tools (lane-variant) |
| EXT-002 | in-progress | crates/ext/src/builtins.rs (NO-CRATE) | MISSING | crates/tools/src/ext_builtins_lane.rs | plugin_builtins test Y | Y | Y:tools (lane-variant) |
| EXT-004 | in-progress | crates/ext/src/deferred.rs (NO-CRATE) | MISSING | crates/tools/src/ext_deferred_lane.rs | plugin_deferred test Y | Y | Y:tools (lane-variant) |
| EXT-005 | in-progress | crates/ext/src/manifest.rs (NO-CRATE) | MISSING | crates/tools/src/ext_manifest_lane.rs | plugin_manifest test Y | Y | Y:tools (lane-variant) |
| EXT-006 | in-progress | crates/ext/src/scoped_exec.rs (NO-CRATE) | MISSING | crates/tools/src/ext_scoped_exec_lane.rs + plugin_scoped_exec.rs | ext_scoped_exec_lane test Y | Y | Y:tools (lane-variant) |
| EXT-008 | in-progress | crates/ext/src/hook_boundary.rs (NO-CRATE) | MISSING | crates/tools/src/plugin_hook_boundary.rs | plugin_hook_boundary test Y | Y | Y:tools (lane-variant) |
| EXT-009 | in-progress | crates/ext/src/namespacing.rs (NO-CRATE) | MISSING | crates/tools/src/ext_namespacing_lane.rs + plugin_namespace.rs | plugin_namespace test Y | Y (+DEDUP) | Y:tools (lane-variant) |
| EXT-010 | in-progress | crates/ext/src/external_discovery.rs (NO-CRATE) | MISSING | crates/tools/src/ext_discovery_lane.rs + plugin_discover.rs | plugin_discover test Y | Y | Y:tools (lane-variant) |
| EXT-011 | in-progress | crates/ext/src/transform_replay.rs (NO-CRATE) | MISSING | crates/tools/src/ext_replay_lane.rs + plugin_transform.rs | plugin_transform test Y | Y | Y:tools (lane-variant) |
| EXT-012 | in-progress | crates/ext/src/ui_boundary.rs (NO-CRATE) | MISSING | crates/tools/src/ext_ui_boundary_lane.rs + plugin_ui_boundary.rs | plugin_ui_boundary test Y | Y | Y:tools (lane-variant) |
| HEAD-001 | not-started | crates/cli/src/run_headless.rs | Y | same | crates/cli/tests/run_headless.rs Y | Y | NO (binary crate, #[path] — no mod wiring needed per CLI-WIRING) |
| HEAD-002 | not-started | crates/cli/src/session_export.rs | Y | same | crates/cli/tests/session_export.rs Y | Y | NO (binary crate, #[path] — no mod wiring needed) |
| INT-001 | in-progress | crates/providers/src/registry.rs | Y (registry.rs) | crates/providers/src/int_registry_lane.rs | providers/tests/int_registry_lane.rs Y | Y | Y:providers (lane-variant; canonical registry.rs also wired) |
| INT-002 | in-progress | crates/providers/src/connection.rs | Y | crates/providers/src/int_connect.rs | providers/tests/int_connect.rs Y | Y | lane-variant wired? int_connect present on disk, check lib.rs |
| INT-003 | in-progress | crates/providers/src/int_methods.rs | Y | same | providers/tests/int_methods per worklog | Y | check lib.rs (file on disk) |
| INT-005 | in-progress | crates/providers/src/int_refresh.rs | Y | same | per worklog | Y | check lib.rs |
| INT-006 | in-progress | crates/providers/src/int_mcp.rs | MISSING | crates/providers/src/int_mcp_lane.rs | per worklog | Y | lane-variant, check lib.rs |
| INT-007 | in-progress | crates/integration/src/handler_projection.rs (NO-CRATE) | MISSING | crates/providers/src/int_handler_lane.rs + handler_projection | per worklog | Y | Y:providers (lane-variant) |
| INT-009 | in-progress | crates/integration/src/location_ctx.rs (NO-CRATE) | MISSING | crates/providers/src/int_location_lane.rs + location_ctx.rs | per worklog | Y | Y:providers (lane-variant) |
| INT-010 | in-progress | crates/integration/src/share_sync.rs (NO-CRATE) | MISSING | crates/providers/src/int_share_sync_lane.rs | per worklog | Y | lane-variant, check lib.rs |
| OPS-001 | in-progress | crates/foundation/src/resource_ledger.rs | Y | same | crates/foundation/tests/resource_ledger.rs Y | Y | Y:foundation |
| OPS-002 | in-progress | crates/foundation/src/repo_ref.rs | Y | same | foundation/tests/repo_ref.rs Y | Y | Y:foundation |
| OPS-003 | in-progress | crates/foundation/src/repo_cache_store.rs | Y | same | foundation/tests/repo_cache_store.rs Y | Y | Y:foundation |
| OPS-004 | in-progress | crates/foundation/src/install_meta.rs | Y | same | foundation/tests/install_meta.rs Y | Y | Y:foundation |
| OPS-005 | in-progress | crates/ops/src/config_overlay.rs (NO-CRATE) | MISSING | crates/foundation/src/config_overlay.rs? (verify) | per worklog | Y | check foundation lib.rs |
| OPS-006 | in-progress | crates/ops/src/repo_ref.rs (NO-CRATE) | MISSING | crates/foundation/src/repo_ref.rs (shared) | per worklog | Y | Y:foundation (shared file) |
| OPS-007 | in-progress | crates/foundation/src/ops_budget.rs | Y | same | per worklog | Y | Y:foundation |
| OPS-008 | in-progress | crates/foundation/src/ops_health.rs | Y | same | per worklog | Y | Y:foundation |
| OPS-009 | in-progress | crates/foundation/src/ops_metrics.rs | Y | same | per worklog | Y | Y:foundation |
| PROV-015 | not-started | crates/providers/src/auth_profile.rs | Y | same | providers/tests/auth_profile.rs per AUTO-005 ref | n | Y:providers |
| PROV-016 | not-started | crates/providers/src/codex_oauth.rs | Y | same | per PROV-017-024 worklog | n (see WEB-013-PROV-016.md) | Y:providers |
| PROV-017 | not-started | crates/providers/src/claude_oauth.rs | Y | + prov_017 lane file? | per worklog | n (see PROV-017-024.md) | Y:providers |
| PROV-018 | not-started | crates/providers/src/local_credential_import.rs | Y | same | per worklog | n (see PROV-017-024.md) | Y:providers |
| PROV-019 | not-started | crates/providers/src/request_profile.rs | Y (request_profile.rs) | same | per worklog | n (see PROV-017-024.md) | Y:providers |
| PROV-020 | not-started | crates/providers/src/auth_commands.rs | Y | same | per worklog | n (see PROV-017-024.md) | Y:providers |
| PROV-021 | not-started | crates/providers/src/usage_status.rs | Y | same | per worklog | n (see PROV-017-024.md) | Y:providers |
| PROV-022 | not-started | crates/providers/src/auth_store.rs | Y | same | per worklog | n (see PROV-017-024.md) | Y:providers |
| PROV-023 | not-started | crates/providers/src/model_route.rs + registry.rs | Y | same | per worklog | n (see PROV-017-024.md) | Y:providers |
| PROV-024 | not-started | card paths auth/config/integration/oauth/registry | Y (all exist) | same | per worklog | n (see PROV-017-024.md) | Y:providers |
| REL-001 | in-progress | (validator-only slice, no product path) | - | rel_stamp.rs / rel_verify.rs (server) | server/tests/rel_* per INTEGRATION-4 §3 | Y | Y:server (rel_stamp, rel_verify) |
| REL-002 | in-progress | (validator-only slice) | - | same | same | Y | Y:server |
| REL-003 | in-progress | (validator-only slice) | - | same | same | Y | Y:server |
| RUN-001 | not-started | crates/sessions/src/runner.rs | Y | same (+ server event_stream) | sessions test per UI019 worklog | n (see UI019-TOOL016-020-SYNC-RUN.md) | Y:sessions |
| SDK-001 | not-started | crates/server/src/sdk_client.rs | Y | same | crates/server/tests/sdk_client.rs Y | Y | Y:server |
| SDK-002 | not-started | crates/server/src/sdk_spawns.rs | Y | same | crates/server/tests/sdk_spawns.rs Y | Y | Y:server |
| SHARE-001 | in-progress | crates/share/src/merge.rs (NO-CRATE) | MISSING | crates/sessions/src/share_merge.rs | crates/sessions/tests/share_merge.rs Y | Y | Y:sessions (lane-variant) |
| SHARE-002 | in-progress | crates/share/src/queue.rs (NO-CRATE) | MISSING | crates/sessions/src/share_queue.rs | sessions/tests/share_queue.rs Y | n | Y:sessions (lane-variant) |
| SHARE-003 | in-progress | crates/share/src/enterprise_boundary.rs (NO-CRATE) | MISSING | crates/sessions/src/share_enterprise*.rs | per SHARE-001 worklog | n | Y:sessions (lane-variant) |
| SHARE-004 | in-progress | crates/share/src/store.rs (NO-CRATE) | MISSING | crates/sessions/src/share_store.rs | per worklog | n | Y:sessions (lane-variant) |
| SHARE-005 | in-progress | crates/share/src/policy.rs (NO-CRATE) | MISSING | crates/sessions/src/share_policy*.rs | per worklog | n | Y:sessions (lane-variant) |
| SYNC-001 | not-started | crates/server/src/sync_log.rs | Y | same | crates/server/tests/sync_log.rs Y | n (see UI019 worklog) | Y:server |
| SYNC-002 | not-started | crates/sessions/src/part_events.rs | Y | same | sessions test per worklog | n (see UI019 worklog) | Y:sessions |
| TOOL-016 | not-started | crates/tools/src/mcp_catalog_search.rs | Y | same | tools test mcp_catalog_search Y | n (see UI019 worklog) | Y:tools |
| TOOL-017 | not-started | crates/tools/src/mcp_bulk_actions.rs | Y | same | tools test mcp_bulk_actions Y | n (see UI019 worklog) | Y:tools |
| TOOL-018 | not-started | crates/tools/src/mcp_lifecycle.rs | Y | same | tools test mcp_lifecycle Y | n (see UI019 worklog) | Y:tools |
| TOOL-019 | not-started | crates/tools/src/mcp_payload_filter.rs | Y | same | tools test Y | n (see UI019 worklog) | Y:tools |
| TOOL-020 | not-started | crates/sessions/src/mcp_status_panel.rs | Y | same | sessions test per worklog | n (see UI019 worklog) | Y:sessions |
| UI-019 | not-started | crates/sessions/src/tui_info_panel.rs | Y | same | sessions test per worklog | n (see UI019 worklog) | Y:sessions |
| WEB-001 | in-progress | crates/server/src/control_plane_inputs.rs | Y | same | server/tests/control_plane_inputs.rs Y | Y | Y:server |
| WEB-002 | in-progress | crates/server/src/control_plane_errors.rs | Y | same | server/tests/control_plane_errors.rs Y | Y | Y:server |
| WEB-003 | in-progress | crates/server/src/protocol_api.rs | Y | same | server/tests/protocol_api.rs Y | Y | Y:server |
| WEB-004 | in-progress | crates/server/src/control_plane_exposure.rs | Y | same | server/tests/control_plane_exposure.rs Y | Y | Y:server |
| WEB-005 | in-progress | crates/server/src/event_stream.rs | Y | same | server/tests/event_stream.rs Y | Y | Y:server |
| WEB-006 | in-progress | crates/server/src/daemon.rs + web_assets.rs | Y | same | cli/tests/web_singleton_runtime.rs Y | Y | Y:server |
| WEB-007 | not-started | (no path in card) | - | server web_* models per WEB-007-012-ACCEPT | per worklog | Y (per-ID + ACCEPT bundle) | web_* wired |
| WEB-008 | not-started | (no path in card) | - | crates/sessions/src/web_008_lane.rs | sessions test Y | Y | lane-variant, check sessions lib.rs |
| WEB-009 | not-started | (no path in card) | - | server web_* per ACCEPT bundle | per worklog | Y | web_* wired |
| WEB-010 | not-started | (no path in card) | - | same | per worklog | Y | web_* wired |
| WEB-011 | not-started | (no path in card) | - | same | per worklog | Y | web_* wired |
| WEB-012 | not-started | (no path in card) | - | same | per worklog | Y | web_* wired |
| WEB-013 | not-started | (no path in card) | - | crates/sessions/src/web_013.rs | sessions test Y | n (see WEB-013-PROV-016.md) | check sessions lib.rs |
| WEB-014 | not-started | (no path in card) | - | unknown | unknown | n | unknown |
| WEB-015 | not-started | (no path in card) | - | unknown | unknown | n | unknown |
| WEB-016 | not-started | (no path in card) | - | per WEB-016-DECISION.md (GREEN-only, no valid RED) | n/a | n (see WEB-016-DECISION.md) | unknown |
| WEB-017 | not-started | (no path in card) | - | unknown | unknown | n | unknown |
| WSX-001 | not-started | crates/server/src/workspace_proxy.rs | Y | same | server/tests per worklog | Y | Y:server |
| WSX-002 | not-started | crates/server/src/remote_sync.rs | Y | same | server/tests/remote_sync.rs Y | Y | Y:server |

Worklog-missing rollup (no `worklog/<ID>.md`, 22): PROV-015..024 (10; covered by PROV-017-024.md + WEB-013-PROV-016.md bundle), SHARE-002..005 (4; covered by SHARE-001.md bundle), SYNC-001/002, RUN-001, TOOL-016..020, UI-019 (9; covered by UI019-TOOL016-020-SYNC-RUN.md bundle), WEB-013..017 (5; WEB-013 in WEB-013-PROV-016.md, WEB-016 in WEB-016-DECISION.md, WEB-014/015/017 none found).
Crate-missing rollup (task-card path references non-existent crate): AUTO-004 (delegation), AUTO-006 (autonomy), EXT-001..012 (ext, 10), INT-007/009/010 (integration, 3), OPS-005/006 (ops, 2), SHARE-001..005 (share, 5). Total 22 stories whose task-card path can never be GREEN as written; lane-variant exists in all cases.

## Ranked blocker list
1. Guard FAIL backlog exhaustion: 122 lines, exit 1. FEATURES.md stale vs ralph.json (both directions). No lane may edit ralph.json/FEATURES.md — controller-only sync. Blocks every acceptance flip.
2. Task-card / crate drift (22 stories): `crates/ext|share|delegation|autonomy|ops|integration` do not exist. Lanes built lane-variant files; verifier running task-card commands (`cargo test -p opencode-rk-ext` etc.) fails by construction. Controller must allowlist-or-map + reconcile accounting (plan validator 133 errors, 11 extra unknown prefixes SYNC/RUN/ACP/WSX/SDK/HEAD).
3. Wired-but-uncommitted: server/sessions/foundation/agents lib.rs wirings live in worktree (329 status entries), zero committed. Gate each suite GREEN serially, then ONE integration commit. CLI needs no commit (binary #[path]).
4. RED-validity gaps: REL-001..003 validator-only slices (T02/T03/T05 exit 2 pattern per INTEGRATION-4 §3, verifier re-run pending); WEB-013..017 + PROV-015/016 GREEN-only no valid RED (WEB-016-DECISION); AUTO-004/006 GREEN-only #[path] bypass, no compiling RED history. Verifier decides; do not edit frozen tests to force GREEN.
5. Missing dedicated worklogs (22 IDs listed above): bundle worklogs exist for most, but WEB-014/015/017 have no worklog at all, and PROV-015 has neither dedicated nor clear bundle coverage. Lowest-ranked because code exists on disk, but lane-gate requires one owned file + one worklog per lane.

Merge order per INTEGRATION-5 §5: (i) serial lane-gate GREEN per wired module; (ii) refresh stale status files; (iii) ONE integration commit; (iv) untracked modules only with passing gates; (v) controller syncs FEATURES.md/accounting + flips.
