# BRIDGE-PAR-176 scratchpad

Claim: BRIDGE-PAR-176 via ses_par176, scratchpad worklog/BRIDGE-PAR-176.md.
Owned file: crates/opentui-bridge/src/footer_assemble.rs (only file touched).

Source evidence:
- TS truth packages/tui/src/routes/session/footer.tsx:1-91 (Footer, row layout, directory slot + status items).
- crates/opentui-bridge/src/footer_width_calc.rs:21-42 (width_for/fits/truncate_to).
- crates/opentui-bridge/src/session_footer_full.rs:44-56 (render join pattern, read-only).

Target boundary: assemble_footer(slot,items,width)->Vec<String> + footer_height()->1.
Empty items -> vec![slot]; else truncate_to then join " | " then char-safe clip.
std-only, forbid(unsafe_code), <120 lines, >=5 in-file tests.

Tests: 6 in-file (empty/joins/tail-drop/ascii-clip/unicode-clip/height+zero).
Verification: rustfmt --check only (no cargo/commit per scope).
