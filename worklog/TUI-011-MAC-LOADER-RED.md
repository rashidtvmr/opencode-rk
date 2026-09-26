# TUI-011-MAC-LOADER-RED

## Claim
Task `TUI-011-MAC-LOADER-RED` claimed in ledger (status: in-progress, session: ses_f226b7a89ffefS159kISJ5bllJ). Scratchpad path: worklog/TUI-011-MAC-LOADER-RED.md

## Source evidence
- `crates/cli/Cargo.toml:9-18` (repo commit 55a4292): `[[bin]] oc2`; CLI crate has NO build.rs.
- `crates/opentui-bridge/build.rs:11-21`: emits only `cargo:rustc-link-search=native=<libdir>`; NO `cargo:rustc-link-arg=-rpath` / no LC_RPATH.
- `crates/opentui-bridge/Cargo.toml:14`: `crate-type = ["rlib"]`; build-script link attrs not transitive to binary.
- Prebuilt binary env: `OC2_NATIVE_BINARY=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/impl-tui011-native-compile/target/debug/oc2` (Mach-O arm64, 45447208 bytes).
- Genuine dylib env: `MAC_OPENTUI_FIXTURE=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/mac-opentui-preserved-4954312/packages/native/lib/aarch64-macos/libopentui.dylib` (Mach-O dylib arm64, sha256 5e0265d4b053004fe78974562561e10e46567cca96697f100e3288653163330e, install name @rpath/libopentui.dylib).

## Observable contract
1. Require both prerequisite paths from env (OC2_NATIVE_BINARY, MAC_OPENTUI_FIXTURE); fail-fast (not RED) if either is missing.
2. Stage owned disposable copies into a fresh temp dir: `bin/oc2` and `lib/libopentui.dylib` (installed layout; sibling lib). No symlinks to user fixtures; no user HOME/DB/secrets.
3. Spawn `bin/oc2 --version` with env_clear + explicit HOME/PATH/TERM only; NO DYLD_LIBRARY_PATH or DYLD_FALLBACK_LIBRARY_PATH.
4. Poll try_wait <= 5s; kill+wait only owned child on timeout. stdout/stderr Stdio::null (no pipe/deadlock; status suffices).
5. Assert status.success() (expected FALSE today, exit 134 from dyld no LC_RPATH's).

## Observed scenario (RED)
- `otool -l <oc2> | grep -c LC_RPATH` = 0.
- `env -u DYLD_LIBRARY_PATH -u DYLD_FALLBACK_LIBRARY_PATH bin/oc2 --version` exits 134: `dyld[...]: Library not loaded: @rpath/libopentui.dylib ... Reason: no LC_RPATH's found`.
- With DYLD_FALLBACK_LIBRARY_PATH set to lib dir, `oc2 0.1.0-alpha.1` exit 0 -> fixture is genuine.

## Target boundary
- Owned files only: tests/e2e/macos_loader.rs (test) + worklog/TUI-011-MAC-LOADER-RED.md (scratchpad) + tasks/completion/claims.json (ledger row).
- No product edits; no frozen test edits; no Cargo build; RED-only lane, ledger BLOCKED (never completed).

## Test
- `installed_layout_loader_finds_dylib_without_dyld_env` — asserts status.success (FALSE today).

## Commands/results
- Compile: `rustc --test tests/e2e/macos_loader.rs --edition 2021 -o /tmp/tui011_macos_loader_test` -> exit 0 (clean, no warnings).
- List: `/tmp/tui011_macos_loader_test --list` -> `installed_layout_loader_finds_dylib_without_dyld_env: test` (1 test, 0 benchmarks), exit 0.
- Run: `/tmp/tui011_macos_loader_test` -> FAILED, exit 101. Panic: `installed-layout oc2 must load sibling lib/libopentui.dylib without DYLD_* env; oc2 failed (dyld no LC_RPATH's found expected at SHA 55a4292); exit_code=-1 signal=6 (134 = 128+SIGABRT)`.
- Freeze hash (tests/e2e/macos_loader.rs): c0a809a28800b542613bfb79b0d9566c07c955e45cb9afe441c0a697ae99ef36

## Resource bounds
- 5s child timeout; kill owned child only; Stdio::null for streams (no cap needed); fresh temp dir cleaned on drop; no user data touched.

## Decisions
- Env override for fixture/binary paths (OC2_NATIVE_BINARY, MAC_OPENTUI_FIXTURE) — defaults to task-pinned absolute paths.
- env_clear then re-add HOME/PATH/TERM only.
- No output capture: dyld error already captured by prior diagnostic; status code is sufficient assertion surface.

## Remaining unknowns
- None. RED test compiles, lists 1, fails on exit134.
