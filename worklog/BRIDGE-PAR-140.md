# BRIDGE-PAR-140: message_router.rs

Claim: ledger in-progress via ses_par140.
Source: dialog-message.tsx:22-106 (Revert/Copy/Fork DialogSelect); session_message_dialog.rs:9 MAX_MESSAGE_ID=64, open/truncate pattern.
Target: crates/opentui-bridge/src/message_router.rs only. lib.rs/Cargo.toml untouched.
Tests: 6 in-file (labels non-empty, request/take, overwrite, empty-none, id trunc, labels distinct).
Decision: MessageAction {Reply,Edit,Delete,Copy} (task spec; TS Revert/Fork map to Delete/Reply intent per task contract).
Verify: rustfmt --check PASS, 115 lines <140, std-only, forbid(unsafe_code).
