# BRIDGE-PAR-262 scratchpad

Claim: BRIDGE-PAR-262, session ses_par262.
Source evidence:
- crates/opentui-bridge/src/theme_engine.rs:19 ThemeEngine {name, mode, locked}, apply->bool, locked gate
- crates/opentui-bridge/src/theme_picker.rs:7 ThemePicker {engine, index}, next/prev/apply_known, is_known gate
- crates/opentui-bridge/src/theme_assets.rs:15 ASSET_NAMES incl dracula/opencode
- Pattern ref: dialog_stack_full.rs:11 DialogFlow counter, toast_full2.rs:11 ToastFlow counter
Target boundary: ONE new file theme_engine_full.rs; no lib.rs/Cargo.toml/theme_engine.rs/theme_picker.rs edits; no cargo/commit.
Tests: 5 in-file (new_zero, apply_known_bumps, apply_unknown_no_bump, next_delegates, multi_apply_counts).
Decisions: apply delegates to apply_known (known-only) so unknown returns false no bump; next delegates without bump (matches picker cycle semantics); saturating_add; pub fields per spec.
Verification: rustfmt --check EXIT 0; wc 79 lines (<110); std-only, forbid(unsafe_code).
Unknowns: none.
