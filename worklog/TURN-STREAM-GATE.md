# TURN-STREAM-GATE — session_turn_stream_api flaky gate

Commit: `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`

## 1. Lock audit (confirmed)

- `crates/server/tests/session_turn_stream_api.rs`: NO `env_lock`, NO `OnceLock`, NO `Mutex`
  (grep `env_lock|OnceLock|Mutex` → `NO_MATCH_STREAM`).
  - Per-test env mutation via `EnvGuard::set`:
    - `session_turn_stream_api.rs:287-288` (`OPENAI_BASE_URL` + `OPENAI_API_KEY`, test 1)
    - `session_turn_stream_api.rs:366-367` (same keys, test 2)
  - Tests: `session_turn_stream_api.rs:284-285`, `:363-364` (`#[tokio::test]`, same binary → same process → shared env).
- `crates/server/tests/session_turn_activity_api.rs` HAS the pattern:
  - `session_turn_activity_api.rs:5` (`sync::{Arc, Mutex, OnceLock}`)
  - `session_turn_activity_api.rs:21-24` (`fn env_lock() -> &'static Mutex<()>`)
  - `session_turn_activity_api.rs:179-181` (`let _env_guard = env_lock().lock()...`, test 1)
  - `session_turn_activity_api.rs:253-255` (same, test 2)

## 2. Root cause

`OpenAiResponsesClient::from_env()` (`crates/providers/src/responses.rs:106-125`)
reads `OPENAI_BASE_URL`/`OPENAI_API_KEY` from process-global env **per request**
(via `ProviderConfig::from_env("openai")`, `responses.rs:107`).
The 2 stream tests set **different** fixture base URLs (ephemeral `127.0.0.1:0`
ports, `session_turn_stream_api.rs:108-154` vs `:156-172`) through unlocked
`EnvGuard`. Under default parallel `tokio::test` threads, test A can read test B's
URL/key (or a half-restored value during `Drop`), dial the wrong fixture port,
and fail/time out nondeterministically. Pure harness race — no server bug.

## 3. Why impl untouched

- Mapping already correct in `crates/server/src/lib.rs`:
  - Pre-stream failure → HTTP error: `create_turn_stream` calls
    `from_env().map_err(provider_failure)?` (`lib.rs:790`) and
    `provider.stream(...).await.map_err(provider_failure)?` (`lib.rs:813-816`)
    before the `201` response is built (`lib.rs:932-940`).
  - Mid-stream failure → `201` + error part: `Err(error)` arm in the unfold
    (`lib.rs:914-924`) emits `stream_error(failure.code, failure.message)`
    (`lib.rs:949-955`) with status already fixed at `201`.
- Frozen tests assert exactly this:
  - success: `HTTP/1.1 201` + 4 events (`session_turn_stream_api.rs:302-331`);
  - provider `response.failed`: `HTTP/1.1 201` + `user_message` + `error/bad_gateway`
    + 1 persisted message (`session_turn_stream_api.rs:379-403`).
  - No contradiction found; changing impl would break frozen contract.
- `TURN_PERMITS.try_acquire()` (`lib.rs:673-675`, `764-766`) semantics frozen:
  prior lane established blocking-acquire risks deadlock (permit held across
  `.await` stream body); `try_acquire` + `429` is the contract. Not touched.

## 4. Serial GREEN evidence (3x, `--test-threads=1`, serial only)

Command (each run, one at a time, `free -h` checked before runs, ~3 GiB avail):
`CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-server --test session_turn_stream_api -- --test-threads=1`

- `/tmp/opencode/stream_serial1/serial.log` (copy of `/tmp/opencode/stream_serial1.log`):
  `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` (0.05s)
- `/tmp/opencode/stream_serial2/serial.log`: same `2 passed; 0 failed` (0.07s)
- `/tmp/opencode/stream_serial3/serial.log`: same `2 passed; 0 failed` (0.05s)

Each log shows both tests ok:
`session_turn_stream_forwards_deltas_before_completion_and_persists_once ... ok`
`session_turn_stream_reports_provider_failure_without_persisting_assistant ... ok`

## 5. Mandated serial invocation (verifier/CI)

Until the harness fix lands, run this binary serially — default parallel threads
reintroduce the env race:

```
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-server --test session_turn_stream_api -- --test-threads=1
```

Do NOT run with default `--test-threads=N>1`, and do NOT parallelize with other
heavy jobs (8 GiB budget; one validation command at a time).

## 6. Proposed harness fix (NOT applied — verifier/test-owner authority)

Rationale for location: frozen tests must not be edited by this lane. Adding a
cross-test `env_lock` is a test-only change owned by the verifier/test owner.

Suggested diff for `crates/server/tests/session_turn_stream_api.rs`
(mirrors `session_turn_activity_api.rs:21-24,179-181,253-255`):

```diff
 use std::{
     env,
     io::{Read, Write},
     net::{SocketAddr, TcpListener, TcpStream},
-    sync::{mpsc, Arc},
+    sync::{mpsc, Arc, Mutex, OnceLock},
     thread,
     time::Duration,
 };
@@
+fn env_lock() -> &'static Mutex<()> {
+    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
+    LOCK.get_or_init(|| Mutex::new(()))
+}
+
 struct EnvGuard {
@@
 async fn session_turn_stream_forwards_deltas_before_completion_and_persists_once() {
+    let _env_guard = env_lock()
+        .lock()
+        .unwrap_or_else(|poisoned| poisoned.into_inner());
     let (provider_base, release_provider, provider_request) = spawn_streaming_openai_fixture();
@@
 async fn session_turn_stream_reports_provider_failure_without_persisting_assistant() {
+    let _env_guard = env_lock()
+        .lock()
+        .unwrap_or_else(|poisoned| poisoned.into_inner());
     let (provider_base, provider_task) = spawn_failed_openai_fixture();
```

Note: per-binary `static LOCK` only serializes within this binary. A full fix
also needs either distinct env keys per test, `from_env` injection, or a
workspace-shared lock if other binaries mutate the same keys concurrently.
Minimal step above matches the activity-test precedent and removes the observed
2-test race. Verifier decides.
