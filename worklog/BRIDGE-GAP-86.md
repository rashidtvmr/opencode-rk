# BRIDGE-GAP-86 scratchpad

Claim: BRIDGE-GAP-86 via ses_gap86. Owned file: crates/opentui-bridge/src/session_subagent_dialog.rs.
Source evidence: packages/tui/src/routes/session/dialog-subagent.tsx:1-26 (DialogSelect Open->subagent.view); crates/opentui-bridge/src/run_subagent.rs:11,40-53 (SUBAGENT_ID_CAP=64, spawn rejects empty).
Target boundary: SubagentDialog{agent_id cap64, task cap1KiB, open} + open/close/summary. std-only, forbid(unsafe_code), <150 lines (111).
Tests: open_empty_id_errs, open_truncates_long_id, open_truncates_long_task, close_marks_closed, summary_some_when_open, summary_none_when_closed.
Decisions: free fn open mirrors spawn naming (run_subagent.rs:40); char-boundary-safe truncate; summary "id: task".
Verification: rustfmt --check PASS. No cargo per scope. No commit per scope.
