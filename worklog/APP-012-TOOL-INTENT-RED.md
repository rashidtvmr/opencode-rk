# Worklog: APP-012-TOOL-INTENT-RED (test-author, RED-only)

## Claim
- Task: APP-012-TOOL-INTENT-RED, type test-author, role worker.
- Session: ses_f218525b5ffeOFVGPHuaWfu46V. Ledger: claimed `in-progress`
  via tools/completion_claims.py before any edit.
- Owned file (ONE): `crates/server/tests/live_tool_broker.rs` (new).
- Other writable: this scratchpad + own ledger row only.
- Route: user-requested Muse Spark 1.3 Contributor via available OpenCode
  provider (9router alias unavailable); executed on opencode/Muse Spark 1.3
  Free. No user allowlist in delegation -> canonical N/A; route matches
  delegation.
- RED-only lane: final ledger status will be `blocked` (implementer lane to
  follow); no merge/main, push branch only.

## Source evidence (base 9cf998a, branch red/APP-012-TOOL-INTENT)
- `crates/security/src/lib.rs:243-254` — `decide()`: `OperationIntent::Tool`
  baseline is unconditional `Decision::Allow`. Permission rules can still
  deny (authorize() layers rules over baseline, first-match-wins).
- `crates/security/src/lib.rs:206-227` — baseline-first: mandatory baseline
  deny/human-gate is never lifted by `*` Allow.
- `crates/server/src/lib.rs:1241-1243` — live `create_turn_stream` builds a
  FRESH `PermissionBroker::new(lean_default(cwd))` with default (empty)
  permissions; no knob injects Deny rules or grants into the live broker.
- `crates/server/src/lib.rs:1327-1375` — shell pre-spawn authorizes generic
  `Tool{name}` (always Allow on the fresh broker), never the real effect
  intent (`tool_authorize::shell_intent` -> Process `bash -c` -> RequireHuman).
- `crates/server/src/lib.rs:1539-1574` — non-shell dispatch authorizes generic
  `Tool{name}` then `ToolExecutor::execute`.
- `crates/tools/src/executor.rs:106-119` — executor only runs
  bash/shell/echo; `write` -> `Unknown tool: write` error, no side effects.
- `crates/security/src/tool_authorize.rs:81-90` — documented design:
  file-backed tool args additionally bind `FileAction::Write` so
  project-root/destructive gates apply. Live path does NOT do this (missing
  behavior, not missing infra: all APIs used by the RED exist today).
- `crates/server/tests/agent_loop_turns.rs:274-340` (LANE-LOOP-LIVE,
  completed/landed) — E1: enabled bash `echo loop-fixture-ran` MUST execute
  over live HTTP. Immutable; RED scenarios must be satisfiable alongside it.
  Consequence: RED uses the `write` tool to a disposable secret path
  (`<tmpdir>/.env`), never bash-content gating.
- `crates/server/tests/runtime_wiring_policy_http.rs` — untrusted reference
  for the scripted-provider + live-stream harness pattern (copied bounds:
  256 KiB provider requests, 20 s read timeout, 64 KiB oneshot bodies).
- `docs/CONVERGENCE.md:63` — "live turn path must not directly bypass
  authorization with bare ToolExecutor" (this lane's spine).
- Convergence gate `python3 tools/convergence_gate.py`: CONVERGENCE BLOCKED,
  total=84, all pre-existing ledger/off-plan findings (none caused by this
  lane). This RED lane is explicitly delegated spine work; proceeding and
  recording the gate state here.

## Observable contract / failure states
- Denied op: provider-requested `write` with file-backed args targeting a
  secret path. Real broker denies `File::Write` to `*/.env` even under `*`
  Allow (secret baseline, lib.rs:257-262) and even with deny-first rules.
- Required live behavior: tool_output reports broker denial
  ("denied"|"not permitted"|"requires human approval" disjunction, same as
  frozen runtime_wiring_policy_http.rs), sentinel file absent (ZERO file side
  effects), no process result, turn still completes via round 2.
- Pre-GREEN observed: output is `Unknown tool: write` (no tool-intent
  authorization path exists) -> denial assertion FAILS = compiling RED.
- Ownership/lifetime: tempdir owns sentinel + storage blobs; provider fixture
  thread joined; server task aborted; env restored via EnvGuard.
- Bounds: provider req <= 256 KiB, server raw <= 512 KiB, oneshot <= 64 KiB,
  20 s socket read timeout, 120 s owned cargo timeout, JOBS=1/THREADS=1.
- Security: no real user DB (in-memory Storage + tempdir); no secret access
  (disposable `.env` with innocuous fixture content, existence-check only,
  never logged); no network beyond loopback fixtures.

## Tests (frozen after RED run, no edits post-freeze)
1. `broker_deny_beats_wildcard_for_tool_effect_intents` — guard (passes):
   star-only broker still denies `.env` write + `rm -rf *`; ordinary write
   Allows; deny-first rules deny.
2. `stale_grant_cannot_release_denied_tool_effect` — guard (passes): fake
   grant issuer (manufactured Grant, stale policy version) -> Deny +
   run_if_allowed runs zero effects.
3. `live_denied_file_write_has_zero_side_effects` — THE RED (fails pre-GREEN
   on the denial-wording assertion; sentinel-absent + completion guards pass).

## Decisions
- `write`-to-disposable-`.env` (not bash): E1-compatible, needs no command
  string parsing from GREEN (structured file-backed args, symmetric to the
  landed `read` sibling lane APP-012-TOOL-RED), secret-deny is
  root-independent so GREEN's File::Write binding denies regardless of cwd.
- Single env-mutating test fn; broker fns touch no env -> parallel-safe, and
  the owned run uses RUST_TEST_THREADS=1 anyway.
- Did not assert fixture-content absence in outputs (would over-constrain
  GREEN's denial text); denial disjunction mirrors the frozen reference.

## Remaining unknowns / handoff
- GREEN wiring mechanism is the implementer's choice; RED passes once live
  dispatch binds file-backed args to File intents through the real broker.
- Verifier re-runs frozen tests independently; this lane stays `blocked`
  (RED-only) until GREEN lands.

## RED receipt (frozen, no test edits after this point)
- RED run: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test
  -p opencode-rk-server --test live_tool_broker -- --test-threads=1`
  -> 2 passed (T1 broker guard, T2 stale-grant guard), 1 failed (T3 live).
- T3 failure (missing behavior, not infra): full live path executed
  (session, 2 provider rounds, `write` advertised, tool_call + tool_output
  events present); denial assertion failed with
  `got: Unknown tool: write` at live_tool_broker.rs:528.
- Frozen test SHA256: 9a9e8424f3ff7ca9e1fbc6c4951bf7fa4044ea9a1ffdc8e8b56e378e5abd0eb6
  (569 lines). No edits to the test file after freeze.
