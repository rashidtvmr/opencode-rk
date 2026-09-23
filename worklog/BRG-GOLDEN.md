# BRG-GOLDEN scratchpad

- Claim: `BRG-GOLDEN` via `ses_brg_golden2`, scratchpad `worklog/BRG-GOLDEN.md`. Ledger `claim` ok.
- Source evidence: crate `crates/opentui-bridge` — `lib.rs` does not wire all
  modules; `input.rs`/`buffer.rs` broken mid-lane, so full-crate `cargo test -p`
  will not build. Hence golden test is self-contained (no
  `use opentui_bridge::...`), std-only, `rustc --test` compiled.
- Observed scenario: owned file `crates/opentui-bridge/tests/bridge_golden.rs`
  does not exist; parent `tests/` dir missing.
- Target boundary: ONLY owned test file + this scratchpad. No crate src edits.
  No commit/push (orchestrator lands).
- Tests (frozen at RED): `golden_chat_80x24`, `golden_resize`,
  `golden_wide_chars`; consts `SNAPSHOT`, `SNAPSHOT_RESIZE`,
  `SNAPSHOT_RESIZED`, `SNAPSHOT_WIDE`, all via `assert_eq`.
- Decisions: tiny `TextGrid` (cols/rows viewport, logical lines, greedy wrap on
  display width, CJK wide ranges => 2, resize only changes viewport so content
  is preserved by re-wrap). RED = stub `wrapped()`/`render_rows()`; GREEN fixes
  only the IMPL section, zero test edits.
- Remaining unknowns: none for this lane.

- GREEN: 3/3 pass via rustc --test (IMPL section fixed only: wrapped/render_rows; zero TESTS edits post-freeze). RED sha256 16da8b96, GREEN sha256 8ccaf27b.
