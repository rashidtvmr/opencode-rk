# LANE-TOOL-PERM scratchpad

- Claim: LANE-TOOL-PERM, session ses_f423cd0b1ffevAoq08DEYzAc6W, status in-progress.
- Source evidence:
  - `crates/tools/src/permission.rs:1` stub (`//! Tool permission module stub.`), exported `crates/tools/src/lib.rs:37` (`pub mod permission;`).
  - `crates/tools/src/ext_perms.rs:33` `grant_perm` (fail-closed shape, idempotent dup, cap 256).
  - `crates/tools/src/ext_secure.rs:27` `grant_all` (length-bound first, empty-name fail, cap 64).
  - `crates/tools/src/tool_allow.rs:22,39` `allow_tool` (empty/full reject, idempotent) + `is_allowed` (exact match — `*` never expands).
  - `crates/tools/src/app_extensions.rs:463,492` `grant_input_ok`/`Authority::request` (human./system./authority./grant. never grantable via input; whole-batch reject).
  - `crates/security/src/lib.rs:154,205` `PermissionBroker::authorize` (baseline-first; mandatory Deny/RequireHuman survive `PermissionSet::star`); `crates/security/src/tool_authorize.rs:188` `run_if_allowed` (effect only on Allow).
  - `crates/tools/src/mcp_spawn.rs:379,386` broker verdict mapping (`RequireHuman` → denied spawn, never bypassed).
- Observed scenario: stub exports no checks; dispatch path (`registry_dispatch.rs:66`) takes injected policy but tools crate has no composed allowlist+grant+broker gate.
- Target boundary: own ONLY `crates/tools/src/permission.rs`. No lib.rs, no test edits, no other lanes.
- Tests: authored in-file `#[cfg(test)]` (module owns its contract; no frozen tests exist for `permission` filter yet):
  - t01 allowlisted ok; t02 unknown-tool deny zero side-effect; t03 ungranted ext deny;
  - t04 `*` never bypasses (allowlist exact-match + broker star still Deny/HumanGate);
  - t05 secure-gate (`secure.<perm>` needs granted SecurePerm; human gate still holds).
- Decisions: deny-unless-authorized check order shape→allowlist→ext grant→secure gate→broker; `*` literal-only; `run_if_authorized` mirrors `run_if_allowed`.
- Remaining: implement, run VERIFY cmd, ledger completed, commit+push lane files only.
