# LANE-STREAM-FIX — session_turn_stream_api regression (blocked, evidence)

Claimed by ses_orch_wave3 (orchestrator) 2026-09-19. Status: BLOCKED — regression predates
wave-3; fix belongs to the turn-stream/fixture owner lane (WEB-009/012 surface).

## Reproduction (exact)

```
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-server --test session_turn_stream_api
```
- HEAD `741e8c9` (dirty tree AND pristine `git worktree` at HEAD): 2/2 FAIL
- Bisect on pristine worktrees: FAIL at `ee32e73`, `dff7cce`, `832ab7a`, `421d0fd` (17:48, TUI-002)
- T1 `session_turn_stream_forwards_deltas_before_completion_and_persists_once`:
  `session_turn_stream_api.rs:307` assert left=2 right=4 (expects user + 2 deltas + final)
- T2 `session_turn_stream_reports_provider_failure_without_persisting_assistant`:
  `:380` response does not start with `HTTP/1.1 201 Created`

## Attribution

Wave-3 (this session) server changes: additive modules `loop_driver_live.rs` /
`rules_globs_live.rs` tests only — no turn-stream edits. Every tree since 17:48 fails,
so the regression was committed by an earlier lane and never caught because no one ran
the full server suite between those landings (each lane ran only its own targets).

## Fix path (owner)

`crates/server/src/lib.rs` turn-stream handler + `spawn_streaming_openai_fixture` in
`crates/server/tests/session_turn_stream_api.rs` consumers — likely fixture/NDJSON
round-trip drift from the WEB-009/012 transcript work. Do NOT edit the frozen test.
