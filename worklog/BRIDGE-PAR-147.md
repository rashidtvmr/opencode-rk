# BRIDGE-PAR-147 scratchpad

- Claim: BRIDGE-PAR-147 via tools/completion_claims claim, session ses_par147, status in-progress then completed.
- Source evidence:
  - /home/rashid/projects/opencode/packages/tui/src/context/permission.tsx:1-26 (mode auto/normal store, no id sets; new file adds pending/granted sets).
  - crates/opentui-bridge/src/permission_kinds.rs:1-142 (PermissionKind enum, info/parse/initial_stage; read-only reference).
- Target boundary: ONE new file crates/opentui-bridge/src/permission_ctx.rs. No edits to lib.rs, Cargo.toml, permission_view.rs, permission_kinds.rs.
- Tests: 5 in-file #[cfg(test)] tests (request_then_grant, deny_removes_pending, missing_grant_deny_false, duplicate_request_false, caps_enforced). Verification rustfmt --check PASS only per task; no cargo run per scope ban.
- Decisions: pending cap 32 each max 128 chars; granted cap 64 each max 64 chars; request rejects dup across both lists; grant validates granted-len cap; deny removes pending only; std-only, forbid(unsafe_code), 126 lines.
- Remaining: none. Needs lib.rs wiring by integrator (out of scope).
