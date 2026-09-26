# BRIDGE-PAR-334 (unclaimed: orchestrator owns ledger, overflow blocks claims; file-only)

Claim: none (per task prompt, no ledger touch).
Source: packages/tui/src/component/dialog-workspace-file-changes.tsx:1 DialogWorkspaceFileChanges files list; height min(len,8); patterns ex dialog_ws_create_full.rs, dialog_select.rs.
Target: crates/opentui-bridge/src/dialog_ws_changes_full.rs.
Tests: push_ok_truncates, push_rejects_empty_and_full, move_cursor_clamps, selected_none_when_empty.
Decisions: truncate (not reject) overlong paths, ponytail ceiling MAX 64/512; clamp cursor not wrap; std-only, forbid unsafe.
Unknowns: wiring into lib.rs left to orchestrator.
