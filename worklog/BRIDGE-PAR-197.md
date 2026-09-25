# BRIDGE-PAR-197 frame_assemble

Claim: ses_par197, ledger in-progress then completed.
Evidence: paint_callsite.rs:12 build_chat_lines; paint_full.rs:50 exact height clamp; toast_center.rs:15 ToastCenter; toast_line.rs:13 clip view.
Boundary: crates/opentui-bridge/src/frame_assemble.rs only. lib.rs untouched (orchestrator pre-wires).
Tests: 6 in-file (height, toast-replace, clip, clamp, none-passthrough, unicode). rustfmt --check PASS.
Decisions: delegate layout/height to build_chat_lines; toast clips to width.max(20) char-safe; pad/truncate to height.max(8).
Unknowns: none.
