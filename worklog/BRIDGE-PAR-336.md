# BRIDGE-PAR-336 scratchpad (UNCLAIMED - ledger overflow, orchestrator owns claims.json)

- Claim: NOT claimed. Per delegation override: tasks/completion/claims.json untouched (ledger overflow). Proceeding file-only.
- Source evidence: packages/tui/src/component/use-connected.tsx:4-12 (useSync provider memo, true when id != "opencode" or model cost.input != 0). Style ref: crates/opentui-bridge/src/offline_banner.rs:1 (forbid unsafe).
- Observed scenario: no use_connected*.rs exists in crates/opentui-bridge/src (dir listing).
- Target boundary: ONE new file crates/opentui-bridge/src/use_connected_full.rs. No lib.rs / Cargo.toml edits. No cargo, no commit.
- Tests: 3 unit tests in-file (new_zero_flips, set_change_only, flips_each_transition).
- Decisions: struct Connected { online: bool, flips: u32 } Copy+Default; set() saturating_add on change only; is_online/flips getters; 74 lines, std-only, forbid(unsafe_code).
- Unknowns: module wiring left to orchestrator (lib.rs untouched so file is standalone until wired).
