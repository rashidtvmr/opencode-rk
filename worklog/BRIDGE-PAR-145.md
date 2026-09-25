# BRIDGE-PAR-145 scratchpad

Claim: BRIDGE-PAR-145 via session ses_par145.
Source evidence:
- /home/rashid/projects/opencode/packages/tui/src/routes/session/dialog-subagent.tsx:1-26 (DialogSelect title "Subagent Actions", read-only ref)
- crates/opentui-bridge/src/session_subagent_dialog.rs:1-111 (naming: SubagentDialog, AGENT_ID_CAP=64, truncate; NOT edited)
Observed: TS dialog is thin select list; Rust side needs picker list model.
Target boundary: ONE new file crates/opentui-bridge/src/subagent_dialog.rs. No lib.rs, Cargo.toml, session_subagent_dialog.rs, subagent_footer.rs edits. No cargo, no commit.
Tests: in-file #[cfg(test)] 7 tests (add/select, bounds false, remove shifts, remove active clears, dup allowed, cap+empty, truncate).
Decisions: std-only, forbid(unsafe_code), truncate at char boundary same pattern as sibling file.
Remaining: rustfmt --check only per task.
