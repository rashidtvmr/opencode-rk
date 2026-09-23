# WEB-009 durable persistence RED

## Claim

- Task: WEB-009
- Session: `ses_f31e58165ffeQHXSGRZFyO9hWF`
- Branch: `lane/WEB-009-durable-red-20260923`
- Owned test: `crates/server/tests/web009_durable_persistence_red.rs`

## Source evidence

- `crates/server/src/lib.rs:1290-1459`: web stream emits reasoning deltas,
  tool calls, tool output and final assistant message; final payload has no
  references field.
- `crates/server/src/lib.rs:1611-1655`: tool output persists only as a plain
  `MessageRole::Tool` text row; no call identity/state metadata is persisted.
- `crates/server/src/lib.rs:617-629`: activity API returns only
  `message_id` and `reasoning_summary`.
- `crates/providers/src/responses.rs:416-430`: annotation/citation event is
  unsupported and fails closed.
- `crates/server/src/daemon_auth.rs:150-175`: authenticated `/api/*` routes
  require `Authorization: Bearer <token>`.

## Contract

Real authenticated provider fixture emits reasoning, a bash function call,
tool output, final answer and URL citation. The turn must expose references,
persist tool identity/state and references, then reproduce all fields after
reopening the same disposable SQLite database. No hidden chain-of-thought.

## RED test

`web009_t05_tool_and_reference_fidelity_survives_restart` uses:

- disposable `TempDir` database/blob root;
- minted daemon bearer token;
- bounded local TCP SSE provider fixture;
- real `router_with_auth`, `SessionService`, `Storage`, provider adapter;
- restart by dropping/reopening app against the same fixture database.

Expected current failure: citation event is unsupported, so no final assistant
event/references or durable tool/reference projection exists.

## Verification

- Command: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120 cargo test -p opencode-rk-server --test web009_durable_persistence_red -- --test-threads=1`
- Result: compiling RED. The test fails at the missing final assistant event;
  provider stream emits `unsupported provider stream event:
  response.output_text.annotation.added`.
- Actual bounded command used because macOS lacks `timeout`: Python
  `subprocess.run(..., timeout=120)` with `CARGO_BUILD_JOBS=1`
  `RUST_TEST_THREADS=1`.
- Test SHA-256: `c0acaf32cbf235452499acbe3eb5f617b7e319f8e6043a7b3cd54720a5b81bb8`.

## Remaining boundary

Implementation must add a native citation/reference producer, durable per-answer
tool/reference schema/API, and reload projection. This lane does not implement
product code.
