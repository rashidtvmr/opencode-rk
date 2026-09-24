# APP012-RESTART-RESUME-CONTRACT-VERIFY

Task: `APP012-RESTART-RESUME-CONTRACT-VERIFY` / type verification / role durable SQLite recovery/state-machine verifier.
Route: `9router-oc-muse-spark-1-3-contributor-free`.
Target commit: `367756e2d7da58baa25753321647d2b9f88a034c`, branch `plan/APP012-RESTART-RESUME`.
Owned file: `worklog/APP012-RESTART-RESUME-CONTRACT-VERIFY.md` (this file). No source/schema/test/DB edits.

## Claim

Claimed via `tools/completion_claims.py` before edits, session `ses_f2cb4cbcaffeCuFVBWVglB0ngq`.
Base HEAD verified `367756e2d7da58baa25753321647d2b9f88a034c`.

## Method

Source/schema authority only. Every contract assertion re-checked against exact file:line below.
Task artifacts treated as claims. No Cargo run, no DB mutation, no RED authoring.

## State-enum verification (all PASS)

`crates/storage/schema/v2/workspace.sql`:
- `executions.state` 0-5, terminal 2/3/5 (`:150`, `:160`). `executions_single_owner_idx` partial UNIQUE `WHERE state IN (0,1,4)` (`:162`). Uncertain holds owner slot: exact.
- `provider_attempts.state` 0-5, terminal 2/3/5 (`:175`, `:183`).
- `tool_calls.state` 0-5, terminal 2/3/4 (`:195`, `:208`). Uncertain=5 open (`finished_at_us IS NULL`), cancelled=4 terminal.
- `approvals.state` 0-4 (`:231`); zero triggers on `approvals` in file (triggers present only for blobs/payloads/messages/inputs/executions/tools). "No transition trigger": exact.
- `approvals` evidence-not-authority comment (`:222`): exact.
- `session_inputs.state` 0-2, terminal 1/2 (`:118`, `:126`).
- `messages.status` 0-3, `status=0 AND completed NULL OR status<>0 AND completed NOT NULL` (`:88`, `:96`). "Non-zero terminal": exact.
- `workspace_state.clean_shutdown` 0/1, `owner_generation` int (`:14-15`): exact.
- Recovery indexes all present and bounded as cited: `executions_recovery_idx` (`:165`, states 0,1,4), `attempts_recovery_idx` (`:185`, states 0,1,4), `tools_recovery_idx` (`:212`, states 0,1,5), `approvals_pending_idx` (`:242`, state=0), `inputs_pending_idx` (`:129`, state=0).
- `tool_assistant_owner` (`:216`), `tool_binding_immutable` (`:219`): exact, no-rebind claim holds.

`crates/storage/src/execution_v2.rs`: `start_execution` `:20`, `transition_execution` `:56`, `record_attempt` `:88`, `finish_attempt` `:115`, `plan_tool` `:142`, `finish_tool` `:178`. Allowed edges (`:217-222`): `(0,1) (0,5) (1,2) (1,3) (1,4) (1,5) (4,2) (4,3) (4,5)`: exact. `finish_attempt` to 4 forces NULL (`:124-126`); `finish_tool` to 5 forces NULL (`:195-199`); terminal requires `finished_us=Some` (`:66`): exact.
`crates/storage/src/approvals_v2.rs`: `request` `:26`, `resolve` `:82`, `expire_sweep` `:130`. Resolve targets 1/2/4 only (`:89-90`), single-winner CAS `WHERE pk=? AND state=0` (`:109-126`): exact. `mandatory_human=1` requires `human_client_id` (`:104-107`): exact. `MAX_SWEEP_ROWS=500` (`:16`), clamp (`:135`): exact.
`crates/storage/src/admission_v2.rs`: `submit_input` `:29`, `promote_input` `:100`, `receipt_lookup` `:188`, `receipt_store` `:214`. Double-promote errors (tests `:370-371`): "promotion idempotent by state" holds in the fail-closed sense. `retry_until_us` expiry (`:206`, `:233`): exact.

## Crash-matrix verification (18 rows, all classifications grounded)

- No-replay invariant: `CRASH_CONSISTENCY.md:22` ("never silently replay ambiguous external effects"), `:165` (dispatch gap conservatively UNCERTAIN), `:168` (unsettled dispatched/running become UNCERTAIN, never fabricated failures/silent retry), `:175` (retry/resume/abandon needs explicit authorized decision). RESUME limited to rows 3/6/13 (pre-effect): consistent.
- Row 4/15/16 `finish_tool(..,5)` valid: `finish_tool` accepts 5 (`:186`), forces NULL. Row 5 `transition_execution(..,1,4,None)` valid edge; `finish_attempt(..,4)` valid (`:121`). Row 11 owner-slot retention: follows from `:162`.
- Row 3 revalidation triple: `Grant::covers` checks expiry (`app_policy.rs:159-161`), policy version (`:163-168`), digest (`:169-171`), workspace/session/requester scope (`:172-186`): exact. Ledger single-use in `decide` not `covers` (`:152-154` doc, `GrantLedger::consume` Replay `:275-277`): exact.
- Rows 17/18 fences: `:162` + resolve CAS + `UNIQUE(assistant_message_pk, ordinal)` (`:203`) + `tool_provider_call_idx` (`:210`): exact.
- Row 9: cancelled=4 terminal with `finished_at_us NOT NULL` per `:208`: exact.

## Gap verification (all PASS)

- `clean_shutdown` written `facade.rs:139` (+`wal_checkpoint(TRUNCATE)` `:140`), zero readers anywhere in `crates` (grep: only `facade.rs:139` + DDL `:15`): write/read gap exact.
- `owner_generation` never advanced at startup: grep hits only `execution_v2.rs:24,30,38,44` (creation param), `workspace.sql:14,151,165` (DDL/index), `stress_v2.rs:261` (test). `schema_v2.rs::open_existing` (`:98-151`) verifies app_id/version/checksum, applies CONNECT_POLICY, re-verifies WAL; no marker read/write: exact.
- CONNECT_POLICY identical both files (`schema_v2.rs:21`, `facade.rs:17`): FK ON, synchronous=FULL, busy_timeout=5000: exact.
- Live path unwired: `create_turn_stream` `:1126`, `TurnStreamStage` `:1113-1124`, persists via `append_text` (`lib.rs:650,939,1165,1690`) / `append_assistant_with_activity` (`:970,1504,1749`); `RequireHuman` terminal error string (`:1429`, `:1639`); v1 `Storage::migrate` (`storage/lib.rs:717-718`) flat tables, no status column; `writer_v2.rs:203-214` hardcodes `status=1`. No durable open/interrupted marker on live path: exact.
- `SessionService` holds `Arc<Storage>` v1 (`sessions/lib.rs:334,350`); `cli/main.rs:541-557` opens v1 `Storage` + branch workspace. Schema existence never advertised as wiring; contract keeps parent open: honest.

## Precision findings (minor, RED unaffected)

1. P1 (inexact evidence sentence): contract claims `ExecV2`/`ApprovalsV2`/`AdmissionV2`/`StorageFacade`/`SchemaV2::open_existing` have "NO production caller outside `branch_v2.rs:92` and unit tests". True for `ExecV2`/`ApprovalsV2`/`AdmissionV2` (callers: storage unit + integration/perf tests only) and for `StorageFacade` itself (callers: facade tests only). Inexact for `SchemaV2::open_existing`: production callers are `facade.rs:35` (`StorageFacade::open`) and `branch_v2.rs:92` (`open_branch_workspace`, reached from `cli/main.rs:549`). Fix: "no production caller wires them into the live turn path; the only production `open_existing` callers are `StorageFacade::open` and `open_branch_workspace`, neither of which drives execution/approval state."
2. P2 (cross-restart replay wording): row 12 cites `GrantLedger` as the replay block. `GrantLedger` is in-memory (`HashSet`, `app_policy.rs:249-252`); across a restart the durable block is `approvals.state=4` (resolve CAS requires `state=0`). Fix: cite durable `state=4` + CAS for restart, `GrantLedger` for in-process single-use.
3. P3 (missing normative-design caveat pointer): `CRASH_CONSISTENCY.md:3` states normative DESIGN, not a claim about implementation; `:27-31` requires OS ownership locks before opening. Contract's startup-caller step should cite the OS-lock prerequisite alongside the FULL/gen/recover ordering. Suggestion only.

## RED feasibility (PASS)

All future-RED APIs exist with verified signatures: `SchemaV2::initialize_workspace`/`open_existing`, `ExecV2::*`, `ApprovalsV2::*`, `AdmissionV2::*` on disposable `tempfile` DBs (pattern already used by `storage/tests/restart_v2.rs`, `integration_v2.rs`). Proposed `RecoveryV2::recover(conn, now_us, owner_generation, limit)` transitions (1->4, attempt->4, tool->5, expire sweep, stale-gen cancel) are all currently legal edges/CAS ops. Single owner file `crates/storage/src/recovery_v2.rs`; startup wiring and router RED correctly deferred to separate lanes. Ownership separation storage/server/runtime exact.

## Commands/results

- `git rev-parse HEAD` -> `367756e2d7da58baa25753321647d2b9f88a034c`; branch `plan/APP012-RESTART-RESUME`.
- `git grep -n 'clean_shutdown\|owner_generation\|state = 4\|state = 5\|recover\|resume' -- crates` -> 58 matches reviewed; key hits confirm write-only `clean_shutdown`, creation-only `owner_generation`.
- `shasum -a 256 crates/server/tests/app012_tool_journey_red.rs` -> `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50` unchanged (expected hash holds).
- `git diff --check` -> exit 0 (re-run after write before commit).
- `python3 tools/validate_repository.py` -> FAIL `backlog exhaustion exit=1` (pre-existing repo-wide; listed stale ownership-gap findings; not repaired per boundary).

## Verdict

REVISE (minor): contract is source-grounded and RED-feasible; land after fixing P1 sentence and P2 row-12 wording (P3 optional). No feature acceptance; parent APP-012 stays open; RED not yet authorized; frozen test untouched.

## Resources/gaps

No Cargo/DB/heavy process run; read-only greps + reads. Unresolved: startup OS-lock + `owner_generation` fencing unimplemented (admitted parent blocker); approval-resume channel unwired (`lib.rs:1639`); v1 messages lack open/interrupted status on live path.
