# WEB-006-READINESS scratchpad

Claim: WEB-006 reclaimed by `ses_f3c4de578ffelQv59xDXmOs03B`; claimed by `ses_f38060eecffel8uIM3uo6rif9k`.
Ownership: `crates/cli/tests/web_startup_readiness.rs`, this scratchpad, WEB-006 ledger row.

Source evidence (HEAD `b550530`):
- `crates/cli/src/main.rs:658-675`, `serve` AlreadyRunning branch performs one `read_backend_descriptor` and errors when absent.
- `crates/server/src/daemon.rs:63-87`, `PidLock::acquire` is the real advisory singleton lock.
- `crates/server/src/daemon.rs:538-559`, authenticated descriptor publication.

Contract: hold real PID lock; delay valid 64-hex descriptor publication 250ms; provide loopback `/health`; second packaged `opencode-rk serve --listen 127.0.0.1:0` remains alive during delay, exits 0 within 3s, prints exact origin, creates no second listener, joins fixture threads.

RED test: added `crates/cli/tests/web_startup_readiness.rs`. No product edits.

Verification: dev binary build passed: `CARGO_BUILD_JOBS=1 cargo build -p opencode-rk-cli --bin opencode-rk`. Disposable harness compiled owned test. Runtime RED command used built test executable with `OPENCODE_RK_TEST_BIN`; failing assertion: `second caller exited during publication window` at test line 145, confirming one-shot `AlreadyRunning` descriptor read. Fixture RAII joins publisher/health threads on unwind. Test SHA-256: `7fad25153f8f08b554541b34c78cf237912e40ee89b97b3c5a2723a57180320`.
