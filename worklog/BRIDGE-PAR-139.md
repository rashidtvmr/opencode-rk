# BRIDGE-PAR-139: dialog_stack.rs

Claim: ledger via ses_par139. Scratchpad worklog/BRIDGE-PAR-139.md.
Source: packages/tui/src/ui/dialog.tsx:70-76 stack store, :116-118 pop on close, :130-132 esc pop, :141-146 drain, :150-151 replace-when-empty. Style ref crates/opentui-bridge/src/dialog.rs:1 forbid unsafe.
Boundary: ONE new file crates/opentui-bridge/src/dialog_stack.rs. No lib.rs/Cargo.toml/dialog.rs/dialogs_system.rs edits. No cargo.
Tests: 6 in-file (roundtrip, dup-top, full-false, close_id-missing, top-none, id-long).
Decisions: Vec<String> cap 8, id cap 64; dup remove+push true; close pop; close_id pos-remove bool; top last-as-str. std-only, forbid(unsafe_code).
Unknowns: none.
