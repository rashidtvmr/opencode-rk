# BRIDGE-PAR-159 scratchpad

Claim: BRIDGE-PAR-159 via ses_par159, scratchpad worklog/BRIDGE-PAR-159.md.
Source evidence: TS `/home/rashid/projects/opencode/packages/tui/src/context/args.tsx:1-16` (createSimpleContext Args, props model/agent/prompt/sessionID + continue/fork/auto). Style ref `crates/opentui-bridge/src/location_ctx.rs:1-119` (forbid unsafe, const caps, new/set/clear/label, 6 tests).
Observed: bridge dir exists, no args_ctx.rs yet; lib.rs untouched per scope.
Target boundary: ONE new file `crates/opentui-bridge/src/args_ctx.rs`, std-only, forbid(unsafe_code), <140 lines.
Tests: 6 in-file #[cfg(test)] (push/get roundtrip, oob none, cap 64, trunc 512, cwd set/get/empty-reject, cwd trunc 1024).
Decisions: Vec<String> + String cwd (spec); char-based truncation; push false at cap; set_cwd false on empty; Default delegates to new.
Unknowns: none.
