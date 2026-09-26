# BRIDGE-PAR-171 scratchpad
claim: ses_par171 in-progress
evidence: subagent-data.ts taskTab label/description/status; run_subagent_data.rs SubagentRow caps pattern; subagent_footer.rs trunc/max pattern
target: crates/opentui-bridge/src/subagent_data_full.rs only, <130 lines, std-only, forbid unsafe
tests: 6 in-file (caps, bounds, label parts, abbrev50, update trunc, clamp)
decisions: char-based trunc (footer pattern); new() clamps via min(100), set_progress rejects >100 false
