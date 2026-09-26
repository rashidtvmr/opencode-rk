# BRIDGE-PAR-384 (unclaimed, file-only per orchestrator)

Claim: implement PromptRef holder in one new file.
Source evidence:
- packages/tui/src/context/prompt.tsx:4-18 (PromptRefProvider current/set holder)
- packages/tui/src/component/prompt/index.tsx:88-96 (PromptRef type: focused/current/set/reset/blur/focus/submit)
Observed scenario: TS context holds `current: PromptRef|undefined` with set(); Rust needs minimal id+live mirror.
Target boundary: crates/opentui-bridge/src/ctx_promptref_full.rs only. No lib.rs/Cargo.toml edits.
Tests: default_not_live, set_roundtrips_id, set_truncates_to_128, clear_unsets_live (4 tests inline).
Decisions: id String cap 128 chars (mirrors ctx_theme_full 64-char pattern); empty set clears live; `ponytail:` dropped, full focused/blur/submit behavior out of scope.
Remaining: wiring into lib.rs by integrator.
