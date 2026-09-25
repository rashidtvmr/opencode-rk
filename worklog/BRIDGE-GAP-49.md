# BRIDGE-GAP-49 scratchpad

Claim: BRIDGE-GAP-49 via ses_gap49. Owned file: crates/opentui-bridge/src/run_command.rs.
Source evidence: crates/opentui-bridge/src/run_footer.rs (sibling pattern); TS ref run/footer.command.tsx absent in tree (glob no match) - spec-derived.
Target boundary: RunCommand {name cap 64, args cap 16x512, cwd Option} + parse + render. std-only, forbid(unsafe_code), <180 lines.
Tests: parse_slash, parse_args, empty_errs, cap_truncates, render_roundtrip.
Decisions: strip one leading slash; split_whitespace; trim caps via chars().take; cwd defaults None.
