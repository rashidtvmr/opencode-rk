# BRIDGE-PAR-390 scratchpad (UNCLAIMED: orchestrator owns claims.json, no claim made per task order)

- Claim: none (file-only lane, ledger untouched).
- Source evidence:
  - TS truth: packages/tui/src/keymap.tsx:1-60 (LEADER_TOKEN:20, mode stack:53-60, pending/leader addons:3-9).
  - Sibling pattern: crates/opentui-bridge/src/keymap_tsx_full.rs:1-40 (forbid unsafe, MAX cap, new/set/bump/get).
  - Bound style: crates/opentui-bridge/src/keybind_config_full.rs:1-40 (64-char sides, fail-closed).
- Target boundary: ONE new file crates/opentui-bridge/src/keymap_ts_full.rs; lib.rs/Cargo.toml untouched.
- Tests: 4 unit tests in-file (empty, accumulate, 64-char cap, take drains).
- Decisions: keep-first-64 truncation (matches sibling take(MAX) style); `take` via mem::take; std-only; 57 lines.
- Unknowns: wiring into lib.rs left to orchestrator (out of scope).
