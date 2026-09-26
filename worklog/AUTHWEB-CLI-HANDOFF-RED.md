# AUTHWEB-CLI-HANDOFF-RED

## Role
Independent TEST-AUTHOR for AUTHWEB-CLI-HANDOFF-RED

## Source evidence
- `crates/server/src/daemon.rs:376-416` `read_backend_descriptor`: reads `<data-dir>/runtime/backend.json` with schema_version 1, live pid, loopback origin, 64-hex auth_token. Returns `BackendDescriptor{pid, http_origin, schema_version, auth_token}`.
- `crates/server/src/daemon.rs:117-127` `BackendDescriptor` struct: fields `pid`, `http_origin`, `schema_version`, `auth_token` (64 hex chars per daemon_auth.rs:32-34).
- `crates/cli/src/main.rs:668-686` `web()` function: reads descriptor, prints `descriptor.http_origin`, calls `open_web_browser(&descriptor.http_origin)`.
- `crates/cli/src/main.rs:761-789` `open_web_browser(origin)`: spawns `open`/`xdg-open`/`cmd /C start` with `origin` as the sole arg. Does NOT append `#oc2-token=<token>` fragment.
- `crates/cli/src/main.rs:793-803` `resolve_data_dir`: honors `--data-dir` (via `cli.data_dir`) and `HOME`/`USERPROFILE`; the clap `env="OPENCODE_RK_HOME"` on line 66 handles env override, so the task prompt note about ignored env is stale.
- `crates/cli/src/main.rs:284-286`: `Command::Web(args)` calls `resolve_data_dir(cli.data_dir)` then `web(data, args)`.

## Gap / RED
The bearer credential (`auth_token`, 64-hex) is published in `backend.json` but is NEVER delivered to the browser-side client. `open_web_browser` passes only the bare `http_origin` (e.g. `http://127.0.0.1:4096`). The expected handoff URL fragment `#oc2-token=<64hex>` is missing from the URL argv passed to `open`/`xdg-open`.

## Observable contract
- The test writes a `backend.json` in a disposable data dir with: schema_version=1, a live pid (the test process itself), a valid loopback origin `http://127.0.0.1:<port>`, and a fake 64-hex auth_token.
- `oc2 --data-dir <dir> web` (NOT `--no-open`, NOT `serve`) reuses the existing descriptor and calls `open_web_browser` with the bare origin.
- A fake `open` (macOS) or `xdg-open` (Linux) script in a prepend-to-PATH dir captures its argv to a record file.
- The test asserts: (1) the probe was invoked, (2) the captured URL argv equals origin + exactly `#oc2-token=<64hex>`, (3) no token in stdout/stderr, (4) no token as query param, (5) child reaped within deadline.

## Fixture constraints (std-only, no cargo build)
- Compiles with `rustc --test --emit=metadata` using only `std`. No serde_json.
- Minimal hand-rolled JSON parser for reading `backend.json`.
- Uses `OC2_BIN` env var (must be set to a pre-built binary path). If absent, test compiles but execution panics with clear message.
- Fake `open`/`xdg-open` is a shell script (POSIX sh).
- Temp dir is disposable; all child processes are killed+waited on drop.

## Decisions
- Replace the entire `tests/e2e/browser_credential_launch.rs` file (draft was unsafe/uncompiled).
- Single behavioral RED test: `open_web_browser` must deliver `#oc2-token=<token>` as a URL fragment to the browser probe.
- RED fails because current code passes bare origin (no fragment).

## Corrections applied before freeze (per task instructions)
- TempDir::new: changed `fs::create_dir_all` to `fs::create_dir` so a name collision fails rather than silently deleting another dir's data; Drop still removes via `remove_dir_all`.
- PATH: changed from `:{home}` (empty first entry) to `{home}:/usr/bin:/bin` (owned probe dir first, then standard system dirs).
- Added bounded stderr read + assertion: `assert!(!stderr.contains(token), "token leaked to stderr")`.
- Kept child timeout/kill/reap (5s deadline, kill+wait) and no live daemon spawned.

## RED verification (genuine, executed)
- Compilation: `rustc --edition=2021 --test --emit=metadata` -> exit 0 (std-only).
- Full compile: `rustc --edition=2021 --test -o /tmp/oc2_red_test` -> exit 0.
- Execution: `OC2_BIN=/private/var/folders/b0/.../target/debug/oc2 /tmp/oc2_red_test --test-threads=1`
- Result: FAILED. Captured URL = `http://127.0.0.1:<port>` (bare origin, NO #oc2-token fragment).
  - Assertion: left=`http://127.0.0.1:53834` != right=`http://127.0.0.1:53834#oc2-token=<token>`
  - Token did NOT leak to stdout or stderr (security assertions pass).
- Binary: `target/debug/oc2` is a real Mach-O 64-bit arm64 executable (43.3 MiB), not guessed.

## Frozen hash
- Test file SHA256: `3c8eea31b80924c4f3f0cb912c2ae03234bd8077a9f32bc55d533e23bd4c9767`
- This hash is frozen; the test will NOT be edited again.

## Ledger status
- RED-only task: status set to `blocked` (genuine RED confirmed, implementation not in scope for this lane).
- Committed to branch `red/AUTHWEB-CLI-HANDOFF` (no main merge, no source edits, no secrets).
