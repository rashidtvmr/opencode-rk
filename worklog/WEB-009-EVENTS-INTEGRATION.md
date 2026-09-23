# WEB-009 runtime event integration

Status: candidate event-publication seam GREEN; parent remains BLOCKED and NOT ACCEPTED.
Owner/session: `ses_f3c4de578ffelQv59xDXmOs03B`.
Candidate base: `f0be008` on `lane/WEB-006-integration`.

## Frozen contract and source evidence

- Frozen RED: `crates/server/tests/runtime_wiring_events_http.rs`, SHA-256
  `a4496430f11fd1cb02e8bc728bc8d1f0abcecd81c233ca895a505c2c00bf5223`.
  The committed test compiled, completed the real two-round tool journey and
  persisted User/Tool/Assistant, then failed only because the first daemon event
  receive timed out at line 350.
- The RED author's scratchpad and committed bytes contain the exact hash above;
  its initial ledger note had a one-character stale typo (`...505a2...`). The
  candidate ledger note below records the verified committed hash.
- `crates/server/src/runtime_wiring.rs::RuntimeWiring::events` exposes the one
  daemon-owned bounded `EventBus`.
- `crates/server/src/event_bus.rs::ServerEvent` provides `MessageAppended` and
  `ToolExecuted`; no durable message sequence currently exists in
  `MessageRecord`, so this seam preserves the established `seq: 0` convention.
- Before this patch, HTTP routes wrote directly through `SessionService` and
  `ToolExecutor` without publishing the injected runtime bus.

## Implemented boundary

- Runtime-backed non-stream and streaming turns publish `MessageAppended` only
  after the corresponding user or assistant durable append succeeds.
- `TurnStreamState` retains only a cheap clone of the bounded daemon event bus.
- After each tool row append succeeds, the stream publishes `ToolExecuted` and
  then `MessageAppended`, preserving the frozen observable order while avoiding
  false events on persistence failure. Denied outcomes use the same durable path.
  Overflow budget notices have no tool name and therefore publish only the
  durable-message event, never a fabricated empty-name execution event.
- Event publication is non-blocking through the existing bounded bus; a full
  subscriber cannot stall provider, persistence or tool execution.
- Legacy two-field `AppState` routers without `RuntimeWiring` publish no daemon
  events and preserve their existing behavior.

## GREEN evidence

All commands used `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1` and
`--test-threads=1`.

- `cargo test -p opencode-rk-server --test runtime_wiring_events_http` — 1/1.
- `cargo test -p opencode-rk-server --test runtime_wiring_policy_http` — 1/1.
- `cargo test -p opencode-rk-server --test agent_loop_turns` — 1/1.
- `cargo test -p opencode-rk-server --test session_turn_activity_api` — 2/2.
- `cargo test -p opencode-rk-server --test runtime_wiring_http` — 1/1.
- `cargo test -p opencode-rk-server --test session_turn_api` — 1/1.
- Frozen event test remained byte-identical to `f0be008` at the exact hash.
- `python3 tools/validate_repository.py` retained 51 pre-existing
  backlog-exhaustion failures; repository protection-policy checks passed.
- Fixtures used loopback provider servers, temporary directories, in-memory
  storage, a fake key and one fixed harmless echo command only.

## Bounds and remaining gaps

- No queue, task, thread, process, permit or retained output was added. The
  existing event bus remains capped at 256 entries per subscriber and uses
  `try_send` rather than awaiting a slow consumer.
- Per-turn/browser-disconnect cancellation is still not wired. The runtime's
  process-wide `CancelState` was deliberately not reused for this purpose.
- Live execution still constructs a request-local registry, broker and executor;
  daemon tool policy is authoritative only through the outer intersection landed
  at `81fba08`.
- Durable native tool-call identity/state, references/citations and accessibility
  requirements remain unresolved; WEB-009 cannot be marked complete.
- Repository/convergence validation remains blocked by pre-existing
  backlog/ledger reconciliation findings. This is candidate evidence only.
