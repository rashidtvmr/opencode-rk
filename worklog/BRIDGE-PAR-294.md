# BRIDGE-PAR-294 scratchpad

Claim: BRIDGE-PAR-294, session ses_par294, via tools/completion_claims.py claim.
Source evidence: TS truth /home/rashid/projects/opencode/packages/tui/src/util/layout.ts:1-25 (sibling-margin only; no split/center - new pure helpers). Pattern: crates/opentui-bridge/src/scroll_util_full.rs:1-27 (forbid unsafe, must_use pure fns, in-file tests), sibling_margin.rs:9-12 (MAX consts).
Observed: sibling_margin.rs owns TS mirror; no split_rows/center_in anywhere in crate.
Target boundary: ONE new file crates/opentui-bridge/src/layout_util_full.rs. No lib.rs/Cargo.toml edit. std-only, forbid(unsafe_code), <100 lines.
Tests: 5 in-file (even fit, header clamp, footer yield+caps, center exact/narrow, center floor).
Decisions: header wins over footer; both capped (4/4/8); saturating math; center_in const fn floor.
Remaining: rustfmt --check; flip completed.
