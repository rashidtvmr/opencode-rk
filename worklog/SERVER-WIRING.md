# SERVER-WIRING

Wired 20 missing `pub mod` lines into `crates/server/src/lib.rs` (additive append, no reorder, no impl changes).

## Wired (all exist on disk, verified)
- acp_bridge, acp_files
- chat_composer
- control_plane_errors, control_plane_exposure, control_plane_inputs
- event_stream
- protocol_api
- remote_sync
- sdk_client, sdk_spawns
- sync_log
- transcript_lane, turn_parts
- voice_capture
- web_artifact, web_attachments, web_entry_probe, web_tool_chooser
- workspace_proxy

## Still missing
None. All 47 `.rs` files under `crates/server/src/` now wired except `lib.rs` itself.

## Notes
- New modules reference no `crate::`/`super::` paths and no non-std extern deps (only `std`, `serde`), so wiring is collision-free.
- `chat_composer.rs:13` and `web_attachments.rs:6` carry inner `#![forbid(unsafe_code)]` (same as `lib.rs:2`, `remote_ledger.rs:2`). Harmless under `cargo check`; flagged for owning lane to lift to crate-level only.
- `sync_log.rs` uses 64 KiB payload cap string; check owning lane names the constant (not verified here).
- `session_turn_stream_api` 2/2 FAIL both with and without wiring -> pre-existing, not caused by this lane. Evidence: stashed lib.rs (pristine) still fails 2/2 in 0.01s; fails are timing/provider-mock assertions (`Timeout`, `left: "assistant_delta" != "error"`), untouched by mod wiring.
