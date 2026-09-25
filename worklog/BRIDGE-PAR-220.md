# BRIDGE-PAR-220 scratchpad
claim: ses_par220 in-progress->completed
evidence: subagent_wire.rs:17-51 SubagentWire::new/push_output/status(STATUS_CAP 512); footer.subagent.tsx:1-50 statusColor/statusIcon rows; run_subagent.rs:12,15 caps
boundary: new file only, no lib.rs/Cargo.toml/subagent_wire.rs edits
tests: 6 (new_is_empty, spawn cap, empty-id, feed, feed-oob, statuses-512)
decision: thin Vec<SubagentWire> wrapper, delegate feed->push_output, statuses->status
verify: rustfmt --check clean, 101 lines <130
