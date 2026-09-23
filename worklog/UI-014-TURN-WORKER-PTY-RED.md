# UI-014 turn-worker PTY RED

## Claim and boundary

- Task: `UI-014`
- Session: `ses_f308d04f2ffeyW7O602eMLxyFe`
- Branch: `lane/UI-014-turn-worker`
- Candidate base: `73298773714f9a2d48507fe9bfdf9c24960d7c42`
- Owned paths only: `crates/cli/tests/ui014_turn_worker_pty.rs`, this scratchpad, UI-014 ledger row.
- No production/shared source edits.

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

UI-014 remains blocked. The real executable reached the delayed provider POST and failed specifically on missing asynchronous input ownership/queue/interrupt behavior. No production fix or test weakening performed.
