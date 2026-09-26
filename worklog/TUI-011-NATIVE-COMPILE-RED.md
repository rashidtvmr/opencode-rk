# TUI-011-NATIVE-COMPILE-RED

## Claim
Task `TUI-011-NATIVE-COMPILE-RED` claimed in worktree ledger (status: in-progress, session: ses_f229377e4ffe4F6IJYFB5ct3Q1).

## Source evidence
- `crates/cli/src/tui_entry.rs:491` — `lines.push("Enter send ...")` where `lines` is `Vec<String>`.
  Error E0308: expected `String`, found `&str`. Fix would be `.to_string()`.
- `crates/opentui-bridge/build.rs:17-21` — links dylib from `native/lib/<triple>/libopentui.dylib` using `TARGET` env var.
- `crates/opentui-bridge/src/safe_renderer.rs:18` — `#[link(name = "opentui")]` FFI.
- Workspace root `Cargo.toml` includes `crates/cli` and `crates/opentui-bridge`.

## Observable contract
1. Stage source at SHA 7938791 into disposable snapshot via `git archive`.
2. Copy verified fixture dylib (sha256 5e0265d4...) to `crates/opentui-bridge/native/lib/aarch64-apple-darwin/libopentui.dylib`.
3. Run `cargo build --locked -p opencode-rk-cli --bin oc2 --features native` with fresh HOME/target.
4. Assert exit 0 AND binary exists.

## Observed scenario
- At SHA 7938791, the build FAILS with E0308 at tui_entry.rs:491.
- The genuine dylib is present and correctly located; build.rs succeeds through linking setup.
- The type mismatch in the `#[cfg(feature = "native")]` code path blocks compilation.

## Target boundary
- Test file only: `tests/e2e/native_compile.rs`.
- No product code changes (this is the RED lane).
- std-only, compiles with `rustc --test`.

## Tests
- `native_compile_succeeds_with_genuine_dylib` — one test, asserts exit 0 + binary exists.
- Compile: `rustc --test tests/e2e/native_compile.rs --edition 2021` — success.
- List: `/tmp/tui011_native_compile_test --list` shows 1 test.
- Run: test panics with E0308 at tui_entry.rs:491, exit code 101. RED confirmed.

## Resource bounds
- CARGO_BUILD_JOBS=1, RUST_TEST_THREADS=1
- BUILD_TIMEOUT = 300s with kill of owned Child only
- stdout/stderr capped at 32 KiB each
- Fresh temp HOME and CARGO_TARGET_DIR
- No DYLD_LIBRARY_PATH for build
- No user DB/secrets accessed

## Decisions
- Used runtime `std::env::var("CARGO_MANIFEST_DIR")` instead of compile-time `env!()` because `rustc --test` does not set `CARGO_MANIFEST_DIR`.
- Used `archive.stdout.take()` before spawning tar to avoid partial move borrow issue.
- Fixture path uses string continuation (`\`) for readability; verified against actual filesystem path.
- `NATIVE_COMPILE_REPO_ROOT` env var override supported; defaults to `CARGO_MANIFEST_DIR` runtime value.

## Remaining unknowns
- None. RED test compiles, runs, and fails for the intended reason (E0308 at line 491).
