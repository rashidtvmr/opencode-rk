# BRIDGE-PAR-319 dialog_stash_full

Claim: attempted via tools/completion_claims.py session ses_par319; FAILED pre-existing ledger drift (load_ledger raises "claims must be a bounded mapping": claims.json lacks schemaVersion=1, huge row count). Out of scope to fix (shared file). Proceeded with owned file only.
Source evidence: packages/tui/src/component/dialog-stash.tsx:29 DialogStash onSelect restore; :37 stash.list most-recent-first; :45 delete confirm gate (TS layer).
Target boundary: ONE new file crates/opentui-bridge/src/dialog_stash_full.rs. No lib.rs/Cargo.toml edits. No cargo/commit.
Tests: 6 in-file (push_ok, blank_reject, truncate, cap, cursor_clamp, restore_clone). Verification: rustfmt --edition 2021 --check PASS (FMT_OK), 113 lines <120. std-only forbid(unsafe_code).
Decisions: model fork_dialog_full.rs shape; delete-confirm left in TS per truth; cursor clamped isize.
Remaining: ledger claim/update impossible until orchestrator repairs claims.json schema; status NOT set (no false completed).
