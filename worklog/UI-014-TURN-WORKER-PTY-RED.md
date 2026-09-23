# UI-014 turn-worker PTY RED

## Claim and boundary

- Task: `UI-014`
- Session: `ses_f308d04f2ffeyW7O602eMLxyFe`
- Branch: `lane/UI-014-turn-worker`
- Candidate base: `5b18b8fd0c680055568ce7c1ce6435475fcf48e2`
- Owned paths only: `crates/cli/src/turn_worker.rs`, this scratchpad, UI-014 ledger row.
- Integrator prewire remains in `crates/cli/src/main.rs`; no edits made here.

## Source evidence

- `crates/cli/src/tui_entry.rs:862-920`: compatibility PTY loop reads a line, calls synchronous `execute_submit` at `:894-903`, then drains queued submissions only after completion.
- `crates/cli/src/tui_entry.rs:870-871`: input iterator owns the next line; no concurrent input owner while `execute_submit` runs.
- `crates/cli/src/chat.rs:402-445`: default turn path synchronously performs `POST /api/sessions/{id}/turns`.
- `crates/server/src/lib.rs:902-987`: authenticated turn endpoint executes provider request before returning.
- `crates/opentui-bridge/build.rs:50-63`: native artifact gate is independent of this test.

## Frozen test

- File: `crates/cli/tests/ui014_turn_worker_pty.rs`
- SHA-256: `813e12181951bdb8967eba8b2b1c9dd1b97672e43a7b26e0b57f20475903f214`
- Real binary selection: `OC2_E2E_BIN`, fallback compile-time `CARGO_BIN_EXE_oc2`.
- PTY: macOS `/usr/bin/script`, explicit argv; no shell command string.
- Fixture: disposable HOME/data, `env_clear`, bounded explicit environment, fake credential, loopback provider, authenticated local daemon, bounded HTTP body/capture, fixed timeouts, child cleanup.
- Scenario: first POST held; second prompt, `:i`, `:q` sent while unresolved; ambiguous first close; expect queued input processing and exactly one provider request.

## Build/run evidence

Standalone non-native binary build (Cargo integration target intentionally not used):

```text
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo build -p opencode-rk-cli --bin oc2 --no-default-features
```

Exact standalone test compile:

```text
CARGO_BIN_EXE_oc2="$PWD/target/debug/oc2" OC2_E2E_BIN="$PWD/target/debug/oc2" rustc --edition=2021 --test crates/cli/tests/ui014_turn_worker_pty.rs -L dependency=target/debug/deps --extern serde_json=target/debug/deps/libserde_json-24416be3666bbe41.rlib -o target/ui014_turn_worker_pty
```

Bounded run: Python `subprocess.Popen(...).communicate(timeout=60)` against `target/ui014_turn_worker_pty --nocapture --test-threads=1`, `OC2_E2E_BIN=target/debug/oc2`.

- Exit: `101` (intentional RED).
- Result: `0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out`; runtime `0.13s`.
- Failure: `UI-014 RED: real oc2 caller did not process queued input/interrupt before delayed POST completed`.
- Evidence: `before=1`, final records `ordinal=1` and `ordinal=2`, `max_active=1`, `closed_without_response=2`.
- First request body: 123 bytes; second request body: 165 bytes. Raw bodies were not retained/logged.
- PTY showed `second-prompt`, `:i`, `:q` echoed while the first request was held, but no queued/interrupted processing marker. After release, the caller processed `you: second-prompt`, issued a second POST, then printed `[interrupted; draft preserved: ""]`.
- No matching `ui014_turn_worker_pty`, `oc2`, or `/usr/bin/script` processes remained after the run.
- `git diff --check`: pass.

## Bounds and limits

- Provider request body bound: 128 KiB.
- PTY/daemon capture bound: 64 KiB; diagnostic output bound: 4096 redacted characters.
- Provider handler bound: 8; first handler wait/release bound: 8s; run bound: 60s (runner cap 180s maximum).
- Build/test concurrency: one (`CARGO_BUILD_JOBS=1`, `RUST_TEST_THREADS=1` where Cargo was used).
- Overflow behavior intentionally omitted: not observable through this real binary journey without adding a separate frozen scenario.
- Native OpenTUI path not exercised on this host. Cargo integration target was not used. Missing `aarch64-apple-darwin` `libopentui` is recorded as the separately active TUI-011 artifact lane, not as the UI-014 behavioral RED blocker.

## Result

## Worker implementation

- `crates/cli/src/turn_worker.rs`: one Tokio task; `COMMAND_CHANNEL_CAPACITY = 1` control mailbox; no prompt queue; `RESULT_CHANNEL_CAPACITY = 4` caller-owned result mailbox.
- `TurnRequest::new`/`validate`: loopback URL, non-empty bounded fields, 32 KiB prompt, 128 KiB serialized request, 8 KiB bearer. Bearer omitted from `Debug` and failures.
- `TurnWorkerHandle::try_submit`: CAS reservation before bounded `try_reserve`; `Busy`/`Full`/`Closed` return the original request for caller ownership.
- `DispatchPhase`: accepted-to-dispatched CAS closes the race. Interrupt before dispatch returns `Cancelled`; after dispatch returns `Uncertain`, with no replay path.
- `send_request`: bounded streamed response (1 MiB), bounded assistant output (1 MiB), no raw response/error retention; transport/timeout/malformed/oversize outcomes after dispatch are `Uncertain`.
- `TurnWorker::shutdown(self).await`: sends cancellation and shutdown through the control channel, awaits the sole `JoinHandle`; `Drop` only aborts best effort and claims no join.

## Verification after implementation

- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo check -p opencode-rk-cli --bin oc2 --no-default-features` -> PASS, `Finished dev profile`; existing warnings only. This exercised the integrator's `mod turn_worker;` prewire.
- `rustfmt --edition 2021 crates/cli/src/turn_worker.rs` -> PASS.
- `git diff --check` -> PASS.
- `cargo test -p opencode-rk-cli --bin oc2 turn_worker --no-default-features -- --test-threads=1` -> BLOCKED before test compile by `crates/opentui-bridge/build.rs:58-62`: missing `native/lib/aarch64-apple-darwin/libopentui.a`/`.dylib`; same TUI-011 artifact blocker. No test edits.
- Strict scoped clippy attempted: `cargo clippy -p opencode-rk-cli --bin oc2 --no-default-features -- -D warnings` -> BLOCKED by pre-existing warnings promoted to errors in security/providers/tools/sessions/server; no owned-file diagnostic.

Final caller lane updates `crates/cli/src/tui_entry.rs`: async `run`/`run_with_dir`, compatibility and native worker ownership, bounded concurrent input/result handling, composer-owned FIFO queue, cancellation classification, explicit worker shutdown before renderer restoration.
- Latest no-default verification: `rustfmt --edition 2021 crates/cli/src/tui_entry.rs`, `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo check -p opencode-rk-cli --bin oc2 --no-default-features`, and `cargo build -p opencode-rk-cli --bin oc2 --no-default-features` pass with existing warnings.
- Standalone frozen PTY rebuilt and rerun: `1 passed; 0 failed`; delayed first POST accepted second input, processed interrupt/exit, observed one provider request, no replay, cleanup completed.
- Native artifact now present at `crates/opentui-bridge/native/lib/aarch64-apple-darwin/libopentui.dylib`; scoped `DYLD_LIBRARY_PATH=... cargo check -p opencode-rk-cli --bin oc2 --features native` passes. Native filtered test command passes build and runs `0 tests` for the filter.
- UI-014 remains blocked for orchestrator integration/native acceptance. Changes are uncommitted; no test edits.
- Baseline repair: `tui_entry::run_with_dir` now short-circuits an origin-less `--once` invocation to the bounded local frame before descriptor resolution or any `/api` request. Explicit `--origin` remains on the validated descriptor/live snapshot path.
- Native parity verification with `--no-default-features`: `cargo test -p opencode-rk-cli --test native_tui_parity --no-default-features -- --test-threads=1` -> `13 passed; 0 failed`; p11 dead-origin and p12 semantics pass.
- Native source verification: scoped `DYLD_LIBRARY_PATH=crates/opentui-bridge/native/lib/aarch64-apple-darwin cargo check -p opencode-rk-cli --bin oc2 --features native` and native build pass. Native parity child launch remains blocked by the frozen harness `env_clear()` removing DYLD search paths while the binary has `@rpath/libopentui.dylib` with no LC_RPATH; this is loader/environment setup, not a tui_entry source failure.
## 2026-09-23 static native renderer repair

- Exact integrated base: `a5339df7a098fb1e4ab2c0bfd06e6c4d76542fb8`.
- Frozen `crates/cli/tests/native_tui_parity.rs` SHA-256 remained
  `81d12f9a9e149b713fe6891fd56f081d528ff944e5d7f25d4cf2728a62abe4c3`.
- Real static OpenTUI execution compiled and ran, then RED 8/13: untouched
  blank cells serialized as U+0A00 and the fixed 24-row snapshot clipped the
  bounded 64-entry memory pane to 20 visible entries.
- Minimal production repair normalizes the pinned backend's U+0A00 blank-cell
  sentinel in `safe_renderer.rs` and sizes scriptable one-shot snapshots to all
  already-bounded frame lines in `tui_entry.rs`.
- Disposable static snapshot GREEN: native parity 13/13 with no test edits.
- UI-014 remains blocked until the static artifact/build lane is integrated and
  the PTY journey is rerun on the exact integrated revision.

## 2026-09-23 real static PTY GREEN

- Frozen PTY test was not edited. Native startup now renders the exact
  `OpenCode RK TUI` marker through the real statically linked OpenTUI backend.
- OpenTUI stdout writes are synchronous, so byte-rate full-frame repainting
  blocked PTY input. Production rendering now coalesces ordinary paints behind
  a bounded 100 ms cadence while queue/interrupt state receives an immediate
  paint before exit.
- Unix input no longer leaves an uncancellable `spawn_blocking` stdin reader at
  Tokio shutdown. A bounded async channel is fed by `AsyncFd` after setting
  `O_NONBLOCK`; task abort is awaited and original descriptor flags are restored.
- Exact command: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli --test ui014_turn_worker_pty --features native -- --test-threads=1`.
  Result: 1/1 passed in 0.32 s; one provider POST, second prompt consumed while
  request one remained unresolved, interrupt and quit handled, no replay,
  terminal modes restored, process exited within the frozen bound.
- Candidate GREEN only. UI-014 stays blocked until the same frozen test passes
  on the exact product-spine integration revision.

## 2026-09-23 integrated and packaged reruns

- Exact product-spine revision
  `2568eec4eeebf630cddfa15cf4ca196078cb5237`: frozen UI-014 PTY 1/1 GREEN.
- Disposable installed release executable was selected with `OC2_E2E_BIN` and
  the same frozen test passed 1/1. The binary hash was
  `cfae8ac37218c98e2a0ead83eb77eeb47753bce4017a94847d99b660aa205b25`.
- Separate installed-binary proof confirmed exiting a native one-shot client
  left the shared authenticated daemon alive and healthy.
- UI-014 remains blocked rather than self-accepted because the frozen parent
  default-entrypoint suite is 1/6 (five immutable `todo!()` failures) and bare
  fresh-HOME `oc2` still does not enter in-app setup or create its first session.
