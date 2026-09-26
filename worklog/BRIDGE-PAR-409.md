# BRIDGE-PAR-409 (unclaimed, file-only per orchestrator)

Lease: ONE new file `crates/opentui-bridge/src/ui_border_full.rs`.
No claim (orchestrator owns claims.json). No edit to lib.rs, Cargo.toml, border_ui_full.rs. No cargo, no commit.

## Source evidence
- TS truth `packages/tui/src/ui/border.ts:1-21`: EmptyBorder + SplitBorder only; no rounded/ascii styles. New helpers are convention fills, marked ponytail.
- Sibling style: `crates/opentui-bridge/src/border.rs:1` forbid(unsafe_code); `border_ui_full.rs:15-17` box_width clamp idiom.

## Target boundary
- border_char(style:&str)->char, border_width(w:u32)->u32 min 2, is_rounded(style:&str)->bool. std-only, forbid(unsafe_code), <60 lines.

## Tests
- 3 tests: ascii_dash_else_box, width_min_two, rounded_only. Not run (cargo banned); rustfmt only.

## Decisions
- Saturating min via max(2); rounded = exact "rounded"; else branch returns U+2500.

## Unknowns
- Compilation unproven (no cargo per scope). Wiring into lib.rs left to orchestrator.
