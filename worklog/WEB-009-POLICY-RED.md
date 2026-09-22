# WEB-009 runtime policy RED

Status: BLOCKED — compiling semantic RED captured.
Owner/session: `ses_f3768887affeJ07LIbxDfSkAOt`
Base: `2fcfe90` on `lane/WEB-006-integration`.

## Source evidence

- `crates/server/src/lib.rs:938-1000` (`create_turn_stream`): current stream builds request-local `ToolRegistry`, advertises tools from `OPENCODE_RK_TURN_TOOLS`, stores request-local `enabled_tools`.
- `crates/server/src/lib.rs:1170-1235` (`TurnStreamStage::Executing`): execution permits a call from `enabled_tools`, then calls `ToolExecutor::execute`; no injected `RuntimeWiring.engine().policy` check.
- `crates/server/src/runtime_wiring.rs:322-345` (`RuntimeWiring::for_daemon`/`with_sessions`): daemon composition owns sessions, tools, `EnginePolicy`; `engine()` exposes shared handles.
- `crates/server/src/app_runtime.rs:130-218` (`EnginePolicy`): `default_deny()` denies unspecified tools; `decision()` deny-by-default.
- `crates/server/tests/agent_loop_turns.rs:130-164,275-377`: bounded scripted SSE provider, request/body bounds, real stream fixture, env setup, persisted message assertions.

## Contract

With `OPENCODE_RK_TURN_TOOLS=bash` (request-local advertisement), injected daemon `RuntimeWiring` uses `EnginePolicy::default_deny()` and must remain authoritative. A real provider `bash` FunctionCall is represented, but execution produces explicit denial, zero echo side effect, round-two `function_call_output` repeats denial, persisted tool message records denial. Bounded loop: two provider requests, bounded request body/read timeout, disposable in-memory session/blob storage, fixed `echo` argv data only.

## RED evidence

Command (required initial command, corrected only for shell env syntax):
`env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test runtime_wiring_policy_http -- --test-threads=1`

Result: compiled; 1 test failed at `crates/server/tests/runtime_wiring_policy_http.rs:318`: `assertion failed: denied.contains("denied") || denied.contains("not permitted")`. This is the intended semantic mismatch: request-local allowlist bypassed injected runtime policy; failure occurred after provider parsing and real function-call/tool-output path, not timeout or fixture setup. Initial attempted `CARGO_BUILD_JOBS=1 ...` without `env` was shell error (`rtk: No such file or directory`), not test evidence.

Frozen test SHA-256: `dc543a5cc9a950d135c75cb0b4765325c5782401d8fffeebe012bf2b7567b6c7`

Reconciliation: `5466ca...` was recorded before the final test-file write and was
stale. The committed bytes at `c6b9284` and the unchanged working-tree bytes now
hash to `dc543a...`; these committed bytes are the frozen contract.

## Implementation proposal

At current execution branch, consult injected `runtime.engine().policy` plus runtime tool snapshot before `ToolExecutor`; unknown or denied calls must execute zero side effects. Preserve one existing persistence path for tool records and function-call outputs. Avoid blindly calling `EngineHandles::dispatch_tool`: it mints a separate `AgentId` and persists its own output. Runtime event publication/cancel remain separate follow-up boundaries.

## Unknowns

No product changes made. Parent remains not accepted; native structured activity/reference persistence remains incomplete.
