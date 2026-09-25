# BRIDGE-PAR-210 scratchpad

claim: BRIDGE-PAR-210 via cc.claim, session ses_par210, scratchpad worklog/BRIDGE-PAR-210.md. OK in-progress.
source evidence:
- crates/cli/src/tui_entry.rs:486 `lines.push("-".repeat(width.min(120)));` per task card; actual repo file uses `"─".repeat(width.min(120))` at :486 and :511 (box-draw char, not ASCII dash). Followed task deliverable ("-" repeat) as authority.
- sibling pattern: crates/opentui-bridge/src/page_adapter.rs:38-40 `fn rule(width) -> String { "-".repeat(width.max(1).min(120)) }` (private); style mirror: crates/opentui-bridge/src/toast_line.rs (forbid unsafe, ponytail note, in-file tests).
observed: no existing rule_line.rs; lib.rs untouched per scope.
target boundary: ONE new file crates/opentui-bridge/src/rule_line.rs. No lib.rs, no Cargo.toml, no cargo, no commit.
tests: 5 in-file (zero-floors-one, exact-width, caps-120, boundary-120/121, width-one). Unrun per scope (no cargo); rustfmt --edition 2021 --check EXIT 0.
decisions: ASCII "-" per deliverable; RULE_CAP const 120 pub; max(1) floor; 66 lines (<70); std-only; forbid(unsafe_code).
remaining: integrator prewires `pub mod rule_line;`.
