# Verifier scratchpad: PHASE1-SECURITY-MAP-VERIFY-RETRY

## Claim
- Task: PHASE1-SECURITY-MAP-VERIFY-RETRY
- Session: ses_f2de4205affep08hvKAIHnknAa
- Role: Independent security-acceptance map verifier
- Owned file: worklog/PHASE1-SECURITY-MAP-VERIFY-RETRY.md
- Candidate map: worklog/PHASE1-SECURITY-ACCEPTANCE-MAP.md at commit 89fa28bcfa718cc3505da448cdb4686fd261c5c5
- Product-spine source of record: origin/lane/PHASE1-product-spine-20260923 at 5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b

## Source evidence (product-spine 5d666830)

### P0: Shell broker bypass
- `crates/tools/src/executor.rs`: `execute()` routes `bash`/`shell` to `execute_shell()` which calls `Command::new("bash").arg("-c").arg(cmd)` with NO PermissionBroker invocation.
- Live callers confirmed via `git grep ToolExecutor`:
  - `crates/server/src/lib.rs:1572` -- production server runtime creates `ToolExecutor::new()` and calls execute.
  - `crates/tools/src/registry_dispatch.rs:171,273` -- dispatch layer passes executor to tool registry.
  - `crates/server/src/app_runtime.rs:687` -- references sandboxed ToolExecutor call.
- Safer path exists: `crates/tools/src/shell_tool.rs` uses `PermissionBroker`, `env_clear`, bounded output (MAX_OUTPUT_BYTES=10MiB), process-group kill via rustix, `kill_on_drop`.
- Map line references (96-122, 124-180) do not match product-spine line numbers (execute_shell starts ~line 240). Behavioral claim is correct despite line drift.
- Map characterization "caller wiring incomplete" UNDERSTATES the finding. The unbrokered path IS the active production caller. This is a correction.

### P0: OS isolation absent
- `crates/security/src/os_backend.rs`: `#![forbid(unsafe_code)]`, no syscall dependency.
- `platform_support()`: sets `enforcement_linked = false`, `available = landlock_detected && enforcement_linked` (always false).
- `engage()`: ALWAYS returns `Err(Blocked)` with detection evidence. Never pretends engagement.
- `require_supported()`: returns `Err(Blocked)` when `!sup.available`.
- `FsGrant`, `run_confined`, `restricted_spawn`: defense-in-depth process hygiene, NOT OS sandbox. Module docs explicitly state this.
- Map classification "honest fail-closed deviation" is CORRECT.

### P0: Credential backend absent
- `crates/providers/src/auth_store.rs:1-6`: header says "does not open a keyring, read or write a file, inspect the environment, use the network, or retain credential bytes."
- `crates/providers/src/local_credential_import.rs`: planning boundary only.
- `crates/providers/src/account_setup.rs`: stores `SecureStoreMarker`, not credentials.
- Map claim CORRECT.

### P0: TOCTOU exposure
- `crates/security/src/project_boundary.rs`: lexical normalization only.
- `crates/security/src/sandbox.rs`: resolves paths then checks prefixes. Symlink swap between check and operation is possible.
- Map claim CORRECT.

### P1: Approval/restart
- `crates/server/src/turn_service.rs`: `AwaitingApproval`, `Executing`, `Uncertain` states. Denies ambiguous replay. In-memory state machine.
- `crates/storage/src/facade.rs`: SQLite reopen persistence.
- `crates/storage/src/approvals_v2.rs`: bounded approval input, single-winner resolution, expiry sweep.
- Integrated journey (tool process + approval row + result + transcript + restart) not proven. Map claim CORRECT.

### P1: Daemon auth
- `crates/server/src/daemon_auth.rs`: 32 random bytes, 64 hex char restore, constant-time Bearer verify. /health public, /api/* gated.
- `crates/server/src/daemon.rs`: symlink metadata, size, owner, schema, live PID, loopback origin, token shape checks. 8 KiB cap. Atomic rename publication.
- Client send-side auth and concurrent duplicate prevention not integration-tested. Map claim CORRECT.

### P1: Secret-free diagnostics
- `crates/providers/src/debug_export.rs`: bounds record/byte output, redacts supplied secret_values, recursive replacement, truncation marker.
- Whole-path coverage (provider errors, process logs, transcripts, approval text) not proven. Map claim CORRECT.

## Convergence gate
- Map reports total=80. Task prompt reports total=81. Discrepancy is pre-existing coordination debt, not security acceptance.
- Gate blocked by off-plan completed claims and notes admitting "no acceptance". Not a security-map issue.

## Decisions
- Verdict: ACCEPT WITH CORRECTIONS
- Correction 1: P0 shell bypass is actively wired in production (server/src/lib.rs:1572, registry_dispatch.rs:171,273), not merely "incomplete caller wiring". Elevate severity language.
- Correction 2: Map line-number references for executor.rs are stale relative to product-spine 5d666830. Behavioral claims remain valid.
- Correction 3: Convergence gate count drifted from 80 to 81 between map audit and verification. Non-blocking.
- All 16 proposed lanes have valid one-file ownership. No dependency conflicts detected.
- Launch-now lanes: SEC-RED-OS, TOOL-RED-SHELL (test-only, no external deps).
- Queued lanes requiring decisions: SEC-OS (needs approved syscall backend), PROV-RED-STORE/PROV-STORE (needs backend dependency), APP-RED-APPROVAL (depends on TOOL-SHELL + PROV-STORE + SEC-OS).

## Remaining unknowns
- Approved syscall-capable backend for os_backend.rs is an integration authority decision.
- Real provider transport caller identity not resolved in planning modules.
- Installed CLI send-side auth trace requires convergence revision integration test.