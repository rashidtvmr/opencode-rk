# APP012-RESTART-RESUME-CORRECTION-VERIFY

Task: `APP012-RESTART-RESUME-CORRECTION-VERIFY` / verification.
Role: independent durable recovery contract verifier.
Route: `9router-xk-gpt56-luna`.
Target: `0c44e799086afcb256fb304e34e0d076e9669fbd`.
Owned artifact: this worklog plus this task's ledger row. No source, schema, test, DB, or parent-card edits.

## Claim and method

Claimed before edits through `tools/completion_claims.py`, session
`ses_f2c03db99ffemfbWZLy0zQAIGG`. Source/schema is authority. The correction
commit changes only the correction artifact and coordination ledger relative to
`c54b68e6b2bd99825689c476ccfef0be653f5208`; the original contract remains
unchanged. Verification reads the original contract plus the correction as one
contract surface. No Cargo, RED authoring, DB mutation, or user DB access.

## P1: production caller correction PASS

The exact production `SchemaV2::open_existing` callers are:

- `StorageFacade::open`, `crates/storage/src/facade.rs:27-43`, call `:35`.
- `open_branch_workspace`, `crates/sessions/src/branch_v2.rs:86-103`, call
  `:92`; reachable through `open_web_sessions`, `crates/cli/src/main.rs:546-550`,
  invoked at `:718`.

`crates/storage/src/schema_v2.rs:98-150` validates app identity, version,
migration checksum, connection policy, and WAL. It does not read
`clean_shutdown`, write `clean_shutdown=0`, or advance `owner_generation`.
Neither production caller drives execution or approval recovery.

The live path remains unwired: `create_turn_stream` at
`crates/server/src/lib.rs:1126` uses the legacy `SessionService` message path;
`RequireHuman` becomes a terminal error at `:1639-1644`. P1 is exactly
resolved as caller-inventory precision, without implying feature wiring.

## P2: durable cross-restart replay fence PASS

`crates/storage/schema/v2/workspace.sql:222-243` defines approvals state 4 as
consumed and indexes only pending state 0. `ApprovalsV2::resolve` accepts
targets 1/2/4 and updates only `WHERE pk = ?4 AND state = 0`
(`crates/storage/src/approvals_v2.rs:82-127`). Therefore state 4 plus the
pending-only CAS is the durable cross-restart replay fence.

`GrantLedger` is an in-process `HashSet<ApprovalId>`
(`crates/security/src/app_policy.rs:245-285`), checked and consumed by
`decide` (`:288-323`). A fresh process has a fresh ledger. It is only the
in-process single-use guard, never the restart fence. P2 is exactly resolved.

## Combined contract preservation

- Original matrix remains 18 rows, same ordering and classifications
  (`worklog/APP012-RESTART-RESUME-CONTRACT.md:140-169`). Correction changes
  only row 12 replay attribution to durable state 4 plus CAS; row 12 remains
  `REJECT`.
- No-blind-replay invariant remains: only rows 3, 6, and 13 are conditional
  pre-effect resume cases. Dispatched/running/uncertain effects remain manual
  review, with no fabricated completion/failure or silent retry.
- Bounds and fences remain: `MAX_SWEEP_ROWS=500` and clamping
  (`crates/storage/src/approvals_v2.rs:16,135`), recovery indexes
  (`crates/storage/schema/v2/workspace.sql:165,185,212,242`), single-owner
  partial unique index (`:162`), immutable tool binding (`:216-220`), and
  duplicate-tool indexes (`:210`).
- Future RED boundary unchanged: separate storage test
  `crates/storage/tests/app012_restart_resume_red.rs`, disposable DB, real v2
  state, reopen/recovery assertions. Frozen installed journey hash remains
  `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.

## P3 external platform caveat PASS

`docs/storage/CRASH_CONSISTENCY.md:3-4` labels the protocol normative design,
not current implementation proof. Its ownership rule requires OS-held locks
before writable opens/scans (`:27-31`), with platform/inherited-capability
proof still external evidence. The correction records this caveat only; it
does not convert it into APP-012 acceptance.

## Commands and results

- `rtk git rev-parse HEAD` -> `0c44e799086afcb256fb304e34e0d076e9669fbd`.
- `rtk git grep -n 'open_existing\|GrantLedger\|state = 4\|clean_shutdown\|owner_generation' -- crates` -> 73 matches; production callers, in-memory ledger, state markers, and creation-only generation reviewed.
- `rtk shasum -a 256 crates/server/tests/app012_tool_journey_red.rs` -> `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.
- `rtk git diff --check` -> exit 0.
- `rtk python3 tools/validate_repository.py` -> FAIL `backlog exhaustion exit=1`, 51 pre-existing repository-wide backlog/ownership findings; no correction-induced finding.
- `rtk python3 tools/convergence_gate.py` -> BLOCKED by existing off-plan ledger and parent-completion findings; not changed.
- No product test, Cargo command, RED authoring, DB mutation, heavy process, or secret access.

## Verdict and remaining gaps

READY FOR SEPARATE RED AUTHORING: P1/P2 are exactly resolved. Combined
contract remains RED-feasible and conservative. This is contract readiness
only, never feature or APP-012 parent acceptance.

Unresolved external/product gaps remain: live v2 execution/approval/recovery
wiring; startup OS lock, `owner_generation`, and `clean_shutdown` fencing;
approval-resume channel; v1 live messages lacking open/interrupted status; and
real platform crash/OS-lock/inherited-capability evidence.
