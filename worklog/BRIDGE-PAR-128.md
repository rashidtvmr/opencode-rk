# BRIDGE-PAR-128 scratchpad

claim: BRIDGE-PAR-128 ses_par128
source: packages/tui/src/context/theme.tsx:84-98 (mode dark|light, lock dark|light|undef, active opencode), lines 114-120 lock>renderer>props fallback
target: crates/opentui-bridge/src/theme_engine.rs only; no lib.rs/Cargo.toml edits
tests: 6 in-file (apply ok, locked blocks, trunc 64, label parts, unlock ok, set_mode roundtrip)
decision: ThemeEngine::new seeds default then apply; empty name falls back opencode; ponytail no registry validation
unknowns: none
