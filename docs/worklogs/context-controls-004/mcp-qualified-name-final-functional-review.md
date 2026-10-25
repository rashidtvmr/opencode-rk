# MCP Qualified Name — Final Functional Review

**Date:** 2026-10-25  
**Review type:** Source + test static review (NO compilation or test execution)  
**Scope:** `crates/server/src/mcp_context.rs` + new test files vs HEAD `fa845813`

## Hash Manifest (Python sha256, exact)

| File | SHA256 | Bytes |
|------|--------|-------|
| `crates/server/src/mcp_context.rs` (production, working tree) | `f39ac724573fcf32d45cdef41ead4a4e88977196be03cf9fa514414d36f9caf1` | 12806 |
| `crates/server/src/mcp_legacy_deny_tests.rs` (new/updated) | `5c83d4ddf195c9cfcbddd94572bb1be77a346a916b0cc9863e1cf82458297744` | 10865 |
| `crates/server/src/mcp_context_collision_tests.rs` (frozen) | `ad8c2501953786d88b60ce7d4b92bcb65fc6b10e139a666c4b0d4e5693085f5f` | 3734 |

> The collision test hash matches the frozen baseline from AGENTS.md exactly. No modification.

## Check Results

| Check | Verdict | Evidence |
|-------|---------|----------|
| Qualifier 64-byte bounds | APPROVE | `stable_name`: suffix cap = `64 - 38 = 26`; max name 62 bytes; `legacy_deny_name`: suffix cap = `64 - 22 = 42`. Tests assert `<= MAX_TOOL_NAME_BYTES (64)`. Non-ASCII tool sanitized to underscore; long-input test caps at 62 bytes, all ASCII. |
| Duplicate names rejected | APPROVE | `effective_tools` inserts into `seen_names` HashSet; returns `Err("duplicate MCP qualified name rejected: {name}")` on collision. No first-match collapse. |
| Registry routing preserved | APPROVE | `execute` lookup unchanged: `defs.into_iter().find(|d| d.name == call_name)`. New 128-bit name used as canonical identity; legacy name ONLY for deny-check. |
| Legacy DENY fingerprints preserved | APPROVE | `is_denied` reconstructs `legacy_intent` with `name: legacy_name` + identical `description` field, then `permission_rules::denied` serializes via `serde_json::to_string` and `fingerprint`. Test `resource_scoped_legacy_deny_blocks_current_intent` calls `super::is_denied` directly and asserts true. |
| Aliases cannot authorize | APPROVE | `is_denied` is deny-only OR. A legacy **allow** rule (resource-scoped) yields `is_denied == false` (no deny) but does NOT grant the new 128-bit name human-approval capability. `execute` still requires the positive approval path; see test `matching_legacy_allow_does_not_deny_current_intent`. Source-level proof only — no test execution.

## Test File Changes Since Initial Review

- `mcp_legacy_deny_tests.rs` changed: hash is `5c83d4dd...` (not the previously reported `9f3c80bb...`).
- New test `matching_legacy_allow_does_not_deny_current_intent` added: legacy allow is non-denying; it does not supply the new-name human-approval capability, and `execute` still requires it.
- Duplicate test renamed: `duplicate_advertised_name_is_rejected` -> `legacy_collision_aliases_are_replaced_by_distinct_current_names`. Still asserts `assert_ne!(name_a, name_b)`.
- RAII env guard (`PermissionRulesEnvGuard`) added: restores `OPENCODE_RK_PERMISSION_RULES` on drop via `OnceLock<Mutex<()>>`.
- `resource_scoped_legacy_deny_blocks_current_intent` now calls `super::is_denied` directly instead of manually reconstructing checks.

## Verdict

**APPROVE** — all five functional axes verified via static source + test review.

## Verification

Source-level static review only. No compilation, test execution, or runtime inspection performed. All hashes verified via `sha256sum` (Python `hashlib.sha256`) against working-tree files at review time. Parent server suite results (if any) are reported separately and do not retroactively alter this static verdict.
