# BRIDGE-PAR-177 scratchpad

- Claim: `cc.claim(..., 'BRIDGE-PAR-177', 'ses_par177', 'worklog/BRIDGE-PAR-177.md')` OK.
- Source evidence:
  - `crates/opentui-bridge/src/scrollback_shared_full.rs:14` `ScrollbackSharedFull { lines, offset }`, `push` (:25), `scroll(delta,height)` (:33), `visible(height)` (:43).
  - `crates/opentui-bridge/src/transcript_paint.rs:22` `paint_lines(entries,&width)`, role else-branch paints `assistant> ` (:30), width-clip via `clip_to_width` (:51).
  - Style model: `run_scrollback.rs` (CAP const, push/commit), `question_view.rs` (forbid unsafe, caps, tests).
- Target boundary: ONE new file `crates/opentui-bridge/src/scrollback_view.rs`. No edit to lib.rs, Cargo.toml, tui_entry.rs, scrollback_shared_full.rs, transcript_paint.rs. No cargo, no commit.
- Tests: 7 unit tests in-file (render newest+prefix, width clip char-safe, scroll window, clamp both ends, push repin, width-0 empty, empty store empty).
- Decisions: entries mapped `(assistant, line)`; paint_lines handles clip trunc + markers; ponytail note for plain-text ceiling.
- Verification: `rustfmt --check crates/opentui-bridge/src/scrollback_view.rs` -> FMT_OK, 113 lines (<150).
- Status: completed.
