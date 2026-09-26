# BRIDGE-PAR-340 (unclaimed, file-only)

- Task: new `crates/opentui-bridge/src/home_index_full.rs`, no lib.rs/Cargo.toml edits, no commit.
- Status: ledger untouched per spawn override (orchestrator owns claims.json); proceeding file-only.
- Evidence: TS `packages/tui/src/routes/home/index.tsx` MISSING (only `session-destination.tsx:26` selected signal); Rust `home_index.rs` MISSING; modeled on `question.tsx:29,270-279` wrap + `dialog_select.rs` bounds style + `home_plugin.rs` forbid/docs/test pattern.
- Boundary: `HomeIndex{items,cursor}` cap 64 x 256 chars, `push->bool`, `move_cursor(delta:isize)` wrap, `selected->Option<&str>`, std-only, forbid unsafe.
- Tests: 4 in-module (push/selected, reject/full, wrap, empty noop).
- Verify: `rustfmt --check` only (pending).
