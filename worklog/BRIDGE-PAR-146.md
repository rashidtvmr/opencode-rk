# BRIDGE-PAR-146 scratchpad

Claim: BRIDGE-PAR-146 via cc.claim, session ses_par146. OK.
Source evidence:
- TS truth packages/tui/src/routes/session/footer.tsx:1-91 (directory slot + LSP/MCP/permission items, row layout).
- Rust sibling crates/opentui-bridge/src/session_footer.rs:1-138 (FooterSlot enum, SessionFooter slot+busy, render cap 64). Read only, not edited.
Target boundary: ONE new file crates/opentui-bridge/src/session_footer_full.rs. No lib.rs/Cargo.toml/session_footer.rs/run_footer edits. No cargo, no commit.
Tests: in-file 6 (slot trunc, add cap, item trunc, render parts, busy flag, empty items).
Decisions: pub fields per spec; char-based truncation (unicode-safe); render "slot: a | b" + " (busy)", cap 512; forbid(unsafe_code); std-only.
Unknowns: none. Verify rustfmt --check only.
