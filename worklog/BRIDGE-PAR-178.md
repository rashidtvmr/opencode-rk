# BRIDGE-PAR-178 scratchpad
claim: BRIDGE-PAR-178 ses_par178 worklog/BRIDGE-PAR-178.md
evidence: crates/opentui-bridge/src/theme_engine.rs:19 ThemeEngine{name,mode,locked}, :55 apply locked-blocks; crates/opentui-bridge/src/theme_assets.rs:6 ASSET_NAMES[33], :43 is_known, :48 fallback_chain
boundary: new file only crates/opentui-bridge/src/theme_picker.rs; no lib.rs/Cargo.toml/theme_engine/theme_assets edits; no cargo/commit
tests: 7 unit tests in-file; verification rustfmt --check only per task
decisions: pub fields engine/index for struct literal compat; locked blocks next/prev/apply_known; ponytail comment kept
status: file 120 lines, fmt OK
