# BRG-RUNTIME scratchpad

## Task
Implement `crates/opentui-bridge/src/runtime.rs` — runtime platform detection and native lib path resolution, mirroring opentui TS `node-asset-target.ts`, `runtime-assets.node.ts`, `assets.ts`.

## Source evidence (opentui @4954312d)
- `packages/core/src/node-asset-target.ts:1-52`:
  - `NodeAssetTarget { platform: darwin|linux|win32, arch: arm64|x64, libc?: glibc|musl }`
  - `getNativeAssetDescriptor`: fileName = darwin `libopentui.dylib`, linux `libopentui.so`, win32 `opentui.dll`
  - key = `@opentui/core-{platform}-{arch}{libcSuffix}/{fileName}`
  - libcSuffix = `-musl` only when platform=linux && libc=musl
  - packageName = `@opentui/core-{platform}-{arch}{libcSuffix}`
- `packages/core/src/platform/runtime-assets.node.ts:26-35`: resolveNativeLibraryPath
  - calls getNativeAssetDescriptor(getCurrentNodeAssetTarget()) -> asset
  - checks OTUI_ASSET_ROOT env via resolveAssetRootPath(asset.key)
  - if configured, return that path
  - else dynamic import(asset.packageName).default
- `packages/core/src/platform/runtime-assets.bun.ts:41-52`: platform detection
  - process.platform, process.arch, OPENTUI_LIBC env on linux
- `packages/core/src/platform/assets.ts:32-53`: resolveAssetRootPath
  - reads OTUI_ASSET_ROOT env, joins root + key, validates isFile

## Target boundary
- Owned file: only `crates/opentui-bridge/src/runtime.rs`
- Std-only, self-contained, no unsafe, no todo!/unimplemented!
- Markers: `pub enum Runtime`, `pub fn detect`, `pub struct AssetPath`, `pub fn resolve`
- >= 80 lines, >= 5 tests

## Decisions
- `Runtime` is `pub enum Runtime` with per-OS variants carrying arch + optional libc (satisfies marker while preserving TS `NodeAssetTarget` data model).
- `detect()` uses `cfg!(target_os/target_arch)` + `OPENTUI_LIBC` env, validates libc-only-on-linux internally.
- `AssetPath.resolve()` checks `OTUI_ASSET_ROOT` (absolute join + file stat), else returns key as package specifier.

## Tests
9 tests, all pass. rustc --edition 2021 --test runtime.rs -> /tmp/opencode/brg_runtime_test OK.

## Status
- Ledger: completed
- Line count: 345 (>= 80)
- Markers: pub enum Runtime (47), pub fn detect (141), pub struct AssetPath (192), pub fn resolve (214)
- forbid(unsafe_code), no todo!/unimplemented!/panic!
- No tests edited post-RED (written alongside impl, frozen)

## Remaining unknowns
- Module not wired into lib.rs (owned-file-only constraint; caller integration is orchestrator's job).
