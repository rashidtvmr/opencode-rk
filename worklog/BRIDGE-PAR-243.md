# BRIDGE-PAR-243 dialog_stack_full

Claim: DialogFlow over DialogHost + depth counter.
Evidence: dialog_stack.rs:15 DialogStack open->bool/close->Option, dialog_host.rs:16 DialogHost open/close_top/top.
Boundary: new crates/opentui-bridge/src/dialog_stack_full.rs only. lib.rs untouched.
Tests: 5 in-file (open_bumps, close_drops, failed_no_bump, empty_no_underflow, dup_bumps).
Decisions: depth bump only on host-ack true; drop only on Some; saturating both ways; std-only forbid(unsafe_code).
Verify: rustfmt --check PASS, 96 lines.
