# APP012-RESTART-RESUME-CORRECTION

## Claim and scope

- Task: `APP012-RESTART-RESUME-CORRECTION`.
- Type: research. Role: durable recovery contract corrector.
- Route: `oc-space-bunny-free` (permitted route).
- Session: `ses_f2c0a4bf5ffeHz75U16t312m9k`.
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/plan-app012-restart-resume-correction`.
- Branch: `plan/APP012-RESTART-RESUME-CORRECTION`.
- Base/source revision: `c54b68e6b2bd99825689c476ccfef0be653f5208` (verifier commit; target contract revision `367756e`).
- Owned artifact: this file only. Ledger row is the only other permitted edit.
- No product, schema, test, parent-card, verifier, or original-contract edits.

## Authority and defect set

Source/schema is authority. Read: `PLAN.md` (APP-012 delivery boundary),
`docs/TDD.md`, `docs/SECURITY.md`, the original contract
`worklog/APP012-RESTART-RESUME-CONTRACT.md`, verifier worklog
`worklog/APP012-RESTART-RESUME-CONTRACT-VERIFY.md`, exact callers, v2 SQL/state
owners, `GrantLedger`, and `docs/storage/CRASH_CONSISTENCY.md`.

This correction addresses exactly the two verifier findings P1/P2 from commit
`c54b68e`. P3 is recorded as a caveat only.

### P1 — precise production-open caller statement

Original overclaim: the contract says `SchemaV2::open_existing` has “no
production caller outside `branch_v2.rs` and their own unit tests.” The source
does have two production `open_existing` call sites. Replace only that clause
with this source-exact wording:

> `SchemaV2::open_existing` has two production call sites: `StorageFacade::open`
> (`crates/storage/src/facade.rs:27-43`, call at `:35`) and
> `open_branch_workspace` (`crates/sessions/src/branch_v2.rs:86-103`, call at
> `:92`; reached by `open_web_sessions` in `crates/cli/src/main.rs:546-550`,
> invoked at `:718`). Neither site drives execution or approval recovery. The
> live turn path remains unwired: `create_turn_stream` persists through the
> legacy `SessionService` path, and `RequireHuman` terminates as an error
> (`crates/server/src/lib.rs:1126-1167, 1639-1644`).

Evidence: `SchemaV2::open_existing` validates identity/version/checksum and
connection policy only (`crates/storage/src/schema_v2.rs:98-150`); it does not
read `clean_shutdown`, write `clean_shutdown=0`, or advance
`owner_generation`. `StorageFacade::open` only selects open-versus-initialize
(`crates/storage/src/facade.rs:27-43`); `open_branch_workspace` only opens the
branch DB (`crates/sessions/src/branch_v2.rs:86-103`). This corrects caller
inventory, not the live-path blocker.

### P2 — row-12 cross-restart replay fence

Replace row 12’s evidence/replay wording with:

> **Row 12 — consumed approval across process restart:** `approvals.state=4`
> (consumed) plus `ApprovalsV2::resolve`’s pending-only CAS
> (`WHERE pk=? AND state=0`) is the durable cross-restart replay prevention.
> `GrantLedger` is a process-local `HashSet` checked/consumed by `decide`; it
> provides only in-process single-use behavior and is not a cross-restart fence.

Evidence: v2 SQL defines state `4` as consumed and indexes pending rows
(`crates/storage/schema/v2/workspace.sql:222-243`). `ApprovalsV2::resolve`
accepts targets `1/2/4` and updates only `state=0`
(`crates/storage/src/approvals_v2.rs:82-127`). `GrantLedger` stores
`HashSet<ApprovalId>` in memory and is consumed by `decide`
(`crates/security/src/app_policy.rs:245-285, 288-323`). A fresh process has a
fresh ledger, so only the durable approval state/CAS can prevent replay after
restart. This remains a future owner contract; current live approval recovery is
not wired.

## Optional caveat (P3)

`docs/storage/CRASH_CONSISTENCY.md:3-4` explicitly calls the protocol normative
**design**, not a claim that the current implementation satisfies it. Its
ownership section requires OS-held locks before writable opens/scans
(`:27-31`). Real OS-lock/platform proof, including inherited-capability
closure, remains a later prerequisite; it is not current APP-012 acceptance and
does not expand this research correction.

## Impact on the 18-row matrix and RED boundary

- Matrix size and row ordering remain 18. No classification changes.
- Row 12 remains `REJECT`; only durable evidence and replay ownership wording
  change. It is not silently replayable after restart.
- P1 changes only the live-path caller inventory. The accepted blocker remains:
  no v2 execution/approval/recovery owner drives the live turn.
- Rows 3, 6, and 13 remain the only conditional/provably pre-effect resume
  cases. Dispatched/running/uncertain cases remain manual review; ambiguous
  effects are never blindly replayed. This follows
  `CRASH_CONSISTENCY.md:162-177` and the original matrix.
- Row 17’s single-owner/pending-CAS fence and row 18’s duplicate-tool fence are
  unchanged. Existing `LIMIT 500`, recovery indexes, unique-owner constraint,
  and revalidation requirements are unchanged.

RED boundary is unchanged and not authored here. The future storage RED remains
`crates/storage/tests/app012_restart_resume_red.rs` with a disposable
`tempfile` database, real `SchemaV2`/`ExecV2`/`ApprovalsV2`/`AdmissionV2`
state construction, drop/reopen, `RecoveryV2::recover`, and assertions for all
matrix outcomes. It must prove row 12 through durable `approvals.state=4` and
pending-only CAS, not through `GrantLedger`; a new process-local ledger must not
be treated as restart persistence. The installed APP-012 journey remains
separate and open. Frozen file hash remains
`945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.

## Why no other contract changes are required

P1 is an evidence-precision correction. P2 corrects attribution of an existing
durable CAS/state machine, not its transitions. The accepted state/race/bounds
contract remains intact: conservative uncertain transitions, explicit
authorized reconciliation, no blind replay, no fabricated completion/failure,
bounded scans (`MAX_SWEEP_ROWS=500`), and duplicate-owner fences. P3 is a
normative-design caveat, not a behavior change. No schema, recovery algorithm,
RED target, product caller, or parent acceptance decision is introduced here.

## APP-012 status and unresolved gaps

APP-012 remains **OPEN**. This artifact does not accept the parent. Remaining
blockers are unchanged: live v2 execution/approval/recovery wiring; startup
OS-lock plus `owner_generation`/`clean_shutdown` fencing; approval-resume
channel; v1 messages without open/interrupted status on the live path; and real
platform crash/OS-lock proof. See `worklog/APP-012.md:98-100` and
`tasks/completion/local.json:15`.

## Validation and resource record

Research-only validation; no Cargo, database, or product test run.

- `rtk git grep -n 'open_existing\|GrantLedger\|state = 4\|clean_shutdown\|owner_generation' -- crates` — completed; confirms the two production `SchemaV2::open_existing` call sites, in-memory ledger, approval state/CAS, and startup-marker gaps.
- `rtk shasum -a 256 crates/server/tests/app012_tool_journey_red.rs` — `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50` (unchanged).
- `rtk git diff --check` — expected exit 0 after artifact/ledger-only changes.
- `rtk python3 tools/validate_repository.py` — pre-existing repository-wide backlog-exhaustion/ownership findings only; no correction-induced failure, no policy/test edits.
- Resource profile: read/grep/ledger validation only; no heavy process, no DB mutation, no user database access, no secret access.
