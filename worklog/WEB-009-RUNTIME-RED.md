# WEB-009 runtime permit RED

Status: blocked; RED only. Claimed by `ses_f37b17e2cffepkbIqiPrne9M2j`.

## Evidence

- Candidate commit: `5d78d94`.
- `crates/server/src/lib.rs:107-115`: `AppState` has exactly `sessions` and
  `catalog`; `router` accepts that compatibility literal.
- `crates/server/src/lib.rs:877-891`: `create_turn_stream` currently acquires
  process-local `TURN_PERMITS` before empty-text validation.
- `crates/server/src/runtime_wiring.rs:305-357,572-575`: `RuntimeWiring` owns
  daemon `TurnPermits`, exposes `available_permits`, and is cloneable.
- `crates/server/src/app_runtime.rs:545-572`: shared permit acquisition is
  bounded and owned; dropping permits reclaims capacity.

## Contract

One disposable real `SessionService` session. Exact two-field `AppState`
literal. Runtime extension holds all shared permits (bounded count, acquired
exactly once per available permit). POST valid JSON with intentionally empty
text must be denied by shared runtime first: `429`, no message persistence,
runtime availability remains zero while permits are retained. Legacy routers
without extension remain compatible through fallback behavior.

## RED command/result

Command:

```text
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test runtime_wiring_http -- --test-threads=1
```

Observed RED: compilation succeeds; test fails at status assertion,
`left: 400`, `right: 429`. Current handler ignores the extension, uses its
static permit, then rejects empty text. No provider, credentials, or network
access occurs. Test source SHA-256 is recorded after the valid RED run.

## Implementation proposal

Handler should accept optional `Extension<RuntimeWiring>`, use its shared permit
when present, retain static permit fallback for legacy routers. Production CLI
must layer the real runtime and retain `EngineLease`. Shared
`OwnedSemaphorePermit` stays in stream state for stream lifetime.

Frozen test SHA-256: `5a2572d2339a53036c236062fef7f7ff9136e25ed7472988804ef5ab96e6ef3e`.
Exact output: `assertion left: 400 right: 429`; one test failed, no compile/provider/network error.

Unknown: exact error body code/message is intentionally not asserted; status is
the stable boundary. No product implementation performed.
