# WEB-009 runtime policy integration

Status: candidate runtime-policy seam GREEN; parent remains BLOCKED and NOT ACCEPTED.
Owner/session: `ses_f3c4de578ffelQv59xDXmOs03B`.
Candidate base: `20c5f86` on `lane/WEB-006-integration`.

## Frozen contract and source evidence

- Frozen RED: `crates/server/tests/runtime_wiring_policy_http.rs`, SHA-256
  `dc543a5cc9a950d135c75cb0b4765325c5782401d8fffeebe012bf2b7567b6c7`.
  The committed test compiled and failed only at line 318 before implementation:
  request-local `OPENCODE_RK_TURN_TOOLS=bash` executed despite an injected
  `EnginePolicy::default_deny()`.
- `crates/server/src/app_runtime.rs:193` defines deny-first engine policy lookup.
- `crates/server/src/app_runtime.rs:586-605` snapshots registered tool IDs and
  enabled state; `EngineHandles::policy` at line 627 is the daemon-owned policy.
- Before this patch, `crates/server/src/lib.rs::create_turn_stream` derived both
  advertised and executable tools solely from the ambient environment list, while
  `TurnStreamStage::Executing` treated that list as permission to reach the
  request-local security broker and executor.

## Implemented boundary

- When `Extension<RuntimeWiring>` is present, `create_turn_stream` intersects the
  explicit environment allowlist with the runtime's enabled tool snapshot and
  `EnginePolicy::Allow` decision before constructing provider tool definitions.
- The same intersected list is retained in `TurnStreamState`, so an unsolicited
  provider function call denied by the runtime reaches neither the security broker
  nor `ToolExecutor`; it produces the existing bounded denial output instead.
- Legacy routers without a runtime extension preserve their existing behavior for
  compatibility. The existing request-local `PermissionBroker` remains an
  additional authorization boundary for calls allowed by the runtime.
- Denial has zero tool side effects in the frozen fixture, is returned to provider
  round two as `function_call_output`, and is persisted once as the tool message.

## GREEN evidence

All commands used `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1` and
`--test-threads=1`.

- `cargo test -p opencode-rk-server --test runtime_wiring_policy_http` — 1/1.
- `cargo test -p opencode-rk-server --test agent_loop_turns` — 1/1.
- `cargo test -p opencode-rk-server --test runtime_wiring_http` — 1/1.
- `cargo test -p opencode-rk-server --test session_turn_stream_api` — 2/2.
- `cargo test -p opencode-rk-server runtime_wiring --lib` — 6/6.
- Frozen test hash remained exact and `git diff --exit-code 20c5f86 --
  crates/server/tests/runtime_wiring_policy_http.rs` was clean.
- `python3 tools/validate_repository.py` remained blocked by 51 pre-existing
  backlog-exhaustion reconciliation findings; protection-policy checks passed.
- Tests used only loopback provider fixtures, temporary directories and in-memory
  storage. No user database or real credential was read.

## Bounds and remaining gaps

- No queue, process, task, retained output or permit was added. The runtime tool
  snapshot and policy are immutable daemon-owned values; per-request intersection
  is bounded by the existing registry and policy caps.
- Live execution still constructs a request-local `ToolRegistry`,
  `PermissionBroker` and `ToolExecutor`; this patch only makes daemon policy and
  snapshot authoritative as a fail-closed outer intersection.
- Runtime event publication and per-turn cooperative cancellation remain unwired.
- Native durable tool-call identity/state, references/citations, browser-disconnect
  cancellation, and WEB-009 accessibility requirements remain unresolved.
- The convergence gate remains blocked by 52 pre-existing ledger/backlog findings.
  This patch is candidate integration evidence, not parent completion or release
  acceptance.
