# BRIDGE-GAP-73

Claim: ses_gap73 via tools/completion_claims.py claim. Status: completed.

Source evidence:
- TS truth: packages/opencode/src/cli/cmd/run/permission.shared.ts (PermissionBodyState queue side; this lane owns queue rows only)
- Sibling (read-only): crates/opentui-bridge/src/run_permission.rs:1-87 (Stage/PermissionReq/advance)

Target boundary: ONE new file crates/opentui-bridge/src/run_permission_shared.rs. No edits to lib.rs, Cargo.toml, run_permission.rs, permission_kinds.rs.

Tests (in-file #[cfg(test)], 7):
- request_ok_pushes_unresolved_row
- request_rejects_tool_over_64_chars
- request_rejects_detail_over_512_chars
- request_rejects_full_queue
- resolve_sets_allow_flag
- resolve_oob_returns_false
- pending_counts_only_unresolved

Decisions: std-only, forbid(unsafe_code), char-count caps (unicode-safe), 128 lines (<170). Empty tool rejected (matches sibling PermissionReq semantics).

Verification: rustfmt --check clean. No cargo run per scope. Unknowns: none.
