# BRIDGE-GAP-30: Native Arch Module

## Claim
- Task: Create `crates/opentui-bridge/src/native_arch.rs`
- Session: ses_gap30
- Status: in-progress

## Source Evidence
- build.rs:50-64 - Fail-closed gate for missing artifacts
- Native lib exists at: `native/lib/x86_64-unknown-linux-gnu/` (contains `libopentui.so`)

## Target Boundary
- One file: `crates/opentui-bridge/src/native_arch.rs`
- std-only, forbid(unsafe_code)
- Under 180 lines
- Triple matrix: x86_64-linux, aarch64-linux, x86_64-macos, aarch64-macos, x86_64-windows
- Status enum: Present/Missing
- Helper functions: artifact_name(triple)->&str, missing_message(triple)->String
- >=5 #[cfg(test)] tests

## Verification
- rustfmt --check only

## Progress
- [x] Claim made
- [ ] File created
- [ ] Tests pass
- [ ] rustfmt check passes