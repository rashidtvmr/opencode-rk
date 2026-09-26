# BRIDGE-PAR-285 scratchpad (ses_par285)
- Claim: BRIDGE-PAR-285 via cc.claim, session ses_par285. OK.
- Source: packages/tui/src/util/collapse-tool-output.ts (19 lines, collapseToolOutput head-preview semantics); existing crates/opentui-bridge/src/collapse.rs:20 (collapse_tool_output) - distinct API, no reuse (different contract: full head+marker+tail vs head-preview).
- Target: crates/opentui-bridge/src/collapse_util_full.rs only. No lib.rs/Cargo.toml edits.
- Impl: COLLAPSE_DEFAULT=20, collapse_lines(lines,max)->Vec<String>; over max: head_n=(max+1)/2, marker "... (N hidden) ...", tail; max==0 -> marker only.
- Tests: 6 (under, exact, even split, odd head-heavy, zero, const). Frozen, unrun (no cargo per task).
- Verify: rustfmt --check PASS (FMT_OK after rustfmt fix of 2 blank-line diffs). 77 lines, std-only, forbid(unsafe_code).
