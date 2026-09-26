# BRIDGE-PAR-133 scratchpad

Claim: BRIDGE-PAR-133 via ses_par133, worklog/BRIDGE-PAR-133.md.
Source: packages/tui/src/editor.ts:26 openEditor input; crates/opentui-bridge/src/editor.rs (naming style, forbid unsafe).
Target: crates/opentui-bridge/src/editor_bridge.rs only. No lib.rs/Cargo.toml edits.
Tests: 6 in-file (empty errs, open/close, goto, status parts, file trunc, status trunc).
Decisions: std-only, forbid(unsafe_code), char-boundary trunc, 124 lines.
Evidence: rustfmt --check EXIT 0.
