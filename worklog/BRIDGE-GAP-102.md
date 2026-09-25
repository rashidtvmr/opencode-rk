# BRIDGE-GAP-102 scratchpad

- Claim: BRIDGE-GAP-102 via cc.claim session ses_gap102, scratchpad worklog/BRIDGE-GAP-102.md. OK.
- Source evidence:
  - TS truth `/home/rashid/projects/opencode/packages/tui/src/util/presentation.ts:1-38` wordmark + sessionEpilogue only, no clamp/ellipsis/indent upstream.
  - Style ref `crates/opentui-bridge/src/message_render.rs:38-43` bound_chars char-safe, `:82-92` trunc_line fail-closed width 0, `forbid(unsafe_code)`, `#[must_use]`.
- Target boundary: ONE new file `crates/opentui-bridge/src/presentation.rs`. No lib.rs/Cargo.toml/message_render.rs edits. No cargo. No commit/push.
- Tests (in-file, 5): clamp_keeps_head, clamp_zero_empty, ellipsis_truncates, ellipsis_short_passthrough, indent_prefix.
- Decisions:
  - clamp_lines: lines().take(max).join, 0/empty -> "".
  - ellipsize: chars<=max passthrough else take(max)+"...", 0 -> "" fail-closed.
  - indent: prefix every split('\n') line, n capped at INDENT_CAP 32.
  - ponytail: skipped wiring mod into lib.rs (out of scope), add when bridge integrates.
- Remaining: rustfmt --check file, flip ledger completed if file+tests exist.
