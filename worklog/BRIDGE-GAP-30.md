# BRIDGE-GAP-30 scratchpad
claim: BRIDGE-GAP-30 ses_gap30 worklog/BRIDGE-GAP-30.md in-progress
source: crates/opentui-bridge/build.rs:50-64 fail-closed gate (expected names per OS/env)
target: crates/opentui-bridge/src/native_arch.rs only; no lib.rs/Cargo/build.rs/cargo/commit
impl: MATRIX 6 triples + ArtifactStatus Present/Missing + artifact_name + missing_message; std-only forbid(unsafe_code)
tests: linux_shared_names macos_shared_names windows_import_names unknown_triple missing_linux_text missing_macos_text missing_windows_text matrix_covers_five_targets (8)
verify: rustfmt --check clean
