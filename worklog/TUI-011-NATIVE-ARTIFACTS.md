# TUI-011 builder lane — macOS arm64 bootstrap script + external build evidence

Status: builder script implemented and externally verified; TUI-011 stays
blocked only on copying generated artifact + provenance files into repo.

## Claim
- Task: TUI-011
- Session: `ses_f307abda8ffeXHdsxUCmvo4Qa6`
- Base commit: `ac13e61f27f0914c85223ad8a808f2ebc9f429e9`
- Branch: `lane/TUI-011-native-artifacts`
- Scratchpad owner file: `worklog/TUI-011-NATIVE-ARTIFACTS.md`
- Sole owned production file: `crates/opentui-bridge/native/build_opentui.sh`
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

## Script contract (owned file only)
`crates/opentui-bridge/native/build_opentui.sh`: `set -euo pipefail`,
explicit argv arrays, no eval, bounded download/archive/extracted sizes,
caller-supplied absolute work/output dirs refused inside repo, temp trap,
no inherited secret forwarding, HTTPS only, curl proto/redirect/timeout
limits, zig SHA+size+version verify, shallow fetch exact commit + commit/tree
verify, vendored `scripts/prepare-zig-deps.sh` + `zig build -Dlibrary-target=
aarch64-macos.13.0 -Doptimize=ReleaseSafe -Dmacos-sdk=...`, Mach-O arm64
file/lipo check, nm ABI symbols, rpath/install-name reject, atomic copy,
SHA-only stdout, `--help` / `--verify-only` modes, exits 0/1/2.

## Fixes this session
- Apple-tool positional compat: `file -b`, `lipo -archs`, `nm -gU`,
  `otool -l`, `otool -D` without GNU `--` (macOS otool/lipo reject it).
  `mkdir/du/mktemp/cp/rm/git/cd/tar/tail` accept `--`; left unchanged.
- `du -sk` without `--`; `shasum -a 256` positional (BSD-safe).
- nm/grep pipe under `set -o pipefail` false-negatived on large nm output
  (SIGPIPE + grep -q early exit): replaced with pure-shell `case` substring
  match over newline-padded nm output. No path validation weakened.
- Verified `--verify-only` accepts absolute repo-external artifact paths
  (path allowlist applies only to `--work-dir`/`--output-dir`).

## Validation matrix
- `bash -n crates/opentui-bridge/native/build_opentui.sh`: OK.
- `--help`: exit 0, prints usage + pins.
- `--bogus`: exit 2 (usage error).
- no args: exit 2.
- `--verify-only` without path: exit 2.
- `--verify-only /nonexistent-tui011`: exit 1 (fail-closed).
- Frozen test unchanged, still RED 0/3:
  `cargo test -p opencode-rk-opentui-bridge --test native_artifact_manifest`
  => `test result: FAILED. 0 passed; 3 failed`.

## External build (approved dirs only, nothing copied into repo)
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

## Verify-only on external dylib (exit 0)
- Path: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/tui011-build/out/libopentui.dylib`
- SHA-256: `798f30dd7f4fbe36d52c8834652ed7bcd7f20dfd2a1203d09cc24880eeb13a91`
- Size: 6863648 bytes.
- `file`: Mach-O 64-bit dynamically linked shared library arm64.
- `lipo -archs`: arm64.
- `otool -D`: `@rpath/libopentui.dylib`.
- LC_RPATH count: 0.
- `nm -gU` exports: `_bufferDrawText`, `_bufferWriteResolvedChars`,
  `_createRenderer`, `_destroyRenderer`, `_getCurrentBuffer`.

## Remaining
- TUI-011 blocked only on a follow-up lane copying the generated
  `libopentui.dylib` + `artifacts.json` manifest + NOTICES/SBOM sidecars into
  `crates/opentui-bridge/native/` (out of scope for this lane; no repo
  artifact/manifest/sidecar files written here).
- Commit exactly: script + this scratchpad + claims row; push
  `lane/TUI-011-native-artifacts` (never force).
