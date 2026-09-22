# BASE-004 scratchpad

Claim: BASE-004 claimed by `ses_f3824491affeLCAK4H9PoqPaJC`; RED-only lane.
Ownership: `crates/server/tests/daemon_long_path.rs`, this scratchpad, own ledger row.

## Source evidence

- `crates/server/src/daemon.rs:136-145`, `DaemonPaths::for_data_dir`, currently appends `runtime/opencode-rk.sock`, `opencode-rk.pid`, and `backend.json` directly to the caller path.
- `crates/server/src/daemon.rs:341-352`, `SingletonDaemon::bind`, binds the supplied Unix socket path.
- `crates/server/tests/web_singleton_lock.rs:13-39`, existing disposable `DaemonPaths`/PID and descriptor coverage.
- `PLAN.md:69-76`, singleton isolation is per OS user and data directory.

## Contract

For a canonical, disposable, deeply nested data directory, socket derivation must be deterministic for the same directory, distinct for different directories, and remain below Darwin `SUN_LEN`'s 104-byte maximum (`sockaddr_un.sun_path` is 104 bytes including the NUL; use `< 104` as the conservative usable pathname bound). PID and backend descriptor paths remain beneath the caller data directory. The test uses no user state, credentials, network, symlinks, or implementation-specific short-root location.

## RED test

Added `daemon_socket_path_stays_below_darwin_sun_len_for_long_data_dirs`. Expected current failure: derived socket pathname exceeds the 104-byte Darwin bound while PID/descriptor ownership and per-data-dir isolation assertions remain meaningful.

## Verification

Command: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test daemon_long_path -- --test-threads=1`

Result: compiling RED; one test failed at `crates/server/tests/daemon_long_path.rs:32` because current derived socket path measured 225 bytes (`>= 104`). SHA-256: `a3402c6ddd2cf4d14823511e1845cedc37bda1a2bd728ae975633673c904101d`.

Status must remain `blocked` with note `frozen RED awaiting implementation`.
