# BRIDGE-PAR-361 scratchpad (unclaimed lane, file-only)

- Task: BRIDGE-PAR-361, status: unclaimed (orchestrator owns `tasks/completion/claims.json`, no claim attempted per lane instruction).
- Source evidence: `packages/tui/src/routes/session/permission.tsx:1-50` (Solid permission route, PermissionRequest, PermissionStage permission/always/reject); sibling pattern `crates/opentui-bridge/src/permission_full.rs:1-77` (PermissionFull tool/args/decision, trunc caps, pending/allow/deny).
- Observed: session permission route needs minimal native state, one pending id plus tri-state decision.
- Target boundary: ONE new file `crates/opentui-bridge/src/route_sess_perm_full.rs`, no `lib.rs`/`Cargo.toml` edits, no cargo/commit.
- Tests: 3 unit tests (new_pending_capped, resolve_sets_decision, ask_resets_pending).
- Decisions: std-only, `forbid(unsafe_code)`, char-based trunc cap 128, `ask` resets to pending, `resolve(bool)` last-wins.
- Unknowns: none; wiring into caller is orchestrator/integration job.
