# BRIDGE-PAR-417 (unclaimed, file-only, orchestrator owns claims.json)

- Claim: NOT claimed (per instructions, no ledger touch).
- Source: packages/tui/src/feature-plugins/system/which-key.tsx:24-40 (toggle/panel/scroll commands, overlay state).
- Pattern: crates/opentui-bridge/src/question_full.rs (forbid unsafe, trunc caps, struct + impl + tests).
- Target: crates/opentui-bridge/src/which_key_full.rs, <90 lines, std-only.
- Tests: 4 (add_caps, truncates_long, hidden_gives_empty, shown_respects_max).
- Decisions: pub fields per spec; visible_lines empty when hidden; add ignores over cap.
- Unknowns: wiring into lib.rs left to orchestrator.
