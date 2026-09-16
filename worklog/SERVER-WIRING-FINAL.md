# SERVER-WIRING-FINAL — lib.rs mod wiring

## Claim
`crates/server/src/lib.rs` declares every `src/*.rs` module. Alphabetical, additive only.

## Evidence
- `grep -c '^pub mod' crates/server/src/lib.rs` → **45**
- `ls crates/server/src/*.rs | wc -l` → **46** (45 modules + `lib.rs` itself)
- Coverage: 45/45 modules wired, 0 missing.

## Full mod list (45, alphabetical)
acp_bridge, acp_files, app_client, auto_loop, auto_report, auto_window,
chat_composer, clients, control_decode, control_plane_errors,
control_plane_exposure, control_plane_inputs, daemon, desktop_bridge,
enterprise_link, error_translate, event_bus, event_stream, origin_check,
protocol_api, rel_stamp, rel_verify, remote_ledger, remote_sync, repo_ops,
route_table, sdk_client, sdk_spawns, sync_log, transcript_lane, turn_parts,
voice_capture, web_artifact, web_assets, web_attachments, web_config,
web_cors, web_entry_probe, web_footer, web_headers, web_host, web_route,
web_suffix, web_tool_chooser, workspace_proxy

## Target boundary (required 11)
acp_bridge, acp_files, sdk_client, sdk_spawns, remote_sync, sync_log,
workspace_proxy, voice_capture, event_stream, turn_parts, chat_composer — all present.

## Tests
- `cargo check -p opencode-rk-server` → exit 0 (only pre-existing sessions warnings).
- `cargo test -p opencode-rk-server --test sync_log --test acp_bridge --test acp_files --test sdk_client --test sdk_spawns --test remote_sync --test workspace_proxy` → all green:
  acp_bridge 5, acp_files 5, remote_sync 5, sdk_client 12, sdk_spawns 11, sync_log 5, workspace_proxy 5. Total **48 passed, 0 failed**.

## Decisions
- Single edit: sorted the 20 appended mods into the existing block alphabetically; no other changes.
- Did not touch ralph.json or any other crate's lib.rs.

## Remaining unknowns
None for wiring. Failing broader suites outside these 7 targets (if any) belong to owning lanes.

## Wave 5 re-verify (2026-09-16T00:05Z)
- `grep -c 'pub mod'` → 45. All 7 targets present: acp_bridge, acp_files, sdk_client, sdk_spawns, remote_sync, sync_log, workspace_proxy.
- `cargo check -p opencode-rk-server` → exit 0 (0 errors, 10 pre-existing warnings).
- Serial (JOBS=2/THREADS=2, timeout 120, rtk): acp_bridge 5, acp_files 5, sdk_client 12, sdk_spawns 11, remote_sync 5, workspace_proxy 5, sync_log 5. Total 48/48, all exit 0.
- No edit. Log: /tmp/opencode/w5-srvnew.log.
