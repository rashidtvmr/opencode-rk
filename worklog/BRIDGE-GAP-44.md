# BRIDGE-GAP-44
claim: ses_gap44 owns file
source: crates/opentui-bridge/src/run_permission.rs:1-51 (Stage Ask/Always/Reject, advance semantics)
boundary: new file crates/opentui-bridge/src/permission_kinds.rs only; lib.rs/Cargo.toml untouched; no cargo run
tests: twelve_infos_non_empty, read_default_allow, glob_grep_default_allow, bash_default_deny, parse_known_names, parse_unknown_returns_none
verify: rustfmt --check (run)
