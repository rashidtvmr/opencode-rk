# WEB-009 daemon runtime integration

Status: candidate shared-permit/session wiring GREEN; parent remains BLOCKED and NOT ACCEPTED.
Owner: `ses_f3c4de578ffelQv59xDXmOs03B`.

## Authority and evidence

- Candidate base: `62f21b2` on `lane/WEB-006-integration`.
- Task: `tasks/WEB-009.md` is mandatory and explicitly remains in progress.
- Frozen RED: `crates/server/tests/runtime_wiring_http.rs`, SHA-256
  `5a2572d2339a53036c236062fef7f7ff9136e25ed7472988804ef5ab96e6ef3e`.
  It compiled and failed `400 != 429` before implementation.
- `crates/server/src/runtime_wiring.rs::RuntimeWiring` is the daemon-owned,
  cloneable runtime. Its bounded permit pool is released by owned-permit Drop.
- `crates/server/src/lib.rs::AppState` remains exactly two fields for frozen
  literal compatibility. Axum `Extension<RuntimeWiring>` is the optional
  compatibility seam; legacy test routers retain the existing static fallback.

## Implemented boundary

- `RuntimeWiring::for_daemon` composes lazily with an empty provider registry,
  the live `SessionService`, one tool snapshot and a deny-by-default engine
  policy. Construction reads no credential and starts no task.
- `RuntimeWiring::try_acquire_turn` returns one owned permit from the existing
  bounded runtime pool.
- Both HTTP turn routes prefer the runtime extension for admission and the
  runtime's shared session service. Streaming retains the owned permit in
  `TurnStreamState`; response-stream drop therefore reclaims capacity.
- `crates/cli/src/main.rs::serve` acquires one process-wide `EngineLease`,
  builds the runtime once, maps the explicit `OPENCODE_RK_TURN_TOOLS` list into
  a deny-by-default policy, and layers that runtime onto the authenticated
  production router. The lease lives until `axum::serve` returns.
- No authentication behavior changed. Existing request-local
  `PermissionBroker` still gates every tool side effect.

## RED / GREEN evidence

All commands were serial with one Cargo job and one test thread where relevant.

- `cargo test -p opencode-rk-server --test runtime_wiring_http -- --test-threads=1`
  - RED before implementation: 0/1, expected status mismatch `400 != 429`.
  - GREEN after implementation: 1/1; shared pool remained exhausted, no
    session message was written, all held permits returned on Drop.
- `cargo test -p opencode-rk-server --test session_turn_stream_api -- --test-threads=1`:
  2/2 GREEN.
- `cargo test -p opencode-rk-server --test session_turn_activity_api -- --test-threads=1`:
  2/2 GREEN.
- `cargo test -p opencode-rk-server runtime_wiring --lib -- --test-threads=1`:
  6/6 GREEN.
- `cargo build -p opencode-rk-cli --bin opencode-rk`: exit 0.
- Canonical Cargo integration run with the genuine pinned arm64 OpenTUI dylib:
  `cargo test -p opencode-rk-cli --test web_startup_readiness -- --test-threads=1`:
  1/1 GREEN in 3.04s; frozen SHA-256 remains
  `7fad25153f8f08b554541b34c78cf237912e40ee89b97b3c5a2723a57180320a`.
- Workspace `cargo fmt --all -- --check` is blocked by broad pre-existing
  formatting drift and a missing `crates/tools/src/mcp_config.rs`; no unrelated
  files were reformatted. Owned code compiles in all commands above.

## Resource and lifetime bounds

- No queue or retained output was added. Permit count remains the existing
  runtime cap (two in the frozen journey).
- No task/process/thread is spawned by runtime construction. The pre-existing
  daemon accept task remains owned and joined by `serve`.
- Permit lifetime is request-local for non-streaming turns and stream-local for
  streaming turns. Drop handles parse failure, provider failure and disconnect.
- Fixtures used in-memory storage and temporary directories only; no user
  database, credentials or network were accessed by the frozen RED/GREEN.

## Remaining mandatory gaps

- Runtime policy/tool snapshots are composed by the daemon but the stream's
  execution branch still builds a request-local `ToolRegistry` and consults the
  runtime policy only indirectly through the same environment allowlist.
- Runtime event publication and cooperative per-turn cancellation are not yet
  connected to this HTTP stream. `CancelState` is process-wide and must not be
  misused as a per-turn cancellation token.
- Native durable tool-call/reference persistence and citations remain absent as
  documented in `tasks/WEB-009.md`; WEB-009 therefore cannot be marked complete.
- The macOS dylib used for verification is a disposable, untracked fixture.
  TUI-011 packaging/provenance authority is still required before release.
- Repository/convergence gates remain blocked by pre-existing ledger/backlog
  reconciliation findings. This candidate is integration evidence, not release
  acceptance.
