# BRIDGE-PAR-312 scratchpad

- Claim: BRIDGE-PAR-312 via cc.claim session ses_par312, scratchpad worklog/BRIDGE-PAR-312.md. OK in-progress.
- Source evidence: TS `packages/tui/src/component/error-component.tsx:10` ErrorComponent(error,reset), `:43` fallback "An unknown error occurred."; existing `crates/opentui-bridge/src/error_component.rs` ErrorView (full crash view, distinct); style model `toast_ui_full.rs:10-48` (caps + line render).
- Observed scenario: no `error_component_full.rs` exists; `error_component.rs` is richer (TaggedError, 32-line cap) and must stay untouched.
- Target boundary: ONE new file `crates/opentui-bridge/src/error_component_full.rs`. No lib.rs, no Cargo.toml, no cargo, no commit.
- Tests: 4 in-file (caps_msg_and_code, code_of_roundtrip, wraps_within_width, caps_eight_rows). Frozen at write.
- Decisions: struct ErrorCard {msg 512, code 32}; new truncates chars; lines wraps `code: msg` greedy + hard-split, cap 8 rows, width 0 empty; std-only, forbid(unsafe_code); ponytail char-width noted.
- Remaining unknowns: none. Verification rustfmt --check only per task.
