claim: BRIDGE-GAP-92 owned by ses_gap92.
source evidence:
- TS truth: /home/rashid/projects/opencode/packages/opencode/src/cli/cmd/run/scrollback.writer.tsx (entryWriter/spacerWriter/turnSummaryWriter render writers; no retained-state cap, active-commit split is new Rust boundary).
- Rust sibling (read-only): crates/opentui-bridge/src/run_scrollback.rs:1-68 (Scrollback CAP 2000, push-evict, freeze; this file adds active-commit staging on top, does not duplicate it).
target boundary: ONE new file crates/opentui-bridge/src/run_scrollback_writer.rs. No lib.rs/Cargo.toml/run_scrollback.rs/scollback_family edits. No cargo. No commit/push.
tests: in-file #[cfg(test)] 6 tests (write_evicts_oldest, active_truncates_to_cap, active_truncation_keeps_char_boundary, flush_moves_active, flush_none_false, tail_window_with_active).
decisions:
- tail(n) = last n committed rows + active appended as preview (len up to n+1); documented in-file.
- set_active truncates bytes at char boundary via is_ascii_boundary floor.
- write_line does not truncate committed rows (per spec: append-or-evict only).
remaining: rustfmt --check only per scope; cargo explicitly forbidden.
