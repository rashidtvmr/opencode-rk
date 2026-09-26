# BRIDGE-PAR-217 scratchpad
- claim: BRIDGE-PAR-217, session ses_par217, scratchpad worklog/BRIDGE-PAR-217.md
- evidence: crates/opentui-bridge/src/perm_gate.rs:8 PermGate{ctx}; permission_ctx.rs:26 request/41 grant/55 deny/66 is_granted; permission_full.rs:20 PermissionFull single-decision; footer.permission.tsx:1-60 allow-once/always/reject stages
- target: new file run_permission_full.rs only; no edit lib.rs/Cargo.toml/permission_full.rs/perm_gate.rs
- impl: PermissionFlow{gate:PermGate,pending:Vec cap 8}+request/allow/deny/pending_list; ID_CAP 128; std-only; forbid(unsafe_code); 106 lines
- tests: 6 unit tests (queue, bad-id, dup/granted, cap8, allow, deny)
- verify: rustfmt --check clean (exit 0)
