# APP-006-membership worklog

Claim: workspace session membership types in owned file only.

## Source evidence (HEAD 5af7884)
- `crates/sessions/src/lib.rs:66` `PAGE_MAX=500`; `clamp_page` at `:317-319` clamps `[1, 500]`.
- `lib.rs:69-154` `SessionManager` rename/archive via SQL, no workspace scope type.
- `crates/contracts/src/lib.rs:15` `MAX_TITLE_BYTES=512`.
- `crates/sessions/src/types.rs:10-16` `MAX_SESSIONS`, `MAX_MESSAGES`, `PAGE_SIZE=100`.
- Contract: `python3 tools/completion_plan.py --card APP-006` T04 scope-reject, T03 rename/archive convergence, T05 pagination bounds.

## Observed scenario
- File `session_membership.rs` absent before lane. RED = compile-fail on missing module.

## Target boundary
- Owned file only: `crates/sessions/src/session_membership.rs`. No `lib.rs` wiring (integrator pre-wires). No Cargo edits.

## Contract
- `WorkspaceId` opaque `[u8;16]`; `Display` hex only for held IDs, never in errors.
- `Role::{Owner,Member}`; `Membership{workspace,role}`; `authorize` -> `Role` or opaque `ScopeReject` (no ID bytes in Display/Debug).
- `WorkspaceView` versioned LWW: `rename` (any role, title-checked), `archive` (owner-only, idempotent), `apply` ignores stale/foreign events.
- Caps: `PAGE_MIN=1 PAGE_DEFAULT=100 PAGE_MAX=500 MAX_TITLE_BYTES=512 MAX_MEMBERS_PER_WORKSPACE=1024`; `clamp_page`, `paginate`, `within_member_cap`.
- `#![forbid(unsafe_code)]`, std only.

## Tests (frozen, 7 in-file)
1. cross_scope_reject_leaks_nothing 2. authorize_returns_held_role 3. rename_converges_across_clients 4. stale_and_foreign_events_ignored 5. archive_owner_only_and_converges 6. paginate_bounds 7. title_bounds_reject_empty_and_oversize

## Decisions
- Member may rename (convergence by version); archive owner-only. Rename while archived allowed (title update, version bump) — archive flag sticky.
- Same-version conflict: first-writer wins (equal version ignored).

## Remaining unknowns
- Wiring into `lib.rs` / server slice: integrator-owned, not this lane.
