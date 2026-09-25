# BRIDGE-GAP-85 scratchpad

Claim: BRIDGE-GAP-85 via ses_gap85, status completed.
Source: packages/tui/src/routes/session/dialog-message.tsx:22-23 (DialogMessage, message lookup); actions in session_msg_dialogs.rs.
Boundary: crates/opentui-bridge/src/session_message_dialog.rs only; no lib.rs/Cargo.toml edits; no cargo run; no commit.
Tests: 7 in-file (empty errs, body truncates, id truncates, closed none, preview format, preview 200, close clears). rustfmt --check FMT_OK.
Decisions: close keeps id/body; render "id: preview(200)"; char-based truncation.
