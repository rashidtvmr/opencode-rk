# BRIDGE-PAR-195 context_screen

Claim: ledger in-progress, session ses_par195.
Evidence: crates/opentui-bridge/src/ctx_bundle.rs:18-63 CtxBundle{kv,route,rt}, summary(); crates/cli/src/tui_entry.rs:537-551 Context arm; sibling pattern palette_screen.rs:22-38 fit/pad-clip/truncate.
Target: crates/opentui-bridge/src/context_screen.rs only. context_lines(bundle,width,height)->Vec<String>: "Context" + summary + kv keys, fit width, cap height. std-only, forbid(unsafe_code).
Tests: 5 in-file (order, pad, clip/cap, body-cap, empty-dims).
Decision: mirror palette_screen fit/truncate; ponytail: skip live provider snapshot fields (TS arm snapshot), add when bridge exposes them.
