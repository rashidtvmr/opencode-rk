# AGENT-LOOP — bounded agentic tool loop over live turns

Status: IMPLEMENTED (lane evidence; verifier decides acceptance).
Base: post-225fed4 (provider stream_with_tools). Date: 2026-09-18.
Bounds: `crates/server/src/agent_loop.rs` (NEW), `crates/server/src/lib.rs`
(turn-stream wiring + honest capabilities), `crates/server/tests/agent_loop_turns.rs`
(NEW). No test file outside this lane modified.

## Cited gaps closed

- crates/server/src/lib.rs (pre-lane): "the current web Responses turn adapter
  does not execute native tools" — tool calls were announced (`tool_call`
  NDJSON) but never executed, and no result was ever fed back.
- No iteration cap existed in the live turn path (MAX_STEPS existed only in
  disconnected state machines).
- Stop reasons were a debug-format string; `max_steps` termination did not exist.

## Contract (frozen RED: tests/agent_loop_turns.rs, 4 scenarios, one fn)

- E1 (enable via OPENCODE_RK_TURN_TOOLS=bash): provider round one returns
  text + a bash `function_call`; the server executes it through the real
  ToolExecutor (`echo loop-fixture-ran` appears in the provider's round-two
  input as a `function_call_output`), persists user/assistant/tool transcript
  rows, streams `tool_call` + `tool_output` NDJSON events, and terminates with
  `stop_reason: completed` after round two's answer.
- E2 (OPENCODE_RK_TURN_MAX_STEPS=1): exactly one provider round; turn
  terminates `max_steps:N`; the fixture serves one round only — a second
  provider connection would be a contract breach.
- E3 (default): tool call is NOT executed; output says "not permitted" and
  that denial is fed back to the provider in round two.
- E4: /api/capabilities marks a tool `available_for_web_turn: true` iff the
  OPENCODE_RK_TURN_TOOLS allowlist admits it, with the reason string citing
  the env var either way.

## Implementation shape

- `agent_loop.rs` (pure state, std-only): `LoopController` (cap N = at most N
  provider rounds; `can_start_next_round()` treats round one as implicit),
  `TurnStop` wire strings (`completed`, `incomplete[:reason]`, `max_steps:N`,
  `policy_denied:tool`), `truncate_calls` (per-round call cap 16 with explicit
  error outputs), `truncate_tool_output` (16 KiB visible cut marker),
  `function_call_output` wire builder. 6 in-file scenario tests (lib suite).
- `lib.rs` turn stream: stages User -> Provider -> (Executing -> EmitOutputs
  -> NextRound)* -> Done. Function calls are appended to the typed history
  when received (replay rule: call before output). Round outputs persist as
  MessageRole::Tool rows and as ResponsesItem::FunctionCallOutput. Forced
  stop path finalizes the transcript with the cap reason.
- Deny-by-default: tools are advertised to the provider only when listed in
  OPENCODE_RK_TURN_TOOLS (comma-separated ids); an advertised tool is
  executable, an unadvertised one is not. Empty by default.

## Gates

| Gate | Result |
|---|---|
| cargo test -p opencode-rk-server --test agent_loop_turns | 1/1 (4 scenarios) |
| server lib suite (incl. 6 agent_loop unit tests) | 125+ passed |
| server full parallel regression | 277 passed; only the 2 documented serial-mandate stream tests fail in parallel and pass 2/2 with --test-threads=1 |
| session_turn_api + web_capabilities_api/full (frozen wire-compat guards) | all green |
| providers + cli suites | 647/647 |
| release build opencode-rk-cli | OK |

## Remaining known limits (honest)

- bash/schema: advertised tool JSON schemas are minimal empty-object
  parameters (registry does not carry parameter schemas yet).
- Policy engine (app_runtime::EnginePolicy) is not yet consulted in the turn
  path; the env allowlist is the only gate. Approval-flow integration is a
  separate slice.
- Non-streaming `/turns` (used by the TUI today) still uses the single-shot
  create() path; the loop is on `/turns/stream`. TUI migration is follow-up.
