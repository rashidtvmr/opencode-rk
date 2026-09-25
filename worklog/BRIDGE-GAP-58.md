# BRIDGE-GAP-58 scratchpad

claim: BRIDGE-GAP-58 ses_gap58 in-progress ok.
source: crates/opentui-bridge/src/keymap.rs (BindingValue, ModeStack); keybind_tables.rs (BindingObject prevent_default/fallthrough, parse_binding_object).
observed: marker form missing; object form exists but no compact !/? stroke + fallthrough resolver.
target: ONE file crates/opentui-bridge/src/keymap_full.rs only.
tests: parse_plain, parse_markers, parse_empty_errs, resolve_first, fallthrough_continues, no_match_none (6 in-file).
decisions: std-only, forbid(unsafe_code), cap 32, 118 lines, no lib.rs/Cargo.toml edits, no cargo per scope.
remaining: none. rustfmt --check FMT_OK.
