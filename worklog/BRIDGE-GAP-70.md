# Claim BRIDGE-GAP-70 ses_gap70
## Source evidence
- TS truth: packages/opencode/src/cli/cmd/run/subagent-data.ts (tabs/details, listSubagentTabs sort running-first, snapshotSubagentData)
- Naming ref: crates/opentui-bridge/src/run_subagent.rs:11 SUBAGENT_ID_CAP=64, :15 TRANSCRIPT_CAP=500, SubStatus Running/Done/Failed
## Target boundary
- ONE file crates/opentui-bridge/src/run_subagent_data.rs only. No lib.rs/Cargo.toml/run_subagent.rs edits. No cargo. No commit.
## Tests
- in-file #[cfg(test)] 5+: summary_counts, empty_zero, filter_matches, filter_none, cap_respected
## Decisions
- SubagentRow flat row (id cap64, status cap32, lines u32); summarize exact format; filter_by_status cap 500
## Unknowns
- none
