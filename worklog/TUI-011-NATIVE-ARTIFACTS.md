# TUI-011 native binary lane — macOS arm64 artifact

Status: generated dylib verified and native bridge suite green. TUI-011 stays
blocked on the coupled `artifacts.json`, `NOTICES`, and `sbom.json` sidecars.

## Claim
- Task: TUI-011
- Session: `ses_f3065a9f3ffea7opvjVcfLQ5I2`
- Starting commit: `ea968bd0c3cfda515f4fbcfc7d441d42b15a3ceb`
- Branch: `lane/TUI-011-native-artifacts`
- Scratchpad owner file: `worklog/TUI-011-NATIVE-ARTIFACTS.md`
- Sole owned production file:
  `crates/opentui-bridge/native/lib/aarch64-apple-darwin/libopentui.dylib`
- Frozen test untouched: `crates/opentui-bridge/tests/native_artifact_manifest.rs`
  SHA-256 `efb642364b6a4dc559fff019d21c14f778c170f34f96bf5e7183b4aeb536aa40`

## Source evidence
- `docs/architecture/COMPLETION_NATIVE_TUI.md:14-20`: fork pin
  `c01292fd0837bafd07ce458c74416b2b375a41ab`, native tree `packages/native`,
  build in `packages/native/build.zig`.
- Live pinned upstream at commit: `build.zig.zon` `minimum_zig_version 0.16.0`;
  `build.zig` SUPPORTED_ZIG_VERSIONS exact 0.16.0; target `aarch64-macos.13.0`;
  iface `zig build -Dlibrary-target=<t> -Doptimize=<m> [-Dmacos-sdk=...]`;
  output `lib/aarch64-macos/libopentui.dylib`.
- Zig index `https://ziglang.org/download/index.json`: 0.16.0 aarch64-macos
  tarball `https://ziglang.org/download/0.16.0/zig-aarch64-macos-0.16.0.tar.xz`,
  sha256 `b23d70deaa879b5c2d486ed3316f7eaa53e84acf6fc9cc747de152450d401489`,
  size 52238004.
- GitHub API commit object: sha `c01292f...`, tree
  `261e8ea4b0ac68bb589760c3c871e202ef71ed6b`.

## Prior builder contract (reference only; not owned this takeover)
`crates/opentui-bridge/native/build_opentui.sh`: `set -euo pipefail`,
explicit argv arrays, no eval, bounded download/archive/extracted sizes,
caller-supplied absolute work/output dirs refused inside repo, temp trap,
no inherited secret forwarding, HTTPS only, curl proto/redirect/timeout
limits, zig SHA+size+version verify, shallow fetch exact commit + commit/tree
verify, vendored `scripts/prepare-zig-deps.sh` + `zig build -Dlibrary-target=
aarch64-macos.13.0 -Doptimize=ReleaseSafe -Dmacos-sdk=...`, Mach-O arm64
file/lipo check, nm ABI symbols, rpath/install-name reject, atomic copy,
SHA-only stdout, `--help` / `--verify-only` modes, exits 0/1/2.

## Prior builder validation matrix (reference only)
- `bash -n crates/opentui-bridge/native/build_opentui.sh`: OK.
- `--help`: exit 0, prints usage + pins.
- `--bogus`: exit 2 (usage error).
- no args: exit 2.
- `--verify-only` without path: exit 2.
- `--verify-only /nonexistent-tui011`: exit 1 (fail-closed).
- Frozen test unchanged, still RED 0/3:
  `cargo test -p opencode-rk-opentui-bridge --test native_artifact_manifest`
  => `test result: FAILED. 0 passed; 3 failed`.

## Prior external build (approved dirs only, nothing copied into repo)
- Work: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/tui011-build/work`
- Out: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/tui011-build/out`
- Stdout log: `tui011-build/build-stdout.log`; stderr: `tui011-build/build-stderr.log`.
- First full run: fetch + zig + prepare + build succeeded; built artifact
  verified (file/lipo ok) but ABI check failed from the grep/SIGPIPE bug.
- After pipe-free fix, second full run exit 0:
  `798f30dd7f4fbe36d52c8834652ed7bcd7f20dfd2a1203d09cc24880eeb13a91  .../tui011-build/out/libopentui.dylib`
- Stderr: source `commit c01292f... tree 261e8ea...` ok; zig archive sha ok;
  zig 0.16.0 ok; file `Mach-O 64-bit dynamically linked shared library arm64`;
  lipo `arm64`; ABI symbols ok; no LC_RPATH; install name `@rpath/libopentui.dylib`.

## Prior external verify-only (exit 0)
- Path: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/tui011-build/out/libopentui.dylib`
- SHA-256: `798f30dd7f4fbe36d52c8834652ed7bcd7f20dfd2a1203d09cc24880eeb13a91`
- Size: 6863648 bytes.
- `file`: Mach-O 64-bit dynamically linked shared library arm64.
- `lipo -archs`: arm64.
- `otool -D`: `@rpath/libopentui.dylib`.
- LC_RPATH count: 0.
- `nm -gU` exports: `_bufferDrawText`, `_bufferWriteResolvedChars`,
  `_createRenderer`, `_destroyRenderer`, `_getCurrentBuffer`.

## Takeover verification @ `ea968bd0c3cfda515f4fbcfc7d441d42b15a3ceb`
- No bytes regenerated or modified. Owned path was already present untracked.
- SHA-256: `798f30dd7f4fbe36d52c8834652ed7bcd7f20dfd2a1203d09cc24880eeb13a91`.
- Size: `6863648` bytes.
- `file -b`: `Mach-O 64-bit dynamically linked shared library arm64`.
- `lipo -archs`: `arm64`.
- `otool -D`: `@rpath/libopentui.dylib`.
- `otool -l`: `LC_RPATH=0`.
- Required ABI exports: `_createRenderer`, `_destroyRenderer`,
  `_getCurrentBuffer`, `_bufferDrawText`, `_bufferWriteResolvedChars`.
- `bash crates/opentui-bridge/native/build_opentui.sh --verify-only <owned>`:
  exit 0; same SHA; all builder checks above passed.
- Pre-test host memory: 5.82 GiB free pages, 2.25 GiB active, 4.34 GiB
  inactive, 2.54 GiB wired; host physical memory 24 GiB.

## Native loader/ABI evidence
- Command:
  `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 DYLD_LIBRARY_PATH="$PWD/crates/opentui-bridge/native/lib/aarch64-apple-darwin" DYLD_FALLBACK_LIBRARY_PATH="$PWD/crates/opentui-bridge/native/lib/aarch64-apple-darwin" cargo test -p opencode-rk-opentui-bridge --features native --lib -- --test-threads=1`
- Exit 0: `73 passed; 0 failed`; loader found and executed real renderer FFI.
- No source, test, manifest, sidecar, or builder-byte edits.

## Frozen artifact test
- Command:
  `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-opentui-bridge --test native_artifact_manifest -- --test-threads=1`
- Cargo output condensed the panic to `Error: No such file or directory`;
  direct execution of the Cargo-produced frozen test binary confirmed exact
  count: `0 passed; 3 failed`.
- Failure causes: `native/artifacts.json` absent; frozen artifact test coupled
  to manifest; `native/NOTICES` absent. `native/sbom.json` is also required
  after the first sidecar failure clears.
- Frozen test file remained unchanged at the recorded SHA-256.

## Remaining blocker
- Sidecar/manifest lane must add bounded `artifacts.json`, `NOTICES`, and
  `sbom.json` under `crates/opentui-bridge/native/`, then rerun frozen test.
- No claim of TUI-011 completion or parent acceptance.
