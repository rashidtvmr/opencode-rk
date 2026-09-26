# BRIDGE-PAR-371 (unclaimed, file-only per spawn orders)

Claim: skipped ledger (orchestrator owns claims.json). Proceeded file-only.
Source: packages/tui/src/keymap.tsx:20 LEADER_TOKEN, :53-60 stack, keybind.ts:41 default.
Observed: no keymap_tsx_full.rs existed; keymap.rs/keymap_format.rs cover close neighbors.
Target: crates/opentui-bridge/src/keymap_tsx_full.rs, KeymapTsx + set_leader/bump/leader_of/count, <80 lines, std-only, forbid unsafe.
Tests: 3 (default, cap-16, bump-saturate). Decisions: chars().take(16) cap; saturating_add. Unknowns: none.
