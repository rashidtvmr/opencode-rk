# SESSIONS-WIRING

Own: crates/sessions runner + sync wiring.

## Claim
tests/runner.rs, part_events.rs, tui_info_panel.rs, mcp_status_panel.rs fail E0432/E0433 — modules exist on disk but unwired in lib.rs.

## Evidence
- HEAD lib.rs:1-46 wires ui_001..ui_013, mcp_status_panel, but not runner, part_events, tui_info_panel.
- src/: runner.rs, part_events.rs, tui_info_panel.rs, mcp_status_panel.rs all present.
- No sync_log.rs / sync*.rs in src; no sync module owned by sessions. Nothing to wire there.
- mcp_status_panel already wired at HEAD; not touched.

## Fix
Additive append after `pub mod ui_013;` (no reorder, no fmt drift):
```rust
pub mod part_events;
pub mod runner;
pub mod tui_info_panel;
```
Diff: 3 insertions, 0 other lines. `cargo fmt --check` drift pre-existing from other lanes reverted via `git checkout HEAD -- lib.rs` then python insertion; final `git diff` shows only 3-line hunk.

## Verification (serial, CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2)
- cargo check -p opencode-rk-sessions: exit 0 (5 pre-existing dead-code warnings in types.rs).
- --test runner: 5 passed.
- --test part_events: 5 passed.
- --test tui_info_panel: 5 passed.
- --test mcp_status_panel: 5 passed (already wired; confirms no regression).

## Notes
- Frozen tests untouched.
- Earlier `cargo fmt` by toolchain (or prior dirty tree) rewrote lib.rs; reverted to keep diff additive-only.
