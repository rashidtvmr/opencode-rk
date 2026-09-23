# Deterministic goal driver — next-phase feature proposal

Status: **PROPOSED, NOT IMPLEMENTED, NOT ACCEPTED**  
Prepared from repository revision: `399c65465a94d353276cdcb432a573f11909f827`  
Canonical-plan authority: this file is a reviewable addendum only. `FEATURES.md`,
`ralph.json`, accepted flags, controller policy, and frozen tests remain unchanged.
After controller/code-owner review, this proposal should be assigned canonical task
IDs and imported into the next-phase plan.

## 1. User outcome

When a user gives the main agent a substantial goal, the application shall:

1. derive explicit, observable success criteria and freeze a versioned goal contract;
2. decompose that goal into a bounded dependency graph of independently owned work;
3. maintain up to the configured safe concurrency, normally 15–20 useful lanes when
   harness, provider, memory, ownership, and verification capacity permit;
4. process each child completion independently and refill the freed slot immediately,
   without waiting for the slowest child in the original wave;
5. verify, integrate, and re-verify evidence before unlocking dependent work;
6. replan when trusted evidence changes what remains, not merely when a model says
   `done`;
7. survive parent-model turns, controller restart, missed live events, and worker
   failure without losing or duplicating work; and
8. continue until verified success, explicit user cancellation, a real HITL/authority
   boundary, or a hard safety/resource/budget/deadline blocker. A blocked or
   no-ready-work state is never reported as success.

“Active all the time” means the **owned controller remains available and reacts to
durable events**. It does not mean an LLM continuously consumes tokens or busy-polls.
The parent model may be idle while safe child work continues.

## 2. Why this must be deterministic

Prompt instructions such as “keep delegating” or “re-prompt yourself after every
completion” are useful policy hints but are not enforcement:

- model text is not durable state and may drift after compaction or restart;
- a worker can claim success without a candidate, independent tests, or integration;
- a live completion notification can be missed during disconnect;
- a timeout may leave an external side effect in an uncertain state;
- one straggler can create a batch barrier when the parent waits for all children;
- an unbounded planner can continually invent work and never converge; and
- prompts cannot enforce path ownership, leases, budgets, cancellation/join, or
  human-only authority.

The model therefore acts as an **untrusted planner**. A trusted, durable controller
owns admission, scheduling, evidence, stopping, and acceptance.

## 3. Existing foundation and exact gaps

Current repository behavior at the prepared revision:

- `tools/completion_scheduler.py:158-260` has an in-memory rolling primitive using
  `asyncio.FIRST_COMPLETED`, execution/verification/integration stages, bounded
  attempts, cancellation, and post-integration verification.
- `tools/completion_scheduler.py:179-180,212-216` permits only one active
  verification/integration pipeline, so completed candidates can queue behind a
  bottleneck.
- `tools/completion_scheduler.py:218` does not count the active checker in the
  unverified-capacity calculation.
- `tools/completion_scheduler.py:231-234` returns when no futures remain but has no
  durable stop cause; pending work can remain without making the run complete.
- `tools/completion_scheduler.py:200-203` identifies ambiguous integration but has no
  scheduler-level `uncertain -> reconcile` transition.
- `config/completion-controller.json:3-31` declares the intended 20-worker,
  refill-on-each-completion policy, one heavy validation, three attempts, leases,
  heartbeats, role reservations, and convergence-first scheduling.
- `prompts/COMPLETE_APP.md:69-89` and `docs/CONVERGENCE.md:70-85` already require
  per-completion refill and preservation of integration/verifier capacity.
- `tools/harness_adapter.py:173-228` defines a native harness boundary, capabilities,
  role separation, bounded logs, and owned-task tracking, but a protocol is not proof
  of a production native binding.
- `crates/agents/src/app_delegation.rs:83-103,285-301` correctly forbids automatic
  replay of an ambiguous last effect, but its state is process-local.
- `crates/agents/src/delegation_lane.rs:233-245,286-319` marks cancellation/join in
  local state without proving that an underlying native task actually stopped.
- `docs/AUTONOMOUS_EXECUTION.md:54-100` defines resumable state and bounded stop
  semantics, while `:128-131` still identifies production leases, heartbeats,
  parallel ready queues, durable acceptance, and bounded discovery as hardening work.
- `docs/CONVERGENCE.md:99-105` correctly states that coordination claims are not
  acceptance authority.

The next phase must compose and harden these mechanisms rather than add another
unwired scheduler.

## 4. Architecture decision

### 4.1 Trusted controller, untrusted planner

The planner may propose:

- the initial success criteria;
- task decomposition and dependencies;
- one-file ownership boundaries;
- discovery children;
- reprioritization after verified evidence; and
- a concise explanation for the user.

The planner may **not**:

- set task or goal status;
- alter frozen criteria, test hashes, evidence, budgets, policy, or pins;
- grant permissions or satisfy human-only authority;
- declare a candidate integrated or accepted; or
- extend scope or cost without the configured admission decision.

The controller exclusively owns validation, admission, leases/fencing, scheduling,
completion ingestion, retries, verification, integration, reconciliation, stop
latches, and the completion certificate.

### 4.2 Goal contract

Before execution, the planner emits a bounded `GoalProposal`. The controller accepts
it only when every mandatory criterion is decidable and evidence-backed.

```text
GoalSpec {
  goal_id
  revision
  objective
  criteria[]
  scope_digest
  plan_digest
  budgets
  discovery_limits
  authority_requirements[]
  created_by
  approved_changes[]
}

Criterion {
  id
  description
  mandatory
  evaluator_kind
  evaluator_spec
  required_evidence[]
  revision_binding
}
```

Allowed evaluator kinds are closed and typed: frozen test command, artifact/hash,
integrated-revision ancestry, durable-state predicate, security/resource bound,
external platform/device evidence, or explicit human decision. Free-form model
judgment alone cannot satisfy a mandatory criterion.

The main agent should derive criteria autonomously when they are measurable. It asks
HITL only when a material product choice is genuinely ambiguous, authority is human-
only, or changing scope/budget/security policy requires the operator. Criteria are
then frozen by revision and hash; a material change invalidates stale evidence.

### 4.3 Durable event journal and materialized state

Use an append-only, bounded journal as the source of orchestration history and an
atomically replaced materialized ledger for fast reads. Append and fsync the event
before applying it. Every event is idempotent by `event_id`.

```text
GoalEvent {
  event_id
  run_id
  controller_epoch
  sequence
  goal_revision
  task_id
  attempt_id
  phase
  owner
  fencing_token
  operation_id
  candidate_revision?
  integrated_revision?
  payload_hash
  created_at
}
```

Live OpenCode events and native completion callbacks are wake-up hints. They are not
truth. On startup, reconnect, timeout, or periodic ticks, the controller reconciles
the journal against authoritative adapter status, leases, VCS ancestry/diffs,
verification receipts, integration receipts, provider usage, and pending HITL.

This is required because the official OpenCode V2 client documents event streams as
live-only, without replay or automatic reconnection:
<https://opencode.ai/v2/docs/build/client/>.

### 4.4 State machines

Run states:

```text
planning -> running -> draining -> success
                    -> awaiting-human
                    -> incomplete
                    -> blocked
                    -> failed
                    -> cancelled
```

Task/attempt states:

```text
proposed -> validated -> admitted -> running -> execution-complete
  -> verification-queued -> verifying -> integration-queued -> integrating
  -> accepted
  -> retry-wait
  -> uncertain -> reconciling -> accepted | retry-wait | blocked | awaiting-human
  -> cancel-requested -> cancelling -> cancelled | uncertain
  -> blocked
```

`success` means all mandatory criteria pass against the exact current integrated
revision, every mandatory task is accepted, no mandatory task is pending/running/
uncertain, and all owned children have been joined. `incomplete`, `blocked`,
`awaiting-human`, `failed`, and `cancelled` are distinct and must never be rendered as
success.

Stop latches are monotonic. Once cancellation, hard budget, safety, invariant failure,
or run deadline is latched, the driver admits no new work and drains/cancels owned
work according to policy.

## 5. Rolling reconcile loop

The controller uses one event-driven loop, not fixed waves:

1. recover journal, ledger, controller epoch, leases, and uncertain operations;
2. evaluate the frozen goal criteria and materialized task graph;
3. reserve verification, integration, provider, memory, ownership, and evidence
   capacity before admitting execution;
4. fill safe execution slots from the ready queue;
5. wait for the **first** completion, control event, deadline, budget trip, lease
   expiry, or reconcile tick;
6. durably record and validate that event;
7. move the candidate into the verification/integration pipeline while retaining its
   ownership lease;
8. refill the freed execution slot immediately from already-ready work;
9. invoke the planner only when the graph genuinely needs a bounded plan delta; and
10. repeat until a terminal run state is proven.

The controller does **not** need a new parent-model call after every ordinary
completion. Known-ready work is refilled deterministically. A bounded parent replan is
triggered only by discovery evidence, invalidated assumptions, user steering, a
failed/blocked critical path, or no-ready-work while the goal remains unsatisfied.
Completion summaries may be injected into the parent session as durable synthetic
messages, but those messages do not control scheduling.

Relevant OpenCode V2 integration surfaces:

- subagents use fresh child sessions and explicit parent `subagent` permissions:
  <https://opencode.ai/v2/docs/agents/>;
- the embedded SDK owns a host, supports event subscriptions and cancellation, and
  must be explicitly closed: <https://opencode.ai/v2/docs/build/sdk/>;
- the client exposes session prompts, live events, abort signals, and authenticated
  local service discovery: <https://opencode.ai/v2/docs/build/client/>;
- plugins expose `session.synthetic`, `session.interrupt`, `session.wait`, permission
  APIs, durable plugin storage, and cancellable event subscriptions:
  <https://opencode.ai/v2/docs/build/plugins/>;
- the generated API includes sessions, active sessions, inboxes, wait, interrupt,
  background tools, events, permissions, and worktrees:
  <https://opencode.ai/v2/docs/api/>.

## 6. Capacity, fairness, and utilization

Concurrency is a vector, not one number:

```text
execution_slots
verification_slots
heavy_validation_permits
integration_slots = 1
reconciliation_slots >= 1
provider/account permits
candidate_count_limit
candidate_byte_limit
```

Policy:

- target 20 useful workers and preserve a useful minimum of 15 only when safe;
- reserve at least four integration-spine lanes and two independent test/verifier
  lanes before breadth, matching `docs/CONVERGENCE.md:78-85`;
- use FIFO with aging and critical-path priority, while preventing one provider,
  task family, or path owner from consuming all capacity;
- include the active verifier in candidate/backpressure accounting;
- reserve candidate and verifier capacity before starting more implementation;
- run at most one heavy Cargo/browser/workspace validation under the current host
  budget;
- stop admission under configured memory/provider/cost pressure while allowing
  in-flight cleanup and reconciliation; and
- report actual occupancy and the limiting resource rather than fabricating 20 busy
  workers.

## 7. Stragglers, deadlines, and cancellation

Each run has absolute monotonic deadlines for the run, task, attempt, execution,
verification queue, verification, integration queue, integration, cancellation
grace, and shutdown grace.

For a worker that would otherwise run for 1000 hours:

1. the attempt deadline expires;
2. the task becomes `cancel-requested` and no dependent work is unlocked;
3. the native adapter receives cancellation;
4. the controller waits a bounded grace period for **observed join**;
5. only after join does it release ownership/provider capacity and classify a safe
   retry or terminal timeout;
6. if join cannot be proven, the task becomes `uncertain`, its fencing token is
   invalidated, its ownership remains protected, and reconciliation/HITL decides the
   next action.

A logical `cancelled` flag is not proof that native work stopped. Controller shutdown
must cancel and join all owned work or preserve it as uncertain for recovery.

## 8. Effects, retries, and fencing

Every external effect receives an operation identity derived from run, task, attempt,
and effect index. Reusing an operation ID with a different payload is a conflict.

- read-only and proven idempotent operations may retry within the attempt cap;
- authentication, authority, security, signing, device, isolation, and exhausted
  budget failures block immediately;
- ambiguous writes never replay automatically;
- lost integration responses enter `uncertain`; reconciliation inspects ancestry and
  receipts before deciding whether the operation already succeeded;
- every mutation carries controller epoch, owner, attempt generation, and fencing
  token; stale events and stale user commands are rejected; and
- leases remain live through execution, verification, integration, and drain.

Controller restart increments its epoch. A stale lease is reclaimed only after proof
the prior owner stopped and the native task joined; PID alone is insufficient because
of PID reuse.

## 9. Bounded replanning and discovery

The planner submits a transaction, not direct work. A proposed child is admitted only
when:

- its parent and dependencies are in approved scope;
- the resulting graph is acyclic;
- its explicit owned path is normalized and conflict-free;
- frozen RED evidence and test obligations exist before implementation;
- its semantic fingerprint is not already present;
- discovery count, graph depth, byte, attempt, cost, and epoch limits remain; and
- any material expansion of mandatory scope has the required controller/human
  authority.

Suggested defaults for next-phase validation, subject to operator review: at most 50
discovered children per run, at most 8 discovery epochs, bounded graph depth, and no
more than 10% scope growth in one autonomous proposal. Hitting a cap yields
`incomplete` or `awaiting-human`, never success and never silent truncation.

## 10. User steering and HITL

User actions are authenticated, ordered control events, not text interpreted from a
worker transcript:

```text
PauseRun, ResumeRun, CancelTask, CancelRun, RetryTask,
SteerTask, ApproveHumanGrant, RejectHumanGrant
```

Each carries client identity, monotonic client sequence, run epoch, target task and
attempt generation, scope, and authorization. It is persisted before application and
uses compare-and-swap against current state. A stale steer cannot mutate a newer
attempt.

`RequireHuman` creates no running work. The driver persists `awaiting-human` with the
exact operation and accepts only a scoped grant bound to identity, operation digest,
policy version, expiry, and one-time/reusable semantics. The model cannot fabricate or
broaden that grant.

## 11. Evidence and completion certificate

Worker output, `passes:true`, claim status, source presence, and live event text are
never acceptance evidence. A task acceptance receipt requires:

1. admitted task and current goal/plan revisions;
2. real reachable candidate revision and independently inspected diff matching the
   ownership grant;
3. unchanged frozen test and command hashes;
4. independent verifier identity distinct from implementer;
5. positive required-test counts and obligation coverage;
6. serialized integration onto current mainline;
7. post-integration GREEN on the exact integrated revision;
8. durable acceptance publication with operation identity; and
9. no stale lease, unjoined child, or unresolved effect.

The goal completion certificate additionally binds all mandatory criterion receipts,
the exact integrated revision, scope/plan/goal digests, resource evidence, unresolved
task count of zero, and the trusted verifier identity.

## 12. Proposed next-phase feature cards

These IDs are provisional and must not be treated as canonical until controller review.

| ID | Feature | Observable contract |
|---|---|---|
| GDRV-001 | Frozen goal contract and typed success oracle | Main agent derives measurable criteria; controller freezes revisions/hashes and rejects self-referential or model-only success. |
| GDRV-002 | Durable journal, ledger, epochs, and replay | Append-before-apply events reconstruct the exact run after crashes; duplicate/stale events are harmless. |
| GDRV-003 | Rolling multi-pool scheduler | Refill on each durable completion while preserving verifier, integration, reconcile, provider, ownership, and memory capacity. |
| GDRV-004 | Native OpenCode child-session adapter | Real background child lifecycle, completion status, cancellation, join, permission scope, and bounded output are wired without per-agent OS orchestration processes. |
| GDRV-005 | Independent verification and serialized integration | Candidate acceptance requires frozen independent proof, current-main integration, and post-integration rerun. |
| GDRV-006 | Straggler deadlines and uncertain-effect reconciliation | Absolute deadlines prevent indefinite barriers; unconfirmed cancellation or side effects enter `uncertain`, never blind retry. |
| GDRV-007 | Bounded dynamic replanning and discovery | Evidence-triggered plan deltas converge under count/depth/byte/cost/epoch limits and cannot silently expand authority. |
| GDRV-008 | Durable user steering and HITL | Ordered authenticated commands and scoped human grants pause/resume/steer safely across restart. |
| GDRV-009 | Resource, provider, cost, and fairness controller | Safe 15–20 target utilization with honest backpressure, one heavy validation, quotas, aging, and bounded queues/output. |
| GDRV-010 | Goal completion certificate and installed journey | Exact-revision end-to-end proof is the only success path; incomplete/blocked/uncertain work fails closed. |

Suggested dependency order:

```text
GDRV-001 -> GDRV-002 -> GDRV-003 -> GDRV-004
                    \-> GDRV-006
GDRV-003 -> GDRV-005
GDRV-002 -> GDRV-007 -> GDRV-008
GDRV-003 -> GDRV-009
GDRV-004..009 -> GDRV-010
```

## 13. Frozen RED scenarios required before implementation

At minimum, independent test authors must establish compiling RED for:

1. **Rolling refill:** completing one of 20 workers durably starts the next eligible
   task before the slowest original worker finishes.
2. **Lost live event:** completion during subscriber disconnect is found exactly once
   by snapshot/journal reconciliation and still refills capacity.
3. **False completion:** `passes:true` or `done` without candidate/proof unlocks no
   dependency and emits no acceptance.
4. **Verifier backpressure:** candidate count and bytes remain bounded; active
   verification counts against capacity; implementation admission pauses safely.
5. **Serialized integration:** concurrent verified candidates integrate one at a time
   without losing either branch.
6. **Ambiguous integration:** integration succeeds but its response is lost; restart
   discovers the integrated revision and does not execute or merge again.
7. **Crash replay:** crashes before and after `TaskAdmitted` produce respectively zero
   work and exactly one resumed attempt.
8. **Stale fencing:** an event from fencing generation `n` cannot mutate state after
   generation `n+1` is issued.
9. **Straggler:** a virtual clock crosses the absolute deadline; cancellation is
   requested, join is bounded, and failure to prove join yields `uncertain` while
   unrelated ready work continues.
10. **Effect ambiguity:** timeout after an external write never auto-replays and
    requires reconciliation or HITL.
11. **Scope and frozen-test integrity:** actual VCS diff outside the grant,
    implementer self-verification, changed hash, zero tests, or ignored obligations
    all fail before integration.
12. **Resource/provider pressure:** memory floor, provider concurrency, cost limit,
    and `Retry-After` stop admission without limit evasion or orphaned work.
13. **Endless discovery:** duplicate or unbounded planner children hit deterministic
    caps and produce `incomplete`/`awaiting-human`, never `success`.
14. **Steer/cancel race:** ordered generation-aware control events cannot mutate a
    newer attempt and cancellation releases resources only after observed join.
15. **HITL:** `RequireHuman` creates no running lane; expired, wrong-operation, or
    wrong-policy grants remain rejected.
16. **Completion certificate:** any pending, blocked, uncertain, off-plan,
    unintegrated, unverified, or unjoined mandatory work makes goal success fail
    closed.

Tests must use a deterministic virtual clock, disposable repositories/state, a real
native adapter test boundary, bounded scripted provider responses, and immutable
frozen hashes. Sleep-based timing, mocked success text, source scans, and edited tests
are not acceptable evidence.

## 14. Non-goals and safety constraints

- No prompt is advertised as a sandbox.
- No detached task lacks an owner, cancellation path, and join evidence.
- No unbounded queue, retained transcript, event journal record, discovery graph, or
  retry loop.
- No per-agent OS process is introduced merely to simulate native orchestration.
- No speculative duplicate of a task with ambiguous side effects.
- No automatic approval of human-only operations.
- No increase to provider/cost/security authority.
- No goal completion based solely on a model response, task claim, module compile, or
  isolated lane GREEN.

## 15. Research basis

Two independent requested research passes were used:

- `9router-oc-muse-spark-1.3-contributor-free`: architecture, repository component
  mapping, state machine, evidence model, and initial tests;
- `9router-xk-gpt56-luna`: adversarial review of false completion, event loss,
  stragglers, restart, fencing, cancellation, ambiguous effects, resource pressure,
  and stopping semantics.

External patterns informing the design:

- OpenCode V2 exposes fresh-context subagents, explicit permissions, session APIs,
  cancellable tools, and live events, but its client events are intentionally not a
  durable replay log.
- Durable workflow systems use deterministic replay from recorded event history and
  isolate nondeterministic I/O as recorded activities:
  <https://docs.temporal.io/workflows>.
- Reconciliation controllers repeatedly compare desired and current state rather
  than trusting one notification:
  <https://kubernetes.io/docs/concepts/architecture/controller/>.
- Structured concurrency requires owned task references, cancellation propagation,
  and joining children; `FIRST_COMPLETED`/`as_completed` avoids batch barriers while
  explicit cancellation and deadlines bound stragglers:
  <https://docs.python.org/3/library/asyncio-task.html>.

These patterns are design evidence, not proof that this repository already implements
the feature.
