# V2-LIVE-INTERRUPT-CONTRACT — authenticated session interrupt

State: **CANDIDATE**; runtime RED/freeze/product acceptance PENDING.

## Package

LIVE-INTERRUPT-CONTRACT. Source-only test owner lease. No Cargo/build/runtime/
network/PTY/container gates executed by this worker. Integration writer Xkiro
ses_f09110167 owns heavy canonical mutations.

## Upstream authority (pinned 95daf90670b7c039c436c85537da5fbfe2205b41)

| Symbol | Location | Behavior |
|--------|----------|----------|
| `session.interrupt` protocol | `packages/protocol/src/groups/session.ts:345-356` | `POST /api/session/:sessionID/interrupt`, singular, returns 204 NoContent |
| Handler | `packages/server/src/handlers/session.ts:366-370` | Calls `session.interrupt(ctx.params.sessionID)`, returns `HttpApiSchema.NoContent.make()` |
| Runner idle no-op | `packages/opencode/src/effect/runner.ts:94-101` | `finishShell`: if state is Idle, no-op; returns to Idle |
| Runner active cancel | `packages/opencode/src/effect/runner.ts:108-113` | `stopShell`: awaits ready, succeeds cancelled Deferred, calls `Fiber.interrupt(shell.fiber)` |

Key contract: interrupt is SINGULAR per sessionID. Idle sessions return 204
no-op. Active sessions fire the cancellation fiber. The response is always
204 NoContent with empty body on success. Missing/wrong auth is handled by
the middleware layer before the handler runs.

## Current Rust state

| Component | Location | Status |
|-----------|----------|--------|
| Router registration | `crates/server/src/lib.rs:182-244` | Has `/api/sessions/{id}/turns` and `/api/sessions/{id}/turns/stream`. NO `/api/sessions/{id}/interrupt` route registered. |
| Bearer middleware | `crates/server/src/daemon_auth.rs:149-176` | Gates `/api/*` paths. Missing → 401 UNAUTHORIZED. Wrong → 403 FORBIDDEN. Public paths (`/health`) pass through. |
| Turn lifecycle | `crates/server/src/turn_service.rs:421-434` | `Turn::interrupt()` fires CancelToken and transitions to `TurnPhase::Cancelled`. Terminal: no transitions out. Idempotent cancel signal. |
| Turn permit | `crates/server/src/lib.rs:114-167` | `TurnExecutionPermit` uses global semaphore (max 2) + per-session slot array. Drop reclaims slot. |
| create_turn | `crates/server/src/lib.rs:833-897` | Non-streaming turn: acquires permit, appends user, calls provider, appends assistant. |
| create_turn_stream | `crates/server/src/lib.rs:1025-1139+` | Streaming turn via `stream::unfold` over `TurnStreamState`. Holds permit for duration. |
| ApiFailure | `crates/server/src/lib.rs:1728-1797` | Typed error responses: bad_request(400), not_found(404), too_many_requests(429), etc. |

**Gap:** The `TurnService::Turn::interrupt()` state machine exists but is NOT
wired to any HTTP endpoint. There is no `POST /api/sessions/{id}/interrupt`
handler in `lib.rs`. This is the genuine product RED these tests detect.

## Test design

Five cases parameterized over auth/presence, one coherent public caller fixture.
All tests use the real `router_with_auth` + minted `DaemonAuth` + in-memory
`Storage`/`SessionService`/`Catalog`. No isolated internal registry or dummy
HTTP codes. Raw TCP client matches `session_execution_ownership.rs` pattern.

Controlled streaming provider emits first delta immediately then holds until
explicit release via Condvar. Tests observe natural provider peer EOF/reset
BEFORE cleanup/fixture manual release.

### Case matrix

| # | Name | Auth | Session state | Expected status | Key assertions |
|---|------|------|---------------|-----------------|----------------|
| 1 | `interrupt_streaming_authorized_returns_204_and_reclaims_slot` | Valid Bearer | Active streaming turn | 204 NoContent, empty body | Original stream reaches EOF; durable user kept; no assistant persisted; same-session new prompt completes (not 429); slot reclaimed |
| 2 | `interrupt_missing_bearer_returns_401_no_cancel` | None | Active streaming turn | 401 UNAUTHORIZED | Provider still running (not cancelled); positive completion after release |
| 3 | `interrupt_wrong_bearer_returns_403_no_cancel` | Wrong 64-hex | Active streaming turn | 403 FORBIDDEN | Provider still running; turn completes normally |
| 4 | `interrupt_idle_session_is_noop` | Valid Bearer | No active turn | 204 NoContent | Repeated interrupts are no-ops; zero provider contacts |
| 5 | `interrupt_other_session_is_noop_for_first` | Valid Bearer | A active, B idle | 204 for B's interrupt | A's turn not cancelled; A completes with durable messages |

### Resource bounds

- Body limit: 256 KiB response wire
- Provider wire: 128 KiB request bound
- Max 2 concurrent provider workers accepted (blocking on macOS)
- Retry transients within absolute 5s windows, not reset deadline
- Bounded HTTP spawn_blocking waits
- Per-case: ≤20s absolute monotonic deadline
- Suite: ≤90s total
- Explicit release fallback + stop + JOIN provider on ALL paths
- Axum server abort + await on ALL paths
- Cancellation compared BEFORE forced cleanup (no false GREEN)

### Environment isolation

- Disposable HOME/XDG via `tempdir()` (matches `session_execution_ownership.rs`)
- All env vars restored in `EnvGuard::drop`
- Test mutex (`ENV_LOCK`) serializes env mutation
- Generated fake API key (`fixture-interrupt-key-a1b2c3d4`), never user secrets
- In-memory storage + temp blob directory
- No user database access

### Route path convention

Upstream uses SINGULAR `/api/session/:sessionID/interrupt`. The test preserves
that pinned public path exactly. Current Rust turn/message routes are plural, but
there is no interrupt route; the integrator must register the singular endpoint.

## Changed paths

Exact two files within grant:

1. `crates/server/tests/session_interrupt_live.rs` — new test file (source only)
2. `worklog/V2-LIVE-INTERRUPT-CONTRACT.md` — this worklog

## Verification commands (for integrator, not run by source owner)

```text
# Static checks
rustfmt --check crates/server/tests/session_interrupt_live.rs
git diff --check

# Runtime (integrator scope)
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test --offline --locked \
  -p opencode-rk-server --test session_interrupt_live \
  -- --test-threads=1 --nocapture
```

Expected result: compilation succeeds, all 5 tests FAIL (genuine RED) because
the `/api/session/{id}/interrupt` route does not exist in the current router.
Cases 2-3 fail at 401/403 assertions (middleware rejects before reaching a
non-existent handler). Cases 1,4,5 fail at 204 assertions (route not found
returns 404 from axum fallback, not 204).

## Base and candidate

- Base SHA: `15b6fbb03cffe3c2f5434e9c28f7c84f4361b32b`
- Candidate SHA: recorded in the handoff after commit
- Branch: `v2/live-interrupt-contract-y52o0gi3`
- Worktree: `/Users/mymac/Projects/opencode-rk-v2-interrupt-contract-y52o0gi3`

## Authority gap report

No conflict between upstream authority and source implementation. The upstream
protocol defines `POST /api/session/:sessionID/interrupt` returning 204. The
Rust `TurnService` already models `Turn::interrupt()` → Cancelled. The gap is
purely the missing HTTP route registration and handler wiring in `lib.rs`,
which is a product implementation task for the integrator, not a design
question or blocker. The tests compile against existing public APIs and detect
this absence as genuine RED.

</content>
