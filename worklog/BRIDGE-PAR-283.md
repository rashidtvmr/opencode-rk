# BRIDGE-PAR-283 scratchpad

- Claim: pre-held by ses_par283 in-progress (re-claim fenced, same session).
- Source evidence: packages/tui/src/ui/border.ts:1-21 (EmptyBorder/SplitBorder only, no Full preset); crates/opentui-bridge/src/border.rs (presets + dialog_frame already covered).
- Target boundary: ONE new file crates/opentui-bridge/src/border_ui_full.rs; lib.rs/Cargo.toml untouched.
- API: border_box(title:&str, lines:&[String], width:usize)->Vec<String>; box_width(width:usize)->usize; consts MAX_BOX_W=120, MIN_BOX_W=4, MAX_BOX_ROWS=64.
- Decisions: single-line box chars ─│┌┐└┘; title embedded `┌─ title─┐`; body pad/truncate to inner width; rows capped 64.
- Tests: 5 tests (width clamp, corners+width, title embed, pad+truncate, 64-row cap). 87 lines, std-only, forbid(unsafe_code).
- Verification: rustfmt --check PASS. No cargo per task scope.
