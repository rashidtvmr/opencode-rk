# BRIDGE-GAP-52 scratchpad

Claim: ses_gap52, scratchpad worklog/BRIDGE-GAP-52.md
Source evidence: crates/opentui-bridge/src/run_width.rs (style: forbid(unsafe_code), must_use, in-file tests); crates/opentui-bridge/src/run_footer.rs (queue/cap patterns)
Observed: no run_splash.rs existed; TS ref run/splash.ts read-only
Target boundary: ONE file crates/opentui-bridge/src/run_splash.rs, std-only, <150 lines (118)
Tests: banner_has_three_rows, banner_clips_to_width, zero_width_clips_all, separator_is_const_dashes, summary_joins_last_five, summary_caps_at_1kib, summary_short_passthrough (7 tests, in-file)
Decisions: clip by chars with char-boundary-safe byte cap; version prefixed v; empty join passthrough
Unknowns: none
