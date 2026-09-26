# BRIDGE-PAR-207 scratchpad

Claim: ses_par207, scratchpad worklog/BRIDGE-PAR-207.md.
Source evidence:
- crates/opentui-bridge/src/error_format.rs:398 `pub fn error_message(error: &Value) -> String` (read only).
- grep `transcript|tui_entry` in opentui-bridge/src: no tui_entry file; run_demo.rs + transcript.rs only. "error:" prefix per task contract.
Target boundary: ONE new file crates/opentui-bridge/src/err_line.rs. No lib.rs/Cargo.toml/error_format.rs edits. No cargo, no commit.
Tests: 4 in-file (prefix, trim, 512 cap, is_err_line incl empty/negative).
Decisions: std-only, forbid(unsafe_code), 79 lines. `trim()` before prefix so cap covers prefix+msg. `is_err_line` starts_with "error:" per spec (not "error: ").
Verification: `rustfmt --edition 2021 --check crates/opentui-bridge/src/err_line.rs` exit 0, no output.
Remaining: none. Integrator prewires `pub mod err_line;`.
