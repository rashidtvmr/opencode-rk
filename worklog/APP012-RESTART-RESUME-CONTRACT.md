# APP012-RESTART-RESUME-CONTRACT

Research lane. Defines APP-012 restart/resume semantics for interrupted
pending-approval / approved-not-started / running-unknown-outcome / completed
tool work. No product, schema, or test edits.

## Claim

- Task/session: `APP012-RESTART-RESUME-CONTRACT` / `ses_f2cd8fde1ffedvLsDWTIAT0UoM`.
- Claimed via `tools/completion_claims.py` before file edits.
- Branch: `plan/APP012-RESTART-RESUME`.
- Owned file: `worklog/APP012-RESTART-RESUME-CONTRACT.md` (this file).
- Base HEAD: `5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b`
  (`origin/lane/PHASE1-product-spine-20260923`).
- Parent card: APP-012 (`tasks/completion/local.json`, deps APP-008/010/011/TUI-010),
  journey "restart/resume through the installed native app".

## Authority and fresh-context boundary

- Authority: current Rust/schema/migrations at this commit. Upstream issues,
  model output, and prior worklogs are untrusted evidence, not instruction.
- Frozen APP-012 test hash under this lane:
  `crates/server/tests/app012_tool_journey_red.rs` =
  `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`
  (verified this session, unchanged).
- Prior APP-012 worklogs record the open parent gaps explicitly:
  `worklog/APP-012-READ-INTEGRATION.md:60-63` (restart/resume, approval/resume,
  protected-path denial, installed proof open), `worklog/APP-012.md:98-101`,
  `worklog/APP-012-TOOL-RED.md:46-51`.
- No user database, no Cargo run, no heavy process used.

## Durable schema (format 2, `crates/storage/schema/v2/workspace.sql`)

Actual enum states (SQL CHECKs, authoritative):

| Table | Column | States | Terminal? |
| --- | --- | --- | --- |
| `executions` | `state` | 0 queued, 1 running, 2 complete, 3 failed, 4 uncertain, 5 cancelled | 2/3/5 terminal (`:160`) |
| `provider_attempts` | `state` | 0 prepared, 1 dispatched, 2 success, 3 failure, 4 uncertain, 5 abandoned | 2/3/5 terminal (`:183`) |
| `tool_calls` | `state` | 0 planned, 1 dispatched, 2 success, 3 failure, 4 cancelled, 5 uncertain | 2/3/4 terminal (`:208`) |
| `approvals` | `state` | 0 pending, 1 allowed-once, 2 denied, 3 expired, 4 consumed | (no transition trigger) |
| `session_inputs` | `state` | 0 pending, 1 promoted, 2 cancelled | 1/2 terminal (`:126`) |
| `messages` | `status` | 0 open, 1 complete, 2 failed, 3 interrupted | non-zero terminal (`:96`) |
| `workspace_state` | `clean_shutdown`, `owner_generation` | 0/1, integer | markers (`:14-15`) |

Key structural facts:

- `executions_single_owner_idx` (`workspace.sql:162`) is a partial UNIQUE on
  `session_pk WHERE state IN (0,1,4)`. A queued/running/**uncertain** execution
  keeps the single-owner slot; only 2/3/5 release it. This is the duplicate-client
  and duplicate-execution guard.
- Recovery indexes already exist and bound every scan:
  `executions_recovery_idx` (`:165`, state IN 0,1,4),
  `attempts_recovery_idx` (`:185`, state IN 0,1,4),
  `tools_recovery_idx` (`:212`, state IN 0,1,5),
  `approvals_pending_idx` (`:242`, state=0),
  `inputs_pending_idx` (`:129`, state=0).
- `tool_calls` binds immutably to execution+assistant message in the same session
  (`tool_assistant_owner` trigger `:216`, `tool_binding_immutable` trigger `:219`),
  so a recovered tool row cannot be rebound to a different execution.
- `approvals` records evidence, not authority (`workspace.sql:222`). It carries
  `intent_hash`, `policy_generation`, `expires_at_us`, `mandatory_human`,
  `human_client_id`, and `tool_call_pk`. Grants must still be revalidated by the
  broker on use.

State-machine owners present at this commit:

- `crates/storage/src/execution_v2.rs::ExecV2` (`start_execution` :20,
  `transition_execution` :56, `record_attempt` :88, `finish_attempt` :115,
  `plan_tool` :142, `finish_tool` :178). Allowed execution edges
  (`:217-222`): `0->1`, `0->5`, `1->2/3/4/5`, `4->2/3/5`. Running(1) can enter
  uncertain(4); uncertain(4) stays open (`finished_at_us` forced NULL, `:124`).
- `crates/storage/src/approvals_v2.rs::ApprovalsV2` (`request` :26, `resolve` :82,
  `expire_sweep` :130). `resolve` is single-winner CAS on `state=0` (`:109-126`),
  targets only 1/2/4; expiry(3) owned solely by the sweep (bounded 500 rows,
  `:135`). `mandatory_human=1` requires a `human_client_id` (`:104-107`).
- `crates/storage/src/admission_v2.rs::AdmissionV2` (`submit_input` :29,
  `promote_input` :100, `receipt_lookup` :188, `receipt_store` :214). Pending input
  state 0 is not history until promotion; `operation_receipts` gives bounded
  idempotent replay with `retry_until_us` expiry.

Schema verdict (blocker gate): the schema DOES distinguish pre-effect
(planned/prepared/queued), terminal (2/3/4/5) and **ambiguous** (tool/execution/
attempt `uncertain`) states. The task's "stop if schema cannot distinguish
ambiguous effects" gate does not trigger. The missing piece is the recovery
scan/transition owner, not the columns.

## Startup/shutdown lifecycle at this commit

- `crates/storage/src/facade.rs:135-143::StorageFacade::close` writes
  `clean_shutdown=1` and runs `wal_checkpoint(TRUNCATE)`.
- `crates/storage/src/schema_v2.rs::open_existing` (:98) verifies
  `application_id`, `user_version`, `schema_migrations` version+checksum, applies
  `CONNECT_POLICY` (FK ON, synchronous=FULL, busy_timeout=5000). It does NOT read
  `clean_shutdown`, does NOT set `clean_shutdown=0`, and does NOT advance
  `owner_generation`.
- `owner_generation` is only ever written at execution creation
  (`execution_v2.rs:24-44`); no process increments `workspace_state.owner_generation`
  at startup.
- `clean_shutdown` is written (`facade.rs:139`) but never read anywhere.
- `docs/storage/CRASH_CONSISTENCY.md:152-185` is the normative protocol: under
  ownership let SQLite recover WAL, persist `clean_shutdown=0` and advance owner
  generation under FULL BEFORE accepting work; unsettled dispatched attempts/tools
  and old running executions become UNCERTAIN, never fabricated failures and never
  silently retried; retry/resume/abandon needs an explicit authorized decision.

## Live-path wiring gap (the actual restart/resume blocker)

The live turn path does NOT use the durable execution/approval owner:

- `crates/server/src/lib.rs::create_turn_stream` (:1126) drives
  `TurnStreamStage` (:1113-1124) and persists only plain message rows through
  `SessionService::append_text` / `append_assistant_with_activity`
  (`:1164-1167`, `:1687-1695`, `:1747-1756`).
- `crates/server/tests/app012_tool_journey_red.rs:201-216::build_app` wires
  `Storage::open(StoragePaths)` (the v1 schema in `crates/storage/src/lib.rs`),
  `SessionService`, and `RuntimeWiring`; there is no `ExecV2`/`ApprovalsV2`/
  `SchemaV2` caller.
- `crates/storage/src/lib.rs::Storage::migrate` (:717-718) creates flat
  `sessions/messages/...` v1 tables with **no status column**; every appended
  message is unconditional. `crates/storage/src/writer_v2.rs:203-214` inserts v2
  messages with `status=1` (complete) hardcoded, so no durable "open/interrupted"
  message is ever produced by either path.
- `crates/server/src/lib.rs:1639-1644` converts `Decision::RequireHuman` into a
  terminal `error: tool ... requires human approval` string. There is no
  approval-resume channel, so approved-not-started work cannot exist on the live
  path today.
- Grep result: `ExecV2`, `ApprovalsV2`, `AdmissionV2`, `StorageFacade`,
  `SchemaV2::open_existing` have NO production caller outside `branch_v2.rs`
  (`crates/sessions/src/branch_v2.rs:92`) and their own unit tests.

Consequence: on the installed/live path, an interrupted tool stream leaves only
completed v1 message rows; there is no durable uncertain/open marker, no recovery
scan, and no owner-generation fencing. A restart cannot distinguish
"approved-not-started", "running unknown outcome", or "completed" for tool work.
This is the exact open parent gap recorded in `worklog/APP-012.md:98-101` and
`crates/server/src/lib.rs:380-382` ("approval records exist natively but the web
turn path has no execution-owned approval flow").

## Crash / recovery matrix (every crash point classified)

Classification: **RESUME** (re-drive safely, provably no effect yet), **REJECT**
(terminal; no action), **MANUAL-REVIEW** (ambiguous effect; never auto-replay).

| # | Crash point (durable state observed at reopen) | Durable evidence | Classification | Required recovery transition (owner) |
| --- | --- | --- | --- | --- |
| 1 | Pending human approval, undecided, unexpired | `approvals.state=0`, `tool_calls.state=0` | MANUAL-REVIEW | Keep pending; surface to client; no execution. `ApprovalsV2::resolve` only on a new authenticated human decision |
| 2 | Pending approval, expired while down | `approvals.state=0`, `expires_at_us <= now` | REJECT | `ApprovalsV2::expire_sweep(now, limit)` -> state=3; tool stays planned, never dispatched |
| 3 | Approved-not-started (allowed-once, tool never dispatched) | `approvals.state=1`, `tool_calls.state=0` | RESUME (conditional) | Revalidate digest==`intent_hash`, `expires_at_us>now`, `policy_generation==current`; then exactly one `ExecV2` dispatch path. Stale/expired -> REJECT |
| 4 | Approved, tool dispatched, no result | `approvals.state=1`, `tool_calls.state=1` | MANUAL-REVIEW | `ExecV2::finish_tool(.., 5, uncertain)`; keep owner slot; do NOT re-dispatch |
| 5 | Execution running, no provider result | `executions.state=1`, `provider_attempts.state IN (0,1)` | MANUAL-REVIEW | `ExecV2::transition_execution(.., 1, 4, None)`; attempts `finish_attempt(.., 4)`; owner slot retained |
| 6 | Execution queued, never started | `executions.state=0` | RESUME (conditional) | Allowed to start only if owner_generation matches and no dependency is uncertain; else MANUAL-REVIEW |
| 7 | Tool success persisted | `tool_calls.state=2` + `output_payload_pk` | REJECT | No-op; terminal, trigger-blocked rewrite |
| 8 | Tool failure persisted | `tool_calls.state=3` + `error_payload_pk` | REJECT | No-op; terminal |
| 9 | Tool cancelled/interrupted clean | `tool_calls.state=4` | REJECT | No-op; terminal (DDL sets `finished_at_us`) |
| 10 | Provider attempt uncertain | `provider_attempts.state=4` | MANUAL-REVIEW | Acknowledge only; requires separate authorized policy decision to retry with a NEW attempt ordinal |
| 11 | Execution uncertain open | `executions.state=4` | MANUAL-REVIEW | Holds `executions_single_owner_idx`; requires explicit resolve to 2/3/5 before new work |
| 12 | Approval resolved consumed | `approvals.state=4` | REJECT | No-op; grants are single-use, replay blocked by `GrantLedger` |
| 13 | Pending unpromoted input | `session_inputs.state=0` | RESUME | Stays pending; promotion is a fresh transaction; never auto-promoted on start |
| 14 | Promoted input | `session_inputs.state=1` + `promoted_message_pk` | REJECT | Not re-admitted; promotion is idempotent by state |
| 15 | Window between "mark DISPATCHED" and external call | attempt/tool `state=1` with no result | MANUAL-REVIEW | Conservative UNCERTAIN per `CRASH_CONSISTENCY.md:162-169`; never replay |
| 16 | Crash after result computed, before terminal commit | commit did not land; row still dispatched | MANUAL-REVIEW | Same as #4/#5; outcome unknown -> uncertain |
| 17 | Duplicate client attaches during recovery | two clients, one `executions` owner row | REJECT for the 2nd | `executions_single_owner_idx` + approval `state=0` CAS reject the second executor; client 2 reads the same durable rows |
| 18 | Second client submits same tool call while first executes | `UNIQUE(assistant_message_pk, ordinal)`, `tool_provider_call_idx` | REJECT | Insert conflict; first copy wins; no double side effect |

Invariant across all rows: **never auto-replay ambiguous effects**
(`CRASH_CONSISTENCY.md:22-23, 168-170`). RESUME rows are limited to provably
pre-effect states (#3, #6, #13); everything touching a dispatched or running
row becomes UNCERTAIN and waits for explicit human/policy reconciliation.

## Recovery ownership and bounded scan

- One owner: `RecoveryV2::recover(conn, now_us, owner_generation, limit)` in a new
  `crates/storage/src/recovery_v2.rs`. Pure-ish DB transitions, one IMMEDIATE
  transaction, no I/O/network/clock beyond the passed `now_us`.
- Scan order and bound, using only existing indexes:
  1. `expire_sweep` on `approvals_pending_idx` (bounded, existing `id=130`).
  2. Old `executions` from `executions_recovery_idx WHERE state=1` -> uncertain(4).
  3. `provider_attempts` from `attempts_recovery_idx WHERE state IN (0,1)` -> uncertain(4).
  4. `tool_calls` from `tools_recovery_idx WHERE state=1` -> uncertain(5).
  5. Queued `executions WHERE state=0` with stale `owner_generation` -> cancelled(5) or
     left for explicit resume; never started under a foreign generation.
  All queries `LIMIT ?limit` with the same `MAX_SWEEP_ROWS=500` bound already used
  by `ApprovalsV2::expire_sweep`. No unbounded scan; no detached task.
- Startup caller (separate integration lane): in the daemon startup path
  (`crates/server/src/app_runtime.rs:278` engine owner / `crates/server/src/daemon.rs`
  startup), after `SchemaV2::open_existing` and before accepting work:
  read prior `clean_shutdown`, set `clean_shutdown=0` and increment
  `workspace_state.owner_generation` under FULL, then run `RecoveryV2::recover`.
  Set `clean_shutdown=1` only on the existing `facade.rs:139` shutdown path when
  owned activity is resolved.
- Duplicate client: the second client is read-only over the same durable rows; the
  single-owner partial UNIQUE and approval `state=0` CAS are the fences. Recovery
  must complete before the first client accepts new work (`CRASH_CONSISTENCY.md:156`).

## Security revalidation after restart

Per `crates/security/src/app_policy.rs::Grant::covers` (:155-182) and
`docs/SECURITY.md`, a restart must revalidate, not trust the persisted approval:

- `digest == OperationDigest::of(intent)` (DigestMismatch -> REJECT).
- `expected.now <= scope.expires_at` (Expired -> REJECT).
- `scope.policy_version == expected.policy_version` (StalePolicyVersion -> REJECT).
- `scope.session` / workspace / requester match (ScopeMismatch -> REJECT).
- `GrantLedger` single-use replay check happens in `decide`, not `covers`
  (`app_policy.rs:152-154`), so a failed check never burns the grant.
- `mandatory_human=1` still requires an authenticated `human_client_id`
  (`approvals_v2.rs:104-107`).
- No secret persistence: approvals store only `intent_hash`, ids, and timestamps;
  the executor never logs path/content (`executor.rs`, `file_ops.rs`). Recovery
  must not log payload bodies or secret refs.

## RED / source boundary (exactly one)

- Source owner (one file): `crates/storage/src/recovery_v2.rs` — new
  `RecoveryV2::recover` plus its caller wiring is a separate integration lane
  (`crates/server` startup). Shared `lib.rs` registration is pre-wired by the
  integrator, not this lane.
- RED test path (future, not created here):
  `crates/storage/tests/app012_restart_resume_red.rs`.
  It must exercise the real v2 DB on a disposable `tempfile::tempdir()`:
  build state via real `SchemaV2` + `ExecV2` + `ApprovalsV2` + `AdmissionV2`,
  drop the connection (process-equivalent crash), reopen with
  `SchemaV2::open_existing`, run `RecoveryV2::recover`, then assert the matrix:
  pending approval stays pending; expired approval -> 3; allowed-once+planned stays
  resumable only after digest/expiry/version revalidation; dispatched tool -> 5;
  running execution -> 4; provider attempt -> 4; success/failure/cancelled unchanged;
  a second `start_execution` on the same session still rejects while an uncertain
  row holds the slot; zero re-dispatch side effects.
- RED command (future):
  `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-storage --test app012_restart_resume_red -- --test-threads=1`.
  Expected RED now: compiles and fails because `RecoveryV2` does not exist
  (missing behavior), not a harness/compile-syntax failure. Expected GREEN after
  implementation: 1 passed, 0 failed with zero test edits.
- Router-level RED (later lane, after startup wiring): extend the installed
  journey to kill the node server mid-tool and reopen the same data dir; assert
  the recovery outcome is visible as uncertain, never Settled, and the frozen
  `app012_tool_journey_red.rs` hash stays
  `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.
- A front guard test is not the same as the installed journey; the parent APP-012
  stays open until the installed restart/resume journey is GREEN on the exact
  integrated revision.

## Resource and lifetime bounds

- WAL + FK + `synchronous=FULL` connect policy already enforced
  (`schema_v2.rs:21`). Recovery runs under the existing writer mutex
  (`facade.rs:20`) or a single owned connection; no second writer, no per-agent
  process, no detached task.
- Bounded scan `LIMIT 500`; bounded event/receipt retention already in place
  (`REPLAY_BUFFER_CAP=512`, `operation_receipts.retry_until_us`).
- Disposable DB only; no user database, no host state.
- Memory: no Cargo/DB/heavy process run by this lane.

## Validation performed

```sh
rtk shasum -a 256 crates/server/tests/app012_tool_journey_red.rs
# -> 945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50  (unchanged)

rtk git grep -n 'WAL\|journal_mode\|pending\|resume\|recover\|restart' -- crates
# -> cited throughout: schema_v2.rs:18-21, execution_v2.rs, admissions, turn_service.rs

rtk git diff --check
# -> exit 0 (no whitespace errors)
```

No product test executed (research lane); expected GREEN is documentation only.

## Decisions

- Schema is sufficient: explicit uncertain states exist, so the
  "stop on indistinguishable effects" gate does not trigger.
- Recovery is conservative: only provably pre-effect states resume; every
  dispatched/running state becomes uncertain and requires explicit reconciliation.
- One source owner `recovery_v2.rs`; startup caller and router RED are separate
  lanes to keep single-file ownership and avoid shared-file races.
- Do not persist or replay secrets; revalidate grants on every post-restart use.

## Remaining unknowns / blockers

- No production caller wires `ExecV2`/`ApprovalsV2`/`SchemaV2` into the live turn
  path; the installed journey cannot exercise restart/resume until that
  integration lands. This is the parent blocker, not a reason to weaken the test.
- `clean_shutdown` is written but never read; `owner_generation` is never advanced
  at startup. Startup fencing is unimplemented.
- Human approval/resume channel (`RequireHuman`) is unwired
  (`crates/server/src/lib.rs:1639-1644`).
- Frozen v1 `messages` path lacks an open/interrupted status, so the live path
  cannot represent an interrupted turn without the v2 writer.
- This lane cannot author the RED (research scope) and cannot verify it; a
  separate RED-authoring lane and a separate verifier lane are required.