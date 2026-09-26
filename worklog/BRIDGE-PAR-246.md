# BRIDGE-PAR-246 scratchpad

claim: BRIDGE-PAR-246 via cc.claim ses_par246 OK (in-progress).
source: crates/opentui-bridge/src/data_projection.rs:1-76 (DataRow/project/filter_kind, forbid unsafe, std-only). NOT edited.
target: ONE new file crates/opentui-bridge/src/data_projection_full.rs. No lib.rs/Cargo.toml/data_projection.rs edits. No cargo/commit.
impl: ProjectionFull {rows cap 512, cursor}, push->bool (trunc 4096 chars), move_cursor(delta isize, clamp), window(n) from cursor cap n. forbid(unsafe_code).
tests: 6 in-file (push_ok, cap, trunc, clamp, empty, window).
verify: rustfmt --check only (scope).
