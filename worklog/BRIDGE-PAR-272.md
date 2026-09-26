# BRIDGE-PAR-272 scratchpad

claim: BRIDGE-PAR-272 via ses_par272, worklog/BRIDGE-PAR-272.md.
source: crates/opentui-bridge/src/home_footer.rs:20 HomeFooter {tips,index}, :50 current()->Option<&str>, :44 next().
target: crates/opentui-bridge/src/home_footer_full.rs only. lib.rs/Cargo.toml/home_footer.rs untouched.
tests: starts_zero, current_returns_tip, current_bumps_views, empty_footer_empty_string, follows_rotation (5).
decisions: struct holds footer by value + u64 views; current takes &mut, saturating_add, clone to String; std-only, forbid(unsafe_code), 71 lines.
evidence: rustfmt --check FMT_OK.
