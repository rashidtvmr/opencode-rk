# BRIDGE-PAR-137 scratchpad (ses_par137)

Claim: BRIDGE-PAR-137 via cc.claim, session ses_par137. Owned file only: crates/opentui-bridge/src/collapse_view.rs.
Source evidence:
- TS truth: /home/rashid/projects/opencode/packages/tui/src/util/collapse-tool-output.ts:1-19 (head-only slice(0,maxLines)+ellipsis, NOT head+tail).
- crate::collapse.rs:20-37 (collapse_tool_output mirrors TS head-only; do NOT edit).
Observed: no head+tail view exists; new file adds it. Boundary: std-only, forbid(unsafe_code), <140 lines, free fns collapse/preview + CollapseView struct, split('\n') is char-safe (UTF-8 boundary preserving).
Tests: in-file >=5 (passthrough, marker, head+tail, empty, max-zero). Verify: rustfmt --check only (no cargo/commit per scope).
Decisions: free fns delegate to shared helper; CollapseView::new(head,tail) with matching methods; empty text -> ""/empty vec; max=0 -> marker-only.
Remaining: write file, rustfmt --check, flip completed.
