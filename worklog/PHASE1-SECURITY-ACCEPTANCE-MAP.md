# Phase 1 security acceptance map

Task: `PHASE1-SECURITY-ACCEPTANCE-MAP`
Audit revision: `36090717e4333b23d9765193ea250000b1de4325`
Branch snapshot: `lane/PHASE1-integration-20260923`
Scope: security-relevant Phase 1 paths only. This artifact changes no product,
test, controller, verifier, policy, dependency, or acceptance file.

## Verdict

Unsigned Phase 1 is blocked. Current code has useful fail-closed policy,
bounded state, daemon bearer checks, and honest unsupported-sandbox receipts.
It does not yet prove a secure installed journey. The highest-risk gaps are:

1. `crates/security/src/os_backend.rs::engage` has no kernel enforcement
   backend. `require_supported` fails closed, so generated execution cannot be
   accepted as isolated.
2. `crates/tools/src/executor.rs::ToolExecutor::execute` dispatches `bash -c`
   without `PermissionBroker`. The live shell path can bypass the broker.
3. `crates/providers/src/auth_store.rs` and
   `crates/providers/src/local_credential_import.rs` are pure planning
   boundaries. No keyring open, credential source read, protected destination
   write, or atomic secret replacement is executed there.
4. Lexical or canonical path checks are separate from the filesystem operation.
   They do not close symlink or TOCTOU races for protected writes.
5. Daemon auth and restart state have unit/state-machine evidence, but no
   integrated installed journey proves client send-side auth, crash recovery,
   no duplicate work, and secret-free durable output together.

The convergence gate is also blocked by repository coordination state, not by
this audit: `python3 tools/convergence_gate.py` reports `total=80`, including
off-plan completed claims and completed notes that admit `no acceptance`. This
prevents a parent or release claim.

## Authority and classification

`PLAN.md:46-57` requires classification of implemented native behavior,
shared compatibility, partial or planned behavior, new requirements, and safer
bounded deviations. `PLAN.md:162-180` and `docs/TDD.md:21-75` require RED,
freeze, GREEN, and independent verification. `docs/SECURITY.md:36-56` rejects
regex or prompt isolation and requires actual platform enforcement plus no
side effects on denial. The classifications below are audit findings, not
acceptance decisions.

## Current surface ledger

| Surface | Current evidence | Classification | Acceptance state |
| --- | --- | --- | --- |
| Provider keyring and file storage | `crates/providers/src/auth_store.rs:1-6` says the module does not open a keyring or file, inspect the environment, or retain credential bytes. `:328-344` performs lexical destination validation only. `:357-382` builds plans and validates a borrowed secret. `:384-430` builds refresh/status projections; status reports `exists: false`. | Partial/planned caller-owned boundary. Not a live backend. | Blocked until a capability-scoped backend executes keyring or owner-only file storage, with atomic replacement and readback tests. |
| Local credential import | `crates/providers/src/local_credential_import.rs:1-6` says no file read, environment inspection, byte copy, or write. `:159-173` checks caller-supplied mode. `:175-226` parses bounded transient JSON and returns only kind. `:228-258` `plan_import` returns a redacted plan. | Partial/planned validation boundary. | Blocked until consent, source-open, metadata, schema, destination, and atomic protected-write operations run through the broker. |
| Account setup and secret lifetime | `crates/providers/src/account_setup.rs:1-8` says no I/O and caller-owned persistence/transport. `:137-190` keeps pending setup in memory. `:325-380` stores only `SecureStoreMarker`; `:381-446` bounds accounts, replaces one provider, and drops the marker on removal. | Implemented bounded in-memory lifecycle; secure storage is asserted, not provided. | Partial. Removal/cancel invariants need an actual backend and integrated credential journey. |
| Secret path policy | `crates/security/src/lib.rs:256-288` denies recognized secret paths and protects system paths, outside-root writes, and deletes. `:555-575` contains the secret-path classifier. | Implemented deterministic policy layer. | Unit evidence only. Must prove every real caller invokes it before I/O and cannot bypass it. |
| Project boundary | `crates/security/src/project_boundary.rs:36-84` normalizes paths lexically; `:48-64` checks root or approved external prefixes. | Implemented lexical policy helper, not filesystem authority. | Blocked for protected writes until operation-time race resistance is demonstrated. |
| Sandbox path helper | `crates/security/src/sandbox.rs:90-123` resolves then checks allowed and denied prefixes. `:125-163` canonicalizes existing prefixes and reconstructs missing tails. | Implemented policy helper with symlink-aware resolution. Not atomic authority. | Partial. A path swap after check can still redirect a later operation. |
| OS isolation | `crates/security/src/os_backend.rs:112-146` sets `enforcement_linked = false` and `available = landlock_detected && enforcement_linked`. `:149-181` makes `require_supported` and `engage` return `Blocked`. `:184-243` provides grant checks, not kernel isolation. | Honest fail-closed deviation. No sandbox claim. | Phase 1 blocker. Real backend must be linked and tested on each supported platform; unsupported platforms must remain denied. |
| Broker and human approval | `crates/security/src/lib.rs:61-72` defines `Allow`, `Deny`, and `RequireHuman`. `:206-240` evaluates policy and retains a bounded audit (`MAX_AUDIT_ENTRIES` at `:153`). `:256-300` protects secrets, destructive operations, outside-root writes, and shell `-c`. | Implemented broker policy, caller wiring incomplete. | Partial. Need integrated execution, one-shot grants, denial side-effect checks, and resume. |
| Grant scope/replay | `crates/security/src/app_policy.rs:125-181` binds a grant to digest, workspace, session, requester, expiry, and policy version. `:288-338` never lifts mandatory deny and consumes valid grants once. `:341-369` rejects remote use of local-only grants and always fails unsupported OS sandbox. | Implemented state/policy logic. | Unit/state evidence only. Integrated approval identity and execution caller remain unproven. |
| Shell/process execution | `crates/tools/src/executor.rs:96-122` routes `bash`/`shell` to `execute_shell`; `:124-180` calls `Command::new("bash").arg("-c").arg(cmd).output()` without a broker. `crates/tools/src/shell_tool.rs:178-237` has a separate argv path with optional broker, `env_clear`, bounded output, `kill_on_drop`, and process-group handling. | Active broker bypass in the generic executor; safer path exists separately. | Phase 1 blocker until all live callers use brokered argv execution and bounded cancellation. |
| File operation execution | `crates/tools/src/file_ops.rs:149-165` exposes `FileTool::execute_authorized`. `:173-200` authorizes only `Write`; `Read`, `List`, and `CreateDir` pass through to `execute`. `:202-217` performs the operation. | Partial caller protection; authorization coverage is operation-dependent. | Blocked for an end-to-end claim until every sensitive operation has the right intent and protected I/O boundary. |
| Daemon bearer auth | `crates/server/src/daemon_auth.rs:31-80` mints 32 random bytes, restores only 64 hex chars, and verifies `Bearer` constant-time. `:150-176` leaves `/health` public and gates `/api/*` with 401/403. | Implemented server middleware and token shape. | Partial integrated evidence. Must verify client sends the token and stale/forged descriptors cannot attach or spawn duplicates. |
| Descriptor authority | `crates/server/src/daemon.rs:376-416` checks symlink metadata, size, owner, schema, live PID, loopback origin, and token shape before reading. `:425-444` caps at 8 KiB and rejects symlinks. `:528-573` mints and atomically renames a descriptor. | Implemented bounded descriptor checks and publication. | Behavioral integrated verification required. Pre-read metadata and later read still need race testing. |
| Human approval and restart | `crates/server/src/turn_service.rs:65-126` defines `AwaitingApproval`, `Executing`, `Uncertain`, and retry classification. `:303-370` separates approval, denial, and clean tool finish. `:391-466` refuses ambiguous replay and marks restart uncertainty. | Implemented in-memory state machine. | Partial. Durable crash recovery, approval persistence, caller execution, and resume UI/API are not one proven journey. |
| Durable storage/reopen | `crates/storage/src/facade.rs:27-61` opens or initializes SQLite and creates a bounded blob root. `:135-143` marks clean shutdown and checkpoints WAL. `:268-288` tests reopen persistence. `crates/storage/src/approvals_v2.rs:18-145` bounds approval input, single-winner resolution, mandatory human identity, and expiry sweep to 500 rows. | Implemented storage primitives and approval transitions. | Partial integrated acceptance. Must prove uncertain turns and tool outcomes survive abnormal restart without replay. |
| Secret-free diagnostics | `crates/providers/src/debug_export.rs:63-107` bounds record/byte output and redacts supplied `secret_values`; `:160-180` recursively replaces matching strings; `:183-212` emits a bounded truncation marker. | Implemented bounded export redaction for supplied values. | Partial. No evidence that all provider, server, process, audit, and durable transcript paths centralize redaction or avoid retaining secret bytes. |

## Observable Phase 1 contract

Acceptance must exercise the installed product, disposable HOME, disposable
workspace, deterministic fixture provider, and real caller path. No real
credentials, user database, signing identity, or live OAuth grant may be used.

1. Bare installed `opencode2` starts or discovers exactly one authenticated
   daemon. A second client attaches; it does not create another owner or DB.
2. Missing credentials open in-app setup. Consent is explicit. Cancel, deny,
   malformed input, permissive source mode, backend failure, and destination
   race leave no credential file, keyring entry, transcript value, or account
   marker.
3. Credential bytes have one bounded owner at a time, cross no debug/display/
   error boundary, and are cleared from caller memory as far as the selected
   backend contract permits. Refresh uses temp-file plus fsync/atomic rename or
   an equivalent keyring transaction; interrupted replacement leaves old or new
   valid state, never a partial secret.
4. A fixture tool request passes through `PermissionBroker`. `.env`, SSH keys,
   cloud credentials, system writes, destructive operations, and wildcard
   permission remain denied. Denial proves no file, process, DB, or transcript
   side effect.
5. An ordinary human-gated operation has a grant bound to operation digest,
   scope, requester, policy version, expiry, and origin. Approval is single-use.
   Wrong digest, replay, expiry, stale policy, wrong requester, remote local-only
   use, and mandatory deny fail closed. Approval resume does not rerun a side
   effect whose outcome is uncertain.
6. Shell execution uses direct argv, broker authorization, cleared environment,
   fixed PATH, bounded stdout/stderr, timeout, process-group cleanup, and owned
   cancellation. Opaque `bash -c` is not reachable through an unbrokered path.
7. Supported OS isolation is exercised on the actual target: inherited file
   descriptors, environment secrets, symlink traversal, direct secret paths,
   and subprocess escapes are denied. Unsupported backend capability reports a
   typed block and starts no generated executable.
8. Tool outcome, approval transition, transcript, and session history persist
   durably. Clean restart resumes settled history. Crash or lost acknowledgement
   yields `Uncertain` and requires explicit reconciliation, never blind retry.
9. Daemon auth rejects missing and bad credentials before handler state changes;
   descriptor symlink, owner, size, origin, PID, schema, and malformed-token
   attacks cannot redirect the client. Token material never enters logs or
   transcripts.
10. Redacted bounded logs, debug exports, audit records, and transcripts contain
    neither fixture secret canaries nor provider credential values, including
    errors, timeout output, approval descriptions, and restart receipts.

## Failure, lifetime, persistence, and resource bounds

| Boundary | Failure state | Ownership/lifetime | Persistence | Required bound |
| --- | --- | --- | --- | --- |
| Credential input | consent/path/schema/permission/backend failure | Caller owns bytes; backend owns only during one operation | Atomic old/new credential record | `auth_store.rs:19-28` caps path/provider/secret; import caps 64 KiB at `local_credential_import.rs:17-20` |
| Broker decision | `Deny`, `RequireHuman`, expired/replay/stale grant | Broker owns bounded audit; grant is single-use | Approval record plus decision receipt | `MAX_AUDIT_ENTRIES=1024` at `security/src/lib.rs:153`; grant ledger cap in `app_policy.rs:245-285` |
| File/process action | no spawn or no I/O on deny; typed cancellation on timeout | Parent owns child and cancellation; no detached child | Tool outcome only after observed result | Shell argv path bounds output and timeout at `shell_tool.rs:178-237`; generic executor currently lacks equivalent cap |
| OS isolation | `Blocked` before generated execution when backend unavailable; typed violation on escape | Backend owns ruleset/process boundary | No ambiguous success receipt | `os_backend.rs:149-181` currently always blocks; real backend must prove inherited-capability close |
| Approval/restart | denied settles without execution; ambiguous result becomes `Uncertain` | Turn owns cancel token and bounded event log | Phase/outcome/receipt durable before resume | `MAX_TURN_EVENTS=64`, `MAX_TEXT_BYTES=8192` at `turn_service.rs:38-41`; approval sweep max 500 at `approvals_v2.rs:14-16` |
| Daemon descriptor | missing/stale/malformed/symlink/foreign owner rejected | Daemon owns token; client owns a short-lived copy | Atomic descriptor publication | 8 KiB descriptor cap at `daemon.rs:425-427`; 32-byte token at `daemon_auth.rs:31-34` |
| Transcript/export | redacted value or bounded truncation marker; no secret-bearing error | Export owns output only until return; caller owns records | Durable transcript must store redacted content only | Debug export budget supplied by caller at `debug_export.rs:68-107`; all other transcript writers need integrated proof |

## RED-first lane plan

Each row names exactly one owned file. Test-author lanes freeze a compiling,
failing fixture before the corresponding implementation lane. Shared module
registration, dependency acceptance, verifier configuration, and controller
state remain integration-owned and are not assigned here.

| Order | Lane | One owned file | RED assertion | Depends on | Failure/resource target |
| --- | --- | --- | --- | --- | --- |
| 1 | SEC-RED-OS | `crates/security/tests/phase1_os_isolation.rs` | Actual target probe proves unsupported backend blocks; supported backend closes inherited FD/env and denies symlink/secret escape with no marker side effect. | None; platform matrix first | No generated child on unsupported target; fixture bytes and FD count bounded; platform result recorded, never mocked. |
| 2 | SEC-OS | `crates/security/src/os_backend.rs` | Make SEC-RED-OS green using an approved syscall-capable backend and fail closed on unsupported targets. | SEC-RED-OS; integrator-approved dependency | One owned child per probe, timeout, kill/reap, bounded grants; no claim from pure path checks. |
| 3 | SEC-RED-TOCTOU | `crates/security/tests/phase1_atomic_paths.rs` | Swap directory/symlink between authorization and operation; protected destination remains protected and denied write has no side effect. | SEC-OS policy boundary | Disposable temp root only; bounded path/depth; repeated race attempts have deterministic receipt. |
| 4 | SEC-TOCTOU | `crates/security/src/sandbox.rs` | Implement operation-time descriptor-relative or equivalent safe authority, including create/missing-tail and owner-only replacement. | SEC-RED-TOCTOU; approved platform primitive | No lexical-only acceptance; no broad root grant; bounded path components. |
| 5 | TOOL-RED-SHELL | `crates/tools/tests/phase1_shell_broker.rs` | Opaque shell, secret env, destructive argv, timeout, denial, and cancellation produce no unauthorized side effect; direct argv allowed only by broker. | SEC-OS and policy contract | Process count, output bytes, timeout, env, and child lifetime measured. |
| 6 | TOOL-SHELL | `crates/tools/src/executor.rs` | Remove unbrokered `bash -c`; route live calls through brokered argv/shell tool and bounded cleanup. | TOOL-RED-SHELL; caller map | No shell-string concatenation; no detached process; output and batch admission bounded. |
| 7 | PROV-RED-STORE | `crates/providers/tests/phase1_auth_storage.rs` | Real disposable keyring/file fixture stores, reads, refreshes atomically, rejects unsafe path/mode, and leaves no partial secret after interruption. | SEC-TOCTOU; capability API | Secret max 64 KiB; one operation owner; file mode 0600; no host keyring or user DB. |
| 8 | PROV-STORE | `crates/providers/src/auth_store.rs` | Replace plan-only return with capability-scoped backend execution and explicit typed unavailable/failure states. | PROV-RED-STORE; approved backend dependency | No ambient env/network; no secret in errors/logs; atomic temp cleanup. |
| 9 | PROV-RED-IMPORT | `crates/providers/tests/phase1_credential_import.rs` | Consent, source metadata, bounded schema, brokered read, and protected destination write work; cancel/deny/race leaves no bytes. | PROV-STORE and SEC-TOCTOU | 64 KiB input, owner-only source, fixed provider schema, no source path leakage. |
| 10 | PROV-IMPORT | `crates/providers/src/local_credential_import.rs` | Wire the caller-owned plan to the approved capability executor without ambient file/env access. | PROV-RED-IMPORT | Secret lifetime ends after backend handoff; ambiguous import never auto-retries. |
| 11 | APP-RED-APPROVAL | `crates/server/tests/phase1_approval_restart.rs` | Installed fixture journey covers deny, one-shot grant, resume, interruption, crash/lost ack, `Uncertain`, durable reopen, and no duplicate effect. | TOOL-SHELL, PROV-STORE, SEC-OS | One active fixture turn, bounded event/output/history, explicit reconciliation required. |
| 12 | APP-APPROVAL | `crates/server/src/turn_service.rs` | Persist/restore lifecycle and bind approval records to real execution outcomes; preserve no-blind-replay semantics. | APP-RED-APPROVAL; storage integration owner | Durable transition ordering; no `Settled` before observed effect; restart cancellation joins children. |
| 13 | DAEMON-RED-AUTH | `crates/server/tests/phase1_daemon_auth.rs` | Fresh HOME, concurrent start, second client, missing/bad bearer, forged descriptor, restart rotation, and no state mutation on rejection. | Existing daemon middleware; installed entrypoint | 8 KiB descriptor, 64-char token, one owner/DB, bounded retry. |
| 14 | DAEMON-AUTH | `crates/server/src/daemon.rs` | Close remaining descriptor publication/read race and integrate authenticated client/server lifecycle. | DAEMON-RED-AUTH; CLI integration owner | No unlink of a foreign socket/descriptor; no token log; no duplicate daemon. |
| 15 | LOG-RED-SECRET | `crates/providers/tests/phase1_secret_free_outputs.rs` | Inject canaries into provider errors, tool output, approvals, transcripts, audit, restart receipts, JSONL, and HTML; assert absence and truncation bounds. | All execution lanes; fixture only | Fixed canary values, bounded output, no production credential. |
| 16 | LOG-SECRET | `crates/providers/src/debug_export.rs` | Extend redaction at every owned export boundary and prove callers pass the secret set or use typed redacted payloads. | LOG-RED-SECRET; caller inventory | No unbounded secret list/output; HTML escaped; marker fits budget. |

The implementation lanes above are proposals, not claims that the files are
ready to edit. Each requires an independent test-author RED receipt, frozen
hash, verifier run, and integrated rerun. Lane order is intentionally
security-spine first: no provider or installed-journey acceptance can rely on a
policy-only sandbox or an unbrokered executor.

## Unsigned Phase 1 blocker register

### P0: real OS isolation missing

Evidence: `crates/security/src/os_backend.rs:112-181` explicitly reports no
linked enforcement and always returns `Blocked`; `docs/SECURITY.md:36-56`
requires actual platform enforcement and inherited-capability closure. Required
receipt: target-platform child probe showing denied secret path, symlink,
inherited FD, environment, and subprocess escape with no side effect. Until
then, generated executable execution remains denied and Phase 1 unsigned.

### P0: generic shell broker bypass

Evidence: `crates/tools/src/executor.rs:105-130` accepts a tool name and invokes
`bash -c` directly. Broker policy recognizes shell `-c` as human-gated at
`crates/security/src/lib.rs:290-300`, but this caller does not invoke it.
`crates/tools/src/shell_tool.rs:189-211` demonstrates the required broker gate,
but a separate safe implementation does not prove every caller uses it.
Required receipt: denied shell produces no marker, no child, no retained secret
output; approved direct argv honors timeout, env, output, cancellation, and
process-group cleanup.

### P0: credential backend and import execution absent

Evidence: `auth_store.rs:1-6` and `local_credential_import.rs:1-6` explicitly
exclude I/O. `account_setup.rs:325-328` stores a marker, not a credential.
Required receipt: disposable capability backend, consent-bound import, atomic
refresh, owner-only mode, bounded secret lifetime, and no secret-bearing
diagnostic. Live provider credentials, OAuth consent, keyring availability, and
signing identities cannot be fabricated by this audit.

### P0: protected-file TOCTOU not closed

Evidence: `project_boundary.rs:45-84` is lexical; `sandbox.rs:90-163` resolves
and checks before the later operation. A symlink or directory replacement can
change the object between those steps. Required receipt: race fixture using
operation-time authority and no outside-root side effect.

### P1: integrated approval/restart journey missing

Evidence: `turn_service.rs:423-466` correctly denies ambiguous retry and marks
`Uncertain`, while `facade.rs:268-288` proves ordinary reopen persistence.
Neither proves that the live tool process, approval row, result, transcript,
and restart path are durably joined. Required receipt: crash/lost-ack fixture,
explicit reconciliation, no blind replay, second-client observation.

### P1: daemon auth integration and descriptor race evidence incomplete

Evidence: `daemon_auth.rs:150-176` and `daemon.rs:376-444` implement strong
server-side checks and bounded descriptor validation. Current evidence does not
itself establish the installed CLI sends `Authorization`, handles stale
credentials, or prevents concurrent duplicate starts on the exact installed
journey. Required receipt: fresh-HOME concurrent launch and restart test.

### P1: secret-free whole-path evidence incomplete

Evidence: `debug_export.rs:63-107,160-212` redacts supplied values and bounds
exports. It does not prove provider/network/process logs, durable transcript
writers, approval text, or every error path carry only redacted values. Required
receipt: canary injection across all output classes, exact byte/record caps,
zero canary hits.

## External and platform evidence disposition

No keyring service, live provider account, OAuth grant, code-signing identity,
notarization service, remote deployment, or supported-platform kernel backend was
used or claimed. The current OS backend reports its unsupported state honestly;
that is a security result, not acceptance. Platform-specific RED/GREEN must run
on the actual target with disposable fixtures. Repository unit tests, static
policy checks, and a passing module compile cannot substitute for those receipts.

## Verification record

- `python3 tools/convergence_gate.py` ran on this revision and failed closed:
  `CONVERGENCE BLOCKED`, `total=80`. Findings include off-plan completed claims,
  `AUD-017` and `AUD-020` notes admitting `no acceptance`, and other ledger
  reconciliation failures. This is preserved as a blocker, not bypassed.
- Source evidence was re-read from the cited paths at the audit revision.
- No frozen test, product source, controller, verifier, policy, dependency, or
  user database was changed for this artifact.
- Final validation for this file: `git diff --check` must pass. The ledger note
  below records no acceptance claim and the required worklog completion only.

## Remaining unknowns

1. Approved syscall-capable backend and supported OS list are integration
   authority decisions; this lane cannot add a dependency or claim isolation.
2. The real provider transport caller and credential capability owner are not
   identified in the current planning modules.
3. Exact installed CLI send-side auth and native TUI tool caller need an
   integrated trace on the convergence revision.
4. Durable schema mapping for approval, turn uncertainty, tool outcome, and
   redacted transcript needs an integrated owner and frozen journey.
5. Independent verifier must reconcile the ledger and rerun all frozen tests on
   the exact integrated revision before any Phase 1 or parent acceptance.
