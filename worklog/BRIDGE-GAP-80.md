# BRIDGE-GAP-80 scratchpad

Claim: BRIDGE-GAP-80 via ses_gap80, ledger in-progress -> completed.
Source evidence:
- TS truth: /home/rashid/projects/opencode/packages/opencode/src/cli/cmd/run/tool.ts (ToolPhase start/progress/final, status running/completed/error; runTask icon running/completed/error at :369)
- Naming/style: crates/opentui-bridge/src/tool_output.rs (forbid unsafe, must_use, #[cfg(test)] mod tests)
Target boundary: ONE new file crates/opentui-bridge/src/run_tool.rs only. No lib.rs/Cargo.toml edits, no cargo run, no commit.
Tests: 6 in-file (upsert_adds_row, upsert_updates_existing, set_state_missing_is_false, counts_split_states, caps_rows_and_name_len, state_transitions).
Decisions: truncate by chars (unicode-safe); upsert matches on truncated key so long names alias; full-table upsert of existing still true.
Verification: rustfmt --check crates/opentui-bridge/src/run_tool.rs -> FMT_OK. Line count 162 (<170).
