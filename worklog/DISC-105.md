# DISC-105 scratchpad — Broker authorization wired into executor/server tool paths

Claim: `tools/completion_claims.py` claim DISC-105 / ses_orch_DISC105 / worklog/DISC-105.md — OK (in-progress).
Owned file: `crates/security/src/tool_authorize.rs` ONLY (+ this scratchpad + own ledger row). No commit/push per lane orders.

## Source evidence (repo HEAD 7b2ba00; audit pin b0bed89)
- `crates/security/src/lib.rs:153-225` — `PermissionBroker::authorize` baseline-first: mandatory `Deny`/`RequireHuman` from `decide` never lifted by `PermissionSet` (incl `*`); allow-path only narrowed. Audit bounded `MAX_AUDIT_ENTRIES=1024` (`:151,235-237`).
- `crates/security/src/lib.rs:288-299` — `authorize_process`: `bash -c`/`cmd /c` → `RequireHuman`; destructive argv → `Deny`.
- `crates/security/src/app_policy.rs:291-339` — `decide()`: grant satisfies ONLY `RequireHuman`, with digest/scope/freshness/version/single-use checks; `Deny` never lifted; stale version → `Deny`, replay → `Deny`, ledger never burned on failed check.
- Repair target RC-AUD006-01 (`sources/completion/audits/AUD-006.json`): broker unwired — `crates/tools/src/executor.rs:130` spawns `Command::new("bash").arg("-c")` with no broker; `crates/server/src/web_artifact.rs:400` `authorize_run` is a local bool permit, not the broker.
- Boundary problem: `lib.rs` does NOT declare `mod tool_authorize`. Integrator must add `pub mod tool_authorize;` to `crates/security/src/lib.rs` (shared file, not mine). For RED/GREEN runs I TEMP-add that one line, run scoped tests, then REVERT so final diff touches only owned file + scratchpad + ledger.

## Target boundary (owned file only)
`tool_authorize.rs`: pure argv-only wiring — intent constructors, `ToolAuthorizer` (broker + `GrantLedger` + bounded REDACTED audit), `run_if_allowed` (denial runs zero effects by construction), `redact_secrets`. No spawn, no FS, no threads, no clock (time via `ExpectedScope.now`), `forbid(unsafe_code)`.

## Tests (frozen after RED)
In-file `#[cfg(test)] mod tests`, 6 tests mapping card T01..T05:
1. `shell_string_requires_human_and_denied_runs_nothing` (T01)
2. `server_tool_path_denial_has_no_side_effects` (T02)
3. `star_cannot_lift_mandatory_across_versions` (T03)
4. `stale_approval_rejected_without_side_effects` (T04)
5. `denied_calls_audited_with_secrets_redacted` + flood bound (T05)
6. `redactor_unit` (support)

## Log
- RED file written (stubs: always-Allow `run_if_allowed`, empty fingerprint, passthrough redact).
- RED run (temp `pub mod tool_authorize;` in lib.rs, reverted after): 1 passed / 5 failed as required — `run_if_allowed` ran effects on Deny (`Some(())` vs `None`), redactor leaked `sk-secret-value`. Compiling RED confirmed. RED sha256: `473cbdf55fd5dafacae9064d2fa81bfc3021feed540968dd158660279970e28c`.
- GREEN: implemented `run_if_allowed` gate-match, digest-bound fingerprint text, stdlib `redact_secrets` (KEY=VALUE scrub, Bearer-next-token, loose `sk-*`/`ghp_*`/`AKIA*` tokens), removed dead test helper, rustfmt clean.
- GREEN run (same temp mod line, reverted after): `tool_authorize`: 6 passed 0 failed. Full `--lib`: 143 passed 0 failed (137 pre-existing + 6 new). `lib.rs` restored byte-identical (`git diff --stat` clean for it; `cp /tmp/opencode/lib.rs.bak` verified).
- FROZEN test hash (post-rustfmt, final): sha256 `0e096292d8de7cf29d28208245751c2e0f44a684355dd7cd35c1b7cd6edac2e4`, 547 lines.
- KNOWN INTEGRATION GAP (for orchestrator/integrator, NOT fixable in this lane): `crates/security/src/lib.rs` does not declare `mod tool_authorize`, so the module is currently unwired until the integrator adds `pub mod tool_authorize;`. Filed here rather than editing a shared file outside my owned path.
