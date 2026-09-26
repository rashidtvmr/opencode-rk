# BRIDGE-PAR-233

Claim: ses_par233, ledger claim ok.
Evidence: dialog-message.tsx:10-22 DialogMessage title "Message Actions"; dialog_host.rs:1-54 host/stack pattern, forbid(unsafe_code).
Target: crates/opentui-bridge/src/dialog_message_full.rs only. No lib.rs/Cargo.toml/dialog_host.rs edits.
Impl: MessageDialog {title cap 128, body cap 4KiB chars, open} + open_with/close/lines(width)->Vec<String> char-safe word wrap, long-word split, cap 32 rows. std-only, forbid(unsafe_code), 129 lines.
Tests: 6 tests (open caps+flag, close, row cap, char-safe wrap, empty body, width 0).
Verify: rustfmt --check PASS. No cargo per task scope.
