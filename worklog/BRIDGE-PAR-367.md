# BRIDGE-PAR-367 (unclaimed, file-only)

Task: crates/opentui-bridge/src/ctx_project_full.rs. Orchestrator owns claims.json; no claim made per task override.
Claim: none (unclaimed lane, file-only delivery).
Evidence: packages/tui/src/context/project.tsx:1-50 (project id/worktree/mainDir + instance path directory); existing style crates/opentui-bridge/src/ctx_bundle.rs, context_project.rs, directory_ctx.rs.
Target boundary: CtxProject {root 512, name 128} + set_root + set_name + label cap 256. std-only, forbid(unsafe_code), <90 lines.
Tests: new_truncates_caps, set_root_empty_false, set_name_empty_false, label_format_and_cap.
Decisions: fail-closed empty rejects; char-wise truncate; label "name @ root" with empty-side fallback; ponytail: no SDK/workspace sync, add when wired.
Unknowns: none.
