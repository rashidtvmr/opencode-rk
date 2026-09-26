# BRIDGE-GAP-42 scratchpad

Claim: BRIDGE-GAP-42, session ses_gap42.
Source: task prompt (foregroundTasks/background-subagent gate).
Target: crates/opentui-bridge/src/task_gates.rs only.
Tests: in-file #[cfg(test)] >=5.
Decisions: plain struct, fail-closed spawn err at cap, saturating finish.
