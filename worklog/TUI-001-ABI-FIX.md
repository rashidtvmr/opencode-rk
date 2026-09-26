# TUI-001 ABI fix (null fg SIGSEGV)

Status: implementation GREEN on all required targets; CLI focused target records pre-existing environment failure; no test edits.

## Claim

- Claimed TUI-001 via `tools/completion_claims.py` as `ses_f37bb976cffebTIxeEDiL1hTCc`, scratchpad `worklog/TUI-001-ABI-FIX.md`.
- Prior row: `not-started`, reclaimedBy `ses_f3c4de578ffelQv59xDXmOs03B`. Claim succeeded, no collision.
- Owned file: `crates/opentui-bridge/src/safe_renderer.rs` only.

## Source evidence (exact)

- Worktree HEAD at start: `ac0b741e666d9661a53e55cfc5b91091da455fa6`, branch `lane/WEB-006-integration`.
- Pinned source commit: `c01292fd0837bafd07ce458c74416b2b375a41ab` (verified `git rev-parse HEAD` in pinned checkout).
- Pinned Zig `packages/native/src/lib.zig`:
  - `ptrToRGBA` lines 107-109: `fn ptrToRGBA(color: [*]const u16) RGBA` reads `color[0..3]` unconditionally.
  - `optionalPtrToRGBA` lines 111-117: null-tolerant, returns null.
  - `bufferDrawText` lines 1800-1810: `fg: [*]const u16` (non-nullable), `bg: ?[*]const u16` (nullable); body calls `ptrToRGBA(fg)` then `optionalPtrToRGBA(bg)`.
- Pinned core `packages/core/src/lib/RGBA.ts`:
  - `DEFAULT_FOREGROUND_RGB = [255, 255, 255]` line 5.
  - `defaultForeground` lines 125-129: `RGBA.fromInts(...DEFAULT_FOREGROUND_RGB)` gives alpha 255, packed with `packMeta(INTENT_DEFAULT)` (`INTENT_DEFAULT = 2`, lines 10/37).
- Pre-fix Rust `crates/opentui-bridge/src/safe_renderer.rs` lines 417-438: `draw_text` native branch passed `std::ptr::null()` for both fg and bg, with false comment at lines 419-420 claiming null selects native defaults.
- Rust color helpers reused, no color.rs edit: `crate::color::pack_rgba8`, `pack_meta`, `INTENT_DEFAULT` (`crates/opentui-bridge/src/color.rs` lines 13-15, 28-30, 35-42).
- Exports verified earlier per `worklog/TUI-001-ABI-REVERIFY.md` (`_bufferDrawText` etc. via `nm -gU`).

## Fix

- In `draw_text` native branch only (`safe_renderer.rs` circa lines 417-452 post-edit):
  - Build local `let fg = crate::color::pack_rgba8(255, 255, 255, 255, crate::color::pack_meta(crate::color::INTENT_DEFAULT, 0));`
  - Pass `fg.as_ptr()` as fg; keep `std::ptr::null()` for bg.
  - Corrected safety comment: fg non-nullable per `lib.zig:1800-1810` + `ptrToRGBA :107-109`; bg nullable per `optionalPtrToRGBA :111-117`.
- No API change, no new dependency, no allocation beyond stack `[u16;4]`, no process/thread/auth change. `MAX_TEXT_BYTES` and bounds/ownership/drop untouched. Unsafe scope not expanded (same single `unsafe` block, `fg` alive for whole call).

## RED (pre-edit, exact command)

- `DYLD_LIBRARY_PATH=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/opentui-pinned/packages/native/lib/aarch64-macos CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-opentui-bridge --features native --lib safe_renderer::tests::render_once_content_present -- --exact --test-threads=1`
- Result: exit 101, `signal: 11, SIGSEGV: invalid memory reference`. Log: rk-tui001/red.log.
- Frozen test body pre-hash (SHA-256 of exact fn incl cfg/test attrs): `bc60682701678c08acc025e5238f68af923e6d0d1e38da010842efcdc99dc8e8`.

## GREEN (post-edit, zero test edits)

1. Exact frozen native test: exit 0, `1 passed`, output contains both rows (hello/world asserted). Log: rk-tui001/green-exact.log.
2. Full bridge native lib: `cargo test -p opencode-rk-opentui-bridge --features native --lib -- --test-threads=1` exit 0, `73 passed; 0 failed`. Log: rk-tui001/green-lib-native.log.
3. Bridge default non-native lib: exit 0, `72 passed; 0 failed`. Log: rk-tui001/green-lib-default.log.
4. `cargo check -p opencode-rk-opentui-bridge --features native`: exit 0 (26 pre-existing warnings). Log: rk-tui001/check.log.
5. Focused CLI `native_launch` exact Scenario B (record-only, no test/CLI edits): FAILED pre-existing environment mismatch, not this lane. Test computes `../native/lib/x86_64-linux/libopentui.so` from cwd (absent on macOS arm64, so `artifact_present=false`) and expects missing-artifact message, but CLI exits first on TTY guard: `stdin and stdout are redirected; interactive TUI requires a terminal and raw mode is refused`. Panic at `crates/cli/tests/native_launch.rs:313`. Log: rk-tui001/green-native-launch.log. Untouched per contract.
- Test-body post-hash: `bc60682701678c08acc025e5238f68af923e6d0d1e38da010842efcdc99dc8e8`, identical to pre. `git diff` on safe_renderer.rs shows only the draw_text hunk (comment + fg local + `fg.as_ptr()`); no test-hunk edits.

## Artifact provenance

- Genuine Zig 0.16.0 arm64 dylib: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/opentui-pinned/packages/native/lib/aarch64-macos/libopentui.dylib`, SHA-256 `de3564d496a92fe48fd6ca24a975b1b886cbb654ad5b7eea15bfe81afbf80b50`, Mach-O arm64. Every native run set `DYLD_LIBRARY_PATH` to that dir.
- Disposable symlink `crates/opentui-bridge/native/lib/aarch64-apple-darwin/libopentui.dylib` is an untracked fixture, explicitly excluded from commit.

## Resource bounds

- One Cargo job, one test thread per run; single test, then single lib target, then default lib, then check; CLI focused target once. No parallel builds, no workspace suite.

## Remaining blockers (no acceptance claimed)

- Packaging/provenance: native artifact remains uncommitted; TUI-011/repository approval absent, so distribution story is still open.
- CLI `native_launch` Scenario B fails on this host for the pre-existing TTY-guard-vs-missing-artifact reason above; needs owner disposition outside this lane (no edit made here).
