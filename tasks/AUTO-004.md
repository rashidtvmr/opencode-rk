# AUTO-004

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-002.
Dependencies: none.
Test obligations: AUTO-004-T01, AUTO-004-T02, AUTO-004-T03, AUTO-004-T04, AUTO-004-T05.

## User-observable outcome

Foreground/background delegation with explicit ownership: a foreground submit
returns a handle bound to its owner, a background detach keeps the owner across
detach, delegation status is queryable from durable controller state without
log spelunking, and cancel reclaims the delegation within a bound. No OS
process per agent. Closes the durable status observability gap recorded in the
agent-delegation reconciliation while retaining the process-local BackgroundJob
semantics.

## Source evidence

- sources/disc-003-reconciliation.json `opencode.agent-delegation` finding:
  "Core BackgroundJob is explicitly process-local and tested as such. The
  opencode task tool layers foreground/background subagent UX on top, while
  the V2 TODO says durable status observation and V2 background-agent
  integration are still deferred." Unresolved: "Do not treat process-local
  BackgroundJob status as durable child-agent state"; resume/steer/navigation
  and persisted child context reconcile separately; cross-provider child
  selection is target behavior, not established by this evidence.
- PLAN.md sections 5-6: slice independence rules, additive fragments, mandatory
  RED-then-GREEN lifecycle; five named tests are a minimum taxonomy.
- docs/TDD.md sections 2-5: lifecycle order, compiling RED that fails for the
  missing behavior, frozen test hash + command manifest, GREEN minimum.
- requirements/user-requirements.json REQ-002: "Plan file Ralph JSON
  independent slices and autonomous loop"; tasks AUTO-001..AUTO-007, mandatory.
- tools/plan_model.py PHASE_RANK (`AUTO: 6`) + PHASE_OVERRIDE
  (`AUTO-001: 0, AUTO-002: 0` only): AUTO-004 has no override, rank 6 (M6
  hardening/completion milestone gating with synthesized lower-rank deps).
- tasks/TOOL-016.md: canonical card format followed here
  (Status/Kind/contract/test obligations/failure states/bounds/TDD/verification).
- Classification: partial upstream (process-local BackgroundJob + task-tool UX
  reference) + new user requirement (durable queryable status, bounded
  cancel, no per-agent OS process). Deliberate resource-bounded deviation:
  status is small queryable controller-owned record, not full durable
  child-agent state; full resume/steer/persisted child context stays out of
  scope per the reconciliation unresolved list.

## Observable contract

- `Mode { Foreground, Background }`; `State { Submitted, Running, Detached,
  Complete, Cancelled, Failed }`.
- `Handle { id: DelegationId, owner: OwnerToken, mode: Mode }`;
  `Status { id, owner, mode, state: State, summary: BoundedSummary }`.
- `submit(owner, mode, work) -> Result<Handle, DelegError>`: foreground submit
  returns a handle whose `owner` equals the submitting owner and whose `mode`
  is `Foreground`; entry enters `Running` and is visible to `status(handle)`.
- `detach(handle, owner) -> Result<Status, DelegError>`: owner-checked;
  background detach keeps the original owner (`status.owner == submit owner`),
  transitions `Running -> Detached`, detaches caller without losing ownership.
- `status(id) -> Result<Status, DelegError>`: queryable from controller-owned
  state without reading logs; unknown id returns `DelegError::Unknown`.
- `cancel(id, owner) -> Result<Status, DelegError>`: owner-checked; terminal
  state `Cancelled` with bounded cleanup (task joined, output dropped).
- Suggested module boundary: `crates/delegation/src/lib.rs` (new crate
  `opencode-rk-delegation`); worker ships additive fragment only, never edits
  shared `lib.rs`, `Cargo.toml`, schemas, migrations, controller state.
- Stdlib + Tokio only; no new dependency, no network, no secret logging.
- Out of scope (separate owners): resume/steer/navigation, persisted child
  context/messages, cross-provider child model selection, full durable
  child-agent state.

## Failure states

- Unknown id on `status`/`cancel`/`detach`: `Err(DelegError::Unknown)`; no
  state created, no side effects.
- Non-owner `detach`/`cancel`: `Err(DelegError::NotOwner)`; delegation state
  unmutated; ownership retained by original owner.
- Double cancel / cancel after terminal state: `Err(DelegError::Terminal)` or
  idempotent `Ok(Cancelled)` (frozen by RED); never resurrects work, never
  leaks a task.
- Work failure/panic: state `Failed` with bounded summary; no unbounded error
  text retained; status still queryable.
- No changes to the user's existing OpenCode database; tests use disposable
  fixtures only.

## Resource bounds

- No unbounded queue: admission bounded by `max_live: usize` (default small,
  e.g. 16); `submit` over cap returns `Err(DelegError::AtCapacity)` without
  queueing.
- No unbounded retained output: per-delegation summary capped at
  `max_summary_bytes` (e.g. 4 KiB, truncation with marker); full logs never
  retained in status record.
- Owner/cancel path: every live delegation has exactly one owner token; caller
  owns handle lifetime; `cancel` joins the task and drops output within
  `cancel_timeout_ms` (e.g. 1000 ms in tests); cancellation asserts task/FD
  reclamation, not just a state string.
- No per-agent OS process: delegation runs as in-process Tokio tasks on the
  single daemon runtime (PLAN.md ADR-001/ADR-003); spawning an OS process per
  agent fails the bound test. No detached task without an owner; no
  background thread beyond the runtime.

## Test obligations (frozen)

- AUTO-004-T01 (foreground submit returns owned handle): `submit(owner_a,
  Foreground, work)` => `assert_eq!(handle.owner, owner_a)`,
  `assert_eq!(handle.mode, Foreground)`,
  `assert_eq!(status(handle.id).state, Running)`,
  `assert_eq!(status(handle.id).owner, owner_a)`.
- AUTO-004-T02 (background detach keeps owner): `submit(owner_a, Foreground)`
  then `detach(handle, owner_a)` => `assert_eq!(st.owner, owner_a)`,
  `assert_eq!(st.state, Detached)`; `detach(handle, owner_b)` =>
  `assert_eq!(err, NotOwner)` and `assert_eq!(status(id).owner, owner_a)`.
- AUTO-004-T03 (status queryable without log spelunking): after submit/detach/
  complete transitions, `status(id)` returns current `State` with no log reads
  (test captures no log dependency: works with logging disabled/null sink);
  unknown id => `assert_eq!(err, Unknown)`.
- AUTO-004-T04 (cancel cleanup bounded): `cancel(id, owner_a)` =>
  `assert_eq!(st.state, Cancelled)`; `assert!(task_joined(id))` (no live task/
  FD remains); completion latency `assert!(elapsed <= cancel_timeout_ms)`;
  non-owner cancel => `assert_eq!(err, NotOwner)` and task still live.
- AUTO-004-T05 (process-local, no OS-per-agent process): spawn N delegations,
  `assert_eq!(child_process_count(), 0)` (no new OS processes attributable to
  delegations); process-local restart drops live set while queryable record
  retains only `Failed`/terminal small summaries (never presented as durable
  child-agent state); over-cap submit => `assert_eq!(err, AtCapacity)` and
  live count unchanged; summary bytes `assert!(summary.len() <=
  max_summary_bytes)`.

## TDD steps

1. Inspect: PLAN.md 5-6, TDD.md 2-5, REQ-002, plan_model.py rank, TOOL-016
   format, disc-003 agent-delegation finding (done, see evidence).
2. Contract: defined above.
3. Author tests AUTO-004-T01..T05; establish compiling RED (fail: no
   delegation module).
4. Freeze test hash + command manifest.
5. Implement minimum native Rust foreground/background delegation.
6. GREEN, refactor, rerun; negative tests (non-owner ops, unknown id, double
  cancel, over-cap, work failure, logging-disabled status).
7. Evidence + patch; verifier decides acceptance.

## Verification

```bash
git status --short -- tasks/AUTO-004.md
cargo test -p opencode-rk-delegation
cargo check --workspace
```
