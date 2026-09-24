# APP012-RESTART-RESUME-RED

## Claim

- Task: `APP012-RESTART-RESUME-RED`. Type: RED authoring. Role: durable SQLite
  recovery test author.
- Session: `ses_f2b9e2f5bffeIZKsTT1YveNzWZ`.
- Route allowed by the user list.
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/red-app012-restart-resume`.
- Branch: `red/APP012-RESTART-RESUME` (base `c7ac440bb7d738992b5c4eafd54f3afc39758a00`).
- Owned file: `crates/storage/tests/app012_restart_resume_red.rs` (this lane).
- Other permitted file: this worklog and this task's ledger row.
- Claimed via `tools/completion_claims.py` before file edits.

## Authority and inputs read

- `.agents/WORKER.md`, `docs/TDD.md`, `docs/SECURITY.md`.
- Accepted contract: `worklog/APP012-RESTART-RESUME-CONTRACT.md` (18-row matrix).
- Accepted correction: `worklog/APP012-RESTART-RESUME-CORRECTION.md` (P1 caller
  precision, P2 durable state-4 replay fence).
- Verifiers: `APP012-RESTART-RESUME-CONTRACT-VERIFY.md` (REVISE minor, since
  fixed), `APP012-RESTART-RESUME-CORRECTION-VERIFY.md` (READY FOR SEPARATE RED
  AUTHORING).
- Source: `crates/storage/schema/v2/workspace.sql`,
  `crates/storage/src/{schema_v2,execution_v2,approvals_v2,admission_v2,facade,writer_v2}.rs`.
- Existing disposable-DB pattern: `crates/storage/tests/restart_v2.rs`.
- Frozen installed test `crates/server/tests/app012_tool_journey_red.rs`
  SHA-256 `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`
  (untouched).

## Resource rule honored

Cancellation RED owns the sole Cargo slot. This lane ran NO `cargo`, `check`,
`test`, freeze, commit, or source/schema/dependency edit. Markers were read with
`read`/`grep` only. Compile verification is a pending command the orchestrator
must run when the Cargo slot frees (see "Pending command" below).

## Seam check: existing startup seam is `StorageFacade::open` (correction)

The earlier "blocked" framing was wrong. The accepted startup behavior is
observable through the EXISTING production seam `StorageFacade::open`
(`crates/storage/src/facade.rs:27`), which routes to
`SchemaV2::open_existing` (`facade.rs:35`) and runs before work is accepted.
`RecoveryV2` is not needed for the RED to compile: the startup tests drive
`StorageFacade::open` on a disposable workspace root and then read durable state
through existing `SchemaV2::open_existing` / read APIs after dropping the facade.

Current implementation gaps proven by the RED (all in `SchemaV2::open_existing`;
`facade.rs:27-43` only selects open-vs-init):
- reads no prior `clean_shutdown`;
- writes no `clean_shutdown=0`;
- advances no `workspace_state.owner_generation`;
- runs no conservative recovery scan (no running->uncertain, no attempt/tool
  uncertainty), so ambiguous effects stay silently "running" across restart.

There is no `RecoveryV2` symbol at this revision and the compiled test does not
reference one (the only mention is a doc comment stating it is not referenced).
The future recovery owner may live in `crates/storage/src/recovery_v2.rs` behind
the same startup seam, but the RED does not depend on that file name.

### Prewire needed for GREEN (implementation, not this lane)

Implement the accepted startup protocol inside the `StorageFacade::open` path
(before returning the facade): read prior `clean_shutdown`, write
`clean_shutdown=0`, increment `workspace_state.owner_generation` once under FULL,
then run a bounded conservative scan using the existing recovery indexes
(`executions_recovery_idx` state IN 0,1,4; `attempts_recovery_idx` state IN
0,1,4; `tools_recovery_idx` state IN 0,1,5) with `LIMIT` clamped to
`MAX_SWEEP_ROWS=500`, transitioning running execution -> uncertain(4) and open
attempt/tool -> uncertain, never rewriting terminal rows and never replaying.

## Tests authored

File `crates/storage/tests/app012_restart_resume_red.rs`, 18 tests, each builds
a disposable `tempfile::tempdir()` workspace, prepares durable state through real
`SchemaV2`/`ExecV2`/`ApprovalsV2`/`AdmissionV2`, crashes by dropping the
connection, and reopens with `SchemaV2::open_existing`.

### GREEN behavior locks (13 pass today)

Durable fences that already hold; they must stay green after recovery lands.

| Test | Matrix rows | Asserts |
| --- | --- | --- |
| `pending_approval_survives_reopen_and_resolves_once` | 1 | state 0 persists; single-winner resolve |
| `consumed_approval_state4_blocks_cross_restart_replay` | 12 | state 4 persists; CAS rejects re-grant |
| `mandatory_human_approval_requires_client_identity_after_restart` | 1 | mandatory gate survives restart |
| `terminal_tool_states_survive_reopen_and_reject_rewrite` | 7,8,9 | 2/3/4 terminal, immutable |
| `uncertain_execution_and_dispatched_tool_hold_owner_slot` | 4,5,11,17 | open states hold single-owner slot |
| `uncertain_execution_resolves_to_terminal_and_releases_slot` | 11 | explicit 4->2 CAS releases slot |
| `second_client_observes_same_durable_rows_and_second_execution_rejected` | 17 | two clients, same rows, dup rejected |
| `duplicate_tool_ordinal_rejected_by_unique_index` | 18 | UNIQUE(assistant_message_pk, ordinal) |
| `pending_input_survives_reopen_and_promotes_once` | 13,14 | pending persists, idempotent promotion |
| `provider_attempt_survives_reopen_and_terminal_attempt_immutable` | 10,15,16 | prepared persists; terminal immutable |
| `expired_approval_sweep_is_explicit_and_bounded` | 2 | sweep-only expiry, idempotent |

(`clean_shutdown_marker_written_on_facade_close` is the 12th lock: close writes
the marker, expected PASS today.)

### TRUE RED (5 fail behaviorally; 1 startup guard already holds)

All drive the existing production startup seam `StorageFacade::open` and then
read durable state via `SchemaV2::open_existing`. Compile against existing APIs
only; no `RecoveryV2` import. Observed: 5 FAILED, and
`startup_does_not_rewrite_terminal_states` passes today because current open is a
no-op; it must remain green after recovery lands.

| Test | Matrix rows | RED assertion (fails now; GREEN after startup protocol) |
| --- | --- | --- |
| `startup_advances_owner_generation` | startup fencing | owner_generation 0 -> 1 on first open |
| `startup_clears_clean_shutdown_before_accepting_work` | clean/unclean | close sets 1; next open must set 0 |
| `startup_converts_running_execution_to_uncertain` | 5,11 | running(1) -> uncertain(4), finished NULL |
| `startup_converts_open_attempts_and_tool_to_uncertain` | 4,15,16 | prepared(0)/dispatched(1) attempt + dispatched tool -> uncertain |
| `startup_does_not_rewrite_terminal_states` | 7,8,9 | terminal 2 rows unchanged by startup |
| `startup_recovery_is_bounded_and_drains_across_passes` | scan bound | 501 crashed rows: first pass leaves >=1, second drains to 0; all become 4 |

`startup_does_not_rewrite_terminal_states` also passes today (current open is a
no-op); it is the guard that the recovery owner must not regress it. It is
listed under TRUE RED group because it exercises the startup seam, but it is
already-satisfied and is expected to remain GREEN.

Rows 3/6 (conditional pre-effect resume) are intentionally not auto-driven: the
accepted contract makes resume conditional on digest/expiry/policy
revalidation, which lives in the broker, not storage; asserting auto-resume here
would contradict the no-blind-replay invariant. They remain covered by the
`Resume (conditional)` classification in the contract matrix and are out of this
storage RED's authority.

## Static compile review (no Cargo run)

Every referenced symbol was checked against the current source signature:
`SchemaV2::initialize_workspace`/`open_existing`, `ExecV2::{start_execution,
transition_execution, record_attempt, finish_attempt, plan_tool, finish_tool}`,
`ApprovalsV2::{request, resolve, expire_sweep}`, `AdmissionV2::{submit_input,
promote_input}`, `StorageFacade::{open, close}`. All DDL CHECKs the fixtures must
satisfy were verified (messages status/completed pairing, tool_calls state vs
finished_at_us, approvals state 1/4 mandatory-human, tool_assistant_owner
trigger, execution state vs finished_at_us, `record_attempt` rejected on terminal
execution so terminal fixtures finish the execution last). No `unsafe`, no
network, no wall-clock assertions, no user DB.

## Focused run and 18-row result matrix (sole Cargo slot, jobs1/threads1)

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 \
cargo test --no-fail-fast -p opencode-rk-storage --test app012_restart_resume_red -- --test-threads=1
```

Compiled clean (`Finished test profile in 21.45s`, first build), then:
`13 passed; 5 failed; 0 ignored` in 0.28s. Re-run after the bounded-drain
refinement reproduced the identical set.

| # | Test | Matrix rows | Expected | Observed | Cause |
| --- | --- | --- | --- | --- | --- |
| 1 | `pending_approval_survives_reopen_and_resolves_once` | 1 | GREEN lock | ok | fence holds |
| 2 | `consumed_approval_state4_blocks_cross_restart_replay` | 12 | GREEN lock | ok | CAS holds |
| 3 | `mandatory_human_approval_requires_client_identity_after_restart` | 1 | GREEN lock | ok | gate holds |
| 4 | `terminal_tool_states_survive_reopen_and_reject_rewrite` | 7,8,9 | GREEN lock | ok | terminal immutable |
| 5 | `uncertain_execution_and_dispatched_tool_hold_owner_slot` | 4,5,11,17 | GREEN lock | ok | partial UNIQUE holds |
| 6 | `uncertain_execution_resolves_to_terminal_and_releases_slot` | 11 | GREEN lock | ok | 4->2 CAS holds |
| 7 | `second_client_observes_same_durable_rows_and_second_execution_rejected` | 17 | GREEN lock | ok | dup-owner holds |
| 8 | `duplicate_tool_ordinal_rejected_by_unique_index` | 18 | GREEN lock | ok | UNIQUE holds |
| 9 | `pending_input_survives_reopen_and_promotes_once` | 13,14 | GREEN lock | ok | promotion idempotent |
| 10 | `provider_attempt_survives_reopen_and_terminal_attempt_immutable` | 10,15,16 | GREEN lock | ok | terminal immutable |
| 11 | `expired_approval_sweep_is_explicit_and_bounded` | 2 | GREEN lock | ok | sweep bounded |
| 12 | `clean_shutdown_marker_written_on_facade_close` | clean | GREEN lock | ok | close writes marker |
| 13 | `startup_does_not_rewrite_terminal_states` | 7,8,9 guard | GREEN lock | ok | open is a no-op now; must stay |
| 14 | `startup_advances_owner_generation` | startup fencing | RED | FAILED | `left: 0, right: 1`; no generation advance (`schema_v2.rs:98-151`) |
| 15 | `startup_clears_clean_shutdown_before_accepting_work` | clean/unclean | RED | FAILED | `left: 1, right: 0`; clean_shutdown never read/written at open |
| 16 | `startup_converts_running_execution_to_uncertain` | 5,11 | RED | FAILED | `(1,None)` != `(4,None)`; no recovery scan |
| 17 | `startup_converts_open_attempts_and_tool_to_uncertain` | 4,15,16 | RED | FAILED | attempt 1 state 0 != 4; dispatched tool stays open; no scan |
| 18 | `startup_recovery_is_bounded_and_drains_across_passes` | scan bound | RED | FAILED | `got 501 running`; no bounded per-open drain |

All 5 failures are fail-for-cause through the real `StorageFacade::open` seam:
the absent startup marker fencing (14, 15), the absent conservative recovery scan
(16, 17), and the absent bounded scan drain (18). None is a compile, harness, or
fixture error. Failures print assertion left/right values as required by
`docs/TDD.md` section 3.

### Bounded-drain contract correctness (audited)

`startup_recovery_is_bounded_and_drains_across_passes` now asserts, after ONE
open, `1 <= running_remaining <= 500`. This is contract-correct and not a mere
"two opens" assumption: the accepted contract fixes `MAX_SWEEP_ROWS=500`
(`approvals_v2.rs:16,135`) and requires every recovery scan query to use
`LIMIT ?limit` under that bound (`worklog/APP012-RESTART-RESUME-CONTRACT.md`
"Recovery ownership and bounded scan"). With 501 crashed rows, a correct per-open
cap of 500 must leave exactly one row running after the first pass and drain it
on the second. The assertion fails today with 501 remaining, proving no per-open
cap exists. Second pass then requires 0 running and 501 uncertain. This matches
the contract; no weakening.

## Freeze

- Frozen RED file:
  `crates/storage/tests/app012_restart_resume_red.rs`
  SHA-256 `d2745fe142fb1add3cc26c9fe9715438c73a51084d5498373c990c0a8e2a8cce`.
- Preserved APP-012 installed journey
  `crates/server/tests/app012_tool_journey_red.rs`
  SHA-256 `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`
  (untouched).
- `rustfmt --edition 2021 --check` clean; `git diff --check` exit 0.
- Command manifest: the focused command above, jobs1/threads1.

## Handoff: exact source owner and file (from accepted contract)

Accepted contract source owner is a NEW module
`crates/storage/src/recovery_v2.rs` implementing
`RecoveryV2::recover(connection, now_us, owner_generation, limit)`, registered in
`crates/storage/src/lib.rs`. The startup protocol that makes tests 14-18 GREEN is
invoked from the existing production seam `StorageFacade::open`
(`crates/storage/src/facade.rs:27`, reaching `SchemaV2::open_existing` at
`:35`): before returning the facade, read prior `clean_shutdown`, write
`clean_shutdown=0`, increment `workspace_state.owner_generation` once under FULL,
then call `RecoveryV2::recover` with `limit=500`. The daemon/startup integration
caller (`crates/server` startup or `crates/sessions` branch open) is a separate
integration lane.

No implementation was performed by this lane.

## Final status

`completed`: compiling stable RED, fail-for-cause, frozen and committed. This is
RED readiness only, never feature or APP-012 parent acceptance. Parent APP-012
stays open.

## Landing

- Commit `929acfcb7a6e5c5a0fdf4718cdaa80f41db1b44f` on
  `red/APP012-RESTART-RESUME`.
- Pushed to `origin/red/APP012-RESTART-RESUME`; `fetch` confirms
  `remote == HEAD`, working tree clean, no force-push.
- Lane files: `crates/storage/tests/app012_restart_resume_red.rs`,
  `worklog/APP012-RESTART-RESUME-RED.md`, `tasks/completion/claims.json`.
- Memory during run: `memory_pressure` reported 42% free before the focused run;
  single jobs1/threads1 Cargo process, no parallel builds, no survivors.
