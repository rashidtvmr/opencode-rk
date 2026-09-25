# BRIDGE-PAR-349 (unclaimed: ledger overflow, orchestrator owns claims)

Claim: none (per spawn instruction, no ledger touch). File-only lane.
Source: packages/tui/src/attention.ts (full read) - focus/sound gates, no
counter upstream; attention.rs:1-164 (existing FocusState/focus_skip gates).
Target: crates/opentui-bridge/src/attention_full.rs - Attention badge only,
companion to attention.rs. No lib.rs/Cargo.toml edits.
Tests: 4 (new, clear, requestx2, saturate). Decisions: saturating_add;
clear resets both (invariant needed == count>0); struct fields pub +
getters; Default+new. Unknowns: none.
Verify: rustfmt --check only (below).
