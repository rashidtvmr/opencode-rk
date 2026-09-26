# BRIDGE-PAR-402 scratchpad (UNCLAIMED - orchestrator owns ledger)

- Task: BRIDGE-PAR-402, file-only lane. No claim made (per delegation: do NOT touch tasks/completion/claims.json).
- Claim: unclaimed. Ledger untouched.
- Source: packages/tui/src/routes/session/dialog-message.tsx:1-50 (DialogMessage, title "Message Actions", message lookup via sync.data.message[sessionID], revert/copy/fork actions).
- Boundary: ONE new file crates/opentui-bridge/src/sess_msg_dlg_full.rs. No lib.rs / Cargo.toml / dialog_message_full.rs edits. No cargo, no commit.
- Target: MsgDlg { text: String cap 4KiB chars, sent: bool } + set + send(&mut self)->bool (true once) + is_sent. std-only, forbid(unsafe_code), under 80 lines, >=3 tests.
- Decision: set() truncates to 4096 chars, resets sent=false (allows re-arm); send() false on empty or already-sent, true once.
- Tests: set_caps, send_once, empty_no_send (+ rearm).
- Verify: rustfmt --check only.
