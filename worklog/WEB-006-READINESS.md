# WEB-006-READINESS scratchpad

Claim: WEB-006 reclaimed by `ses_f3c4de578ffelQv59xDXmOs03B`; claimed by `ses_f38060eecffel8uIM3uo6rif9k`.
Ownership: `crates/cli/tests/web_startup_readiness.rs`, this scratchpad, WEB-006 ledger row.

Source evidence (HEAD `b550530`):
- `crates/cli/src/main.rs:658-675`, `serve` AlreadyRunning branch performs one `read_backend_descriptor` and errors when absent.
- `crates/server/src/daemon.rs:63-87`, `PidLock::acquire` is the real advisory singleton lock.
- `crates/server/src/daemon.rs:538-559`, authenticated descriptor publication.

Contract: hold real PID lock; delay valid 64-hex descriptor publication 250ms; provide loopback `/health`; second packaged `opencode-rk serve --listen 127.0.0.1:0` remains alive during delay, exits 0 within 3s, prints exact origin, creates no second listener, joins fixture threads.

RED test: added `crates/cli/tests/web_startup_readiness.rs`. No product edits.

Verification: focused command `rtk sh -c 'CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli --test web_startup_readiness -- --test-threads=1'` blocked before test compilation by pre-existing `opencode-rk-opentui-bridge` custom build failure. Expected runtime RED is `backend is already running but its endpoint descriptor is unavailable` before publication. Test SHA-256: `069866974500d94ce027c3f5e679c59aefe6db20c899c03278970ab6b602cbb7`.
