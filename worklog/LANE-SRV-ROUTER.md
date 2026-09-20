# LANE-SRV-ROUTER — delete legacy open router() alias

Claim: LANE-SRV-ROUTER, session ses_f423cd0e2ffejOqhPZZSuaZbMD.
Source evidence:
- crates/server/src/lib.rs:111-113 `pub fn router(state) -> Router { router_with_auth(state, None) }` (rev 6b19524).
- crates/server/src/lib.rs:118 `router_with_auth(state, Option<DaemonAuth>)`; :170-176 `Some=>require_bearer` layer, `None=>app` open.
- crates/cli/src/main.rs:684 serve path already `router_with_auth(AppState{..}, Some(credential))` — fail-closed, credential minted :673 + published :674.
- daemon_auth_api.rs + discovery_auth_red.rs use `router_with_auth(.., Some(..))`.
- ~17 integration test files use bare `router(..)` (frozen, NOT touched per lane bounds; migration is orchestrator's job).

Observed scenario: bare `router()` is a convenience unauthenticated constructor; only serve path matters for prod and it passes `Some`.
Target boundary: delete alias only. Keep `router_with_auth(state, Option<DaemonAuth>)` signature unchanged so auth-test files still compile; do NOT touch tests, CLI, other lanes.
Tests: `cargo test -p opencode-rk-server --lib` (unit only; integration breakage from deleted alias is out of scope per task VERIFY).
Decisions: minimal 3-line deletion, doc comment kept (still accurate: None = test-only, serve passes Some).
Remaining unknowns: orchestrator must migrate frozen `router(..)` callers to `router_with_auth(.., Some(..))` or test creds.

## Landing attempt 1 (2026-09-20)
- Edit applied: lib.rs alias deleted (diff: 4 deletions, only router_with_auth remains at :114).
- `cargo test -p opencode-rk-server --lib`: ok, 205 passed 0 failed (warnings pre-existing: unused Duration/id2/id3).
- Claim already in-progress under my session; completed update + commit+push next.
