# BRIDGE-PAR-403 scratchpad (UNCLAIMED - orchestrator owns claims.json, file-only lane)

- claim: skipped per parent instruction (no claims.json touch). status: unclaimed.
- source: packages/tui/src/routes/session/dialog-subagent.tsx:1-26 (DialogSubagent, DialogSelect "Subagent Actions", option subagent.view).
- target: crates/opentui-bridge/src/sess_sub_dlg_full.rs only. lib.rs/Cargo.toml untouched. no cargo/commit.
- impl: SubDlg {agent: String cap 128 chars, picked: bool}, new/default, pick(&mut,&str), confirm(&mut)->bool (one-shot consume), agent_of(&self)->&str. std-only, forbid(unsafe_code).
- tests: pick_sets_agent, confirm_consumes, cap_128_chars (3, inline).
- verify: rustfmt --check only (not run - no shell per scope; parent runs).
- unknowns: none. confirm consume semantics chosen; rewire trivial if caller wants peek.
