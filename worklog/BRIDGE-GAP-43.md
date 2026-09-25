# BRIDGE-GAP-43 scratchpad

Claim: BRIDGE-GAP-43 via ses_gap43.
Source evidence: crates/opentui-bridge/src/revert_diff.rs:18 FileDiff, :12 MAX_FILES; lib.rs:67 pub mod revert_diff.
Target: crates/opentui-bridge/src/revert_banner.rs only (180-line ceiling, std-only, forbid unsafe).
Tests: hidden_by_default, show_renders_one_line, empty_show_errs_fail_closed, over_cap_truncates_to_32, hide_clears_state, message_capped_at_512_chars.
Decisions: message built once in show, capped 512 chars; render clones message.
Unknowns: none. No cargo run per task scope; rustfmt --check PASS.
