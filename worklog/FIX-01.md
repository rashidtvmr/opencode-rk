# FIX-01: platform_manifest_full.rs

## Claim
- Task: create platform artifact manifest module
- Scope: `crates/opentui-bridge/src/platform_manifest_full.rs` only
- Owner: DeepSeek V4.1 (subagent)

## Source evidence
- `crates/opentui-bridge/build.rs:19` - joins `native/lib/<triple>`
- `crates/opentui-bridge/build.rs:26-64` - fail-closed panic on missing native artifact; gates `.so`/`.dylib`/`.lib`/`.dll`
- Context: only `linux-x86_64` `.so` vendored today (25.4M); `--features native` linux-x86_64 only

## Target boundary
- `TRIPLES` list with: linux-x86_64 .so, linux-aarch64 .so, darwin-aarch64 .dylib, darwin-x86_64 .dylib, windows .lib/.dll
- `artifact_filename(triple) -> Result<&'static str, ManifestError>`
- `is_supported(triple) -> bool`
- fail-closed `ManifestError` enum
- `#![forbid(unsafe_code)]`, std-only, <100 lines, >=4 in-file tests
- Tests cite `build.rs:19,26-64`

## Verification
- `rustfmt --check crates/opentui-bridge/src/platform_manifest_full.rs` -> EXIT 0 (PASS)
- 99 lines (under 100 limit)
- 4 in-file tests (>=4 requirement), cite build.rs:19,26-64
- No cargo build (out of scope)
