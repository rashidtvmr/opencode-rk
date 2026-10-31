# FIX-STREAM worklog

> **ORCHestrator ADDENDUM (ses_buffy_orch_1, 2026-09-20 late):**
> The env race is now FIXED. Root cause confirmed exactly as this worklog
> hypothesizes: both frozen tests mutate the process-global OPENAI_BASE_URL
> through unlocked EnvGuards while from_env() reads it per request.
> A human-approved contract-review resolution APPLIED the env_lock patch
> (sync-only, zero assertion changes) to
> crates/server/tests/session_turn_stream_api.rs — it mirrors
> session_turn_activity_api.rs:21-24 per worklog/TURN-STREAM-GATE.md §6.
> Evidence: serial `--test-threads=1` 2/2 GREEN, then parallel default 3x
> runs 2/2 GREEN (pre-fix parallel was 3/3 FAIL).
> DO NOT revert the env_lock in the frozen test file. If your lane considers
> the test diff out of scope, do nothing to it and report; it is a
> documented, human-approved harness repair, not an implementer test edit.

Claim: FIX-STREAM, session ses_f40414a24ffebu5vbL7mjjf5j6. Owned file: crates/server/src/lib.rs (turn path only). No test edits, no commit/push.

## Source evidence
- `crates/server/src/lib.rs:877` `create_turn_stream` — handler entry.
- `crates/server/src/lib.rs:908` `OpenAiResponsesClient::from_env()` pre-stream read (global env).
- `crates/server/src/lib.rs:956` `stream_with_tools` first round.
- `crates/server/src/lib.rs:1306` `OpenAiResponsesClient::from_env()` re-read inside `NextRound` unfold (mid-stream).
- `crates/server/src/lib.rs:832` `loop_control: LoopController` in `TurnStreamState` (no client stored).
- `crates/providers/src/responses.rs:455` `from_env()` — builds client from process-global env.
- `crates/providers/src/config.rs:84` `ProviderConfig::from_env("openai")` reads `OPENAI_BASE_URL`, `OPENAI_API_KEY_ENV`, etc. via `std::env::var`.
- `crates/providers/src/config.rs:149` `get_api_key()` reads `env::var(&self.api_key_env)` — also global.
- `crates/server/tests/session_turn_stream_api.rs:42` `EnvGuard::set` — `env::set_var` on shared process env, NOT locked (no mutex).
- Tests `:285` T1 streaming-success fixture, `:364` T2 failure fixture; each sets distinct `OPENAI_BASE_URL` per test.

## Observed scenario
- Baseline 62f7eb1, lib.rs untouched (git diff empty on owned paths).
- Serial `--test-threads=1`: 2/2 GREEN.
- Parallel default: RUN1 1pass/1fail, RUN2 0/2, RUN3 0/2. Nondeterministic across runs.
- RUN3 tails: T2 `:383` got `events[1]=="assistant_delta"` want `"error"` (T2 stream served by T1 success fixture); T1 `:302` missing `201 Created` + fixture `:136` release Timeout (T1 client went to wrong fixture or read env post-drop → MissingCredential 503).
- Mechanism: both tests mutate the same process-global env concurrently; handler reads at :908 and :1306 can observe the sibling test's URL. Impl cannot attribute global env per test.

## Target boundary
- Challenge hypothesis: capture endpoint/credentials once at stream creation, thread explicitly through stream state. Prototype in owned file only, measure, revert if race persists.

## Tests
- Serial + 2x parallel tails recorded below. Zero test edits.

## Decisions
- Prototype (capture endpoint/credentials once, thread through stream state)
  REVERTED. Serial stayed GREEN 2/2, but parallel stayed flaky: run1 0/2
  (T1 :302 + fixture :136 Timeout, T2 :383 assistant_delta-vs-error),
  run2 1/2 (T2 :380 missing 201). Identical signature to baseline
  (baseline parallel: RUN1 1/2, RUN2 0/2, RUN3 0/2). Fix did not move the
  distribution — expected, since entry read :908 still races before any
  capture. Reverted `crates/server/src/lib.rs` byte-identical; owned diff empty.
- No guessing, no test edits, no provider-crate edit (unproven call path +
  forbidden without minimal-call-path proof; Clone on client would also only
  cover post-entry reads).

## Remaining unknowns
- None on mechanism. Parallel GREEN needs test-side env isolation
  (serializes EnvGuard or per-test keying) — out of lane authority.

## Remaining unknowns
- None on mechanism. Open: whether capture-once changes parallel outcome distribution (predict: no — entry read :908 stays racy).
