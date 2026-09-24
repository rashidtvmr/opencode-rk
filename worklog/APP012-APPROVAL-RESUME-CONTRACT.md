# APP012 durable approval/resume contract

## Claim and authority

- Task: `APP012-APPROVAL-RESUME-CONTRACT`
- Session: `ses_f2cd8fde2ffdJ2jDd9pI1JVB61`
- Branch: `plan/APP012-APPROVAL-RESUME`
- Research base: `5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b`
- Scope: research artifact only. No product, schema, route, runtime, or test edits.
- Frozen APP-012 read test: `crates/server/tests/app012_tool_journey_red.rs`
- Frozen SHA-256: `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`
- Independent boundary: future RED author, implementer, and verifier remain separate. This artifact proposes a contract; it is not product acceptance.

## Governing requirements

- `AGENTS.md:Authority and ownership`, `AGENTS.md:Convergence and parent-completion boundary`, `AGENTS.md:Non-negotiable engineering rules`.
- `PLAN.md:105-111` ADR-006: authorization outside the model; grants bind resource, operation, identity, expiration, and policy version.
- `docs/SECURITY.md:9-22,62-81`: capability-based authorization, wildcard cannot lift human-only authority, bounded state, no secret leakage.
- `docs/TDD.md:21-41,58-75`: compile-and-fail RED, frozen hash, independent verification, no test weakening.
- `docs/CONVERGENCE.md:8-24,54-68`: live installed journey, real broker/tool/persistence, no isolated-module acceptance.
- `tasks/completion/local.json:15` APP-012: installed journey must include approval/tool edit, second client, restart/resume, failure recovery, bounded resources.
- `tasks/SEC-001.md:35-68`: mandatory human gates survive `*`; grants require explicit identity/scope.
- `tasks/DB-004.md:21-34`: v2 approval states, pending CAS, mandatory human identity, bounded expiry.

## Source ledger: current behavior versus required behavior

| Source | Current verified behavior | Required integration consequence |
|---|---|---|
| `crates/security/src/lib.rs:61-77,206-240` (`Decision`, `PermissionBroker::authorize`, `record`) | `RequireHuman` is mandatory; baseline decisions are not lifted by rules; audit is capped at 1024. | Re-run the trusted broker immediately before dispatch. Never treat an approval row as a capability. |
| `crates/security/src/app_policy.rs:35-49,68-181,245-339` (`OperationDigest`, `Scope`, `Grant::covers`, `GrantLedger`, `decide`) | Scope binds workspace/session/requester/expiry/policy version; replay ledger is process-local and capped at 4096; digest is FNV-1a-64 binding-only. | Replace process-local replay state with durable approval/execution state. Unify digest algorithm/version across security, tools, and storage before wiring. |
| `crates/security/src/tool_authorize.rs:122-161,188-195` (`ToolAuthorizer`, `run_if_allowed`) | Human gate/deny does not invoke the effect closure; audit text is redacted/bounded. | Preserve no-side-effect construction in the real dispatch path. |
| `crates/tools/src/app_services.rs:310-355` (`approval_digest`, `check_binding`) | Separate 32-byte tool digest; binds tool/args/scope, not transport ID/timeout. | Do not silently treat this digest as `OperationDigest`; use one canonical versioned digest and reject algorithm/version drift. |
| `crates/storage/src/approvals_v2.rs:18-22,26-145` (`ApprovalsV2`) | Evidence states 0/1/2/3/4; resolve is pending-only CAS; expiry sweep clamps to 500; mandatory approval requires human client bytes. | Coordinator must reserve state 1 for human allow-once and state 4 for post-claim consumption. Current `resolve` accepting target 4 must not be used as a human decision shortcut. |
| `crates/storage/schema/v2/workspace.sql:144-220` (`executions`, `provider_attempts`, `tool_calls`) | Durable running/complete/failed/uncertain/cancelled states; single-owner index prevents concurrent live execution; binding columns are immutable. | Approval resume must claim one execution/tool row atomically. Uncertain rows require reconciliation, never blind replay. |
| `crates/storage/schema/v2/workspace.sql:222-249` (`approvals`, `approval_resources`) | Approval binds session/tool/intent hash/policy generation/action/human client/expiry; no workspace, textual requester, decision kind, or dispatch lease column. | Schema owner must add or formally map workspace/requester/human principal, cancellation/decision kind, and dispatch/reconciliation identity. Do not fake these with unrelated columns. |
| `crates/storage/schema/v2/workspace.sql:284-300` (`operation_receipts`, `event_outbox`) | Receipts are bounded/expiring; outbox payload is at most 4096 JSON bytes. | Receipts/outbox are evidence and deduplication aids, not substitutes for execution ownership or uncertain-effect reconciliation. |
| `crates/storage/src/execution_v2.rs:54-81,113-209` | Conditional execution/attempt/tool transitions; uncertain attempts/tools remain open. | Persist dispatch claim and result; recover by bounded indexed scan. |
| `crates/storage/src/retention_v2.rs:58-105` | Resolved approvals and receipts have bounded retention sweeps. | Do not delete an approval before its execution/receipt/reconciliation contract is safe; coordinate retention with recovery. |
| `crates/server/src/lib.rs:271-334` (`router_with_auth`) | Bearer middleware protects `/api/*`; no approval decision/resume route exists. | Add authenticated human decision and resume routes through an integration-owned route registration. |
| `crates/server/src/lib.rs:1126-1437` (`create_turn_stream`, `TurnStreamState`) | Fresh ephemeral broker; `RequireHuman` becomes a terminal tool error. | Replace terminal handling with durable request/continuation; no approval inference from stream state. |
| `crates/server/src/turn_service.rs:303-466` (`Turn`) | `approve` only changes in-memory phase; `Uncertain` is terminal and retry is denied. | Use as lifecycle vocabulary only; bind it to durable coordinator state and owner generation. |
| `crates/server/src/daemon_auth.rs:150-176` (`require_bearer`) | API bearer authenticates the daemon request, not a human principal. | Decision route must require a separate authenticated human/client identity and local/remote policy checks. |
| `crates/server/src/event_bus.rs:12-18,68-116` (`ServerEvent`, `EventBus`) | Bounded transient bus; `PermissionRequested(String)` lacks durable bindings. | Emit durable outbox events first; bus carries notifications only. |
| `crates/server/src/remote_approvals.rs:18-21,198-338` | In-memory reference model: 1024 pending cap, one decision winner, binding checks, revocation, single-use consume. | Reuse semantics, not storage/lifetime; local implementation needs authenticated durable coordinator. |
| `crates/contracts/src/lib.rs:40-87` | `ApprovalId`, `SessionId`, `AgentId` are UUID IDs. | Use opaque IDs in wire/storage bindings; do not expose raw credentials. |

## Target call graph

### Current live path

```text
POST /api/sessions/{id}/turns/stream
  -> create_turn_stream (`crates/server/src/lib.rs:1126`)
  -> fresh in-memory PermissionBroker (`:1253-1255`)
  -> broker.authorize (`crates/server/src/lib.rs:1390-1437`)
  -> RequireHuman
  -> terminal `shell_startup_errors` / stream failure
```

No durable approval row, authenticated decision endpoint, continuation, or restart path exists. `Turn::approve` at `crates/server/src/turn_service.rs:315-326` only changes an in-memory phase.

### Required path

```text
authenticated approval request
  -> validate human principal, workspace, session, requester
  -> canonicalize exact operation and compute versioned digest
  -> broker baseline decision
  -> transactionally insert approval(0) + outbox(requested)
  -> bounded wait registration owned by daemon runtime

human decision
  -> authenticated decision route
  -> reload row and compare every binding field
  -> immediate broker revalidation
  -> CAS pending(0) -> allowed_once(1) or denied(2)
  -> append outbox(decision)

resume
  -> authenticate caller and owner/generation
  -> transactionally claim execution/tool dispatch
  -> approval(1) -> consumed(4) in same transaction as dispatch intent
  -> revalidate live operation, scope, policy, expiry, identity
  -> execute through brokered tool path
  -> persist result/uncertain state + receipt + outbox
  -> continue provider loop exactly once

restart
  -> bounded indexed recovery scan
  -> expire pending rows
  -> resume unconsumed continuations only
  -> reconcile consumed/dispatched unknown results
  -> never blind-replay ambiguous effects
```

## Approval binding contract

Every request and decision must bind the following fields:

1. Opaque `ApprovalId`.
2. Operation kind, tool name, canonical arguments, normalized path/scope, and exact digest.
3. Digest algorithm/version and canonical-encoding version.
4. Workspace ID and canonical workspace path.
5. `SessionId`, execution ID, tool-call ID, assistant-message/owner ID.
6. Requester identity supplied by the authenticated request context.
7. Authenticated human principal and stable human client/device identity.
8. Policy version/generation.
9. Creation time, expiry time, local-only/human-only flags.
10. Decision actor/time/reason, dispatch claim, receipt, and recovery generation.

A human decision is valid only when all fields match the stored request and current policy. A changed path, argument, tool, scope, requester, workspace, session, policy, identity, or expiry fails closed before any effect.

The existing FNV implementations are not interchangeable:

- `app_policy::OperationDigest` is 16 hex characters (`:35-49`).
- `tools::app_services::approval_digest` is 32 bytes (`:310-337`).
- Storage accepts a 32-byte `intent_hash` (`approvals_v2.rs:26-31`).

Integration must choose and version one canonical digest. Until then, cross-layer approval is not admissible. A cryptographic digest upgrade requires dependency/integration approval; current comments explicitly label FNV binding-only.

## Human authority and security

- API bearer authentication is necessary but insufficient; it does not identify a human (`daemon_auth.rs:150-176`).
- Human-only decisions require authenticated human presence/authority, stable client identity, and the request's original requester/workspace/session binding.
- Wildcard `*`, project config, model output, prompt text, or an agent-generated approval ID never grants authority.
- `Decision::Deny` is immutable and cannot be lifted by a grant (`app_policy.rs:288-339`).
- `RequireHuman` may be satisfied only by a fresh, matching, unconsumed decision.
- Do not log raw arguments, paths containing secrets, bearer tokens, or environment values. Events carry bounded redacted summaries and digests only.

## State machine

### Approval evidence states

| State | Meaning | Legal next action |
|---:|---|---|
| `0` pending | Awaiting human decision | `1`, `2`, or expiry `3` |
| `1` allowed_once | Decision won; no dispatch claim yet | `4` only through atomic resume claim |
| `2` denied | Human rejection/cancellation decision | none |
| `3` expired | Expiry sweep won | none |
| `4` consumed | Dispatch claim committed | none; replay returns existing outcome/reconciliation |

The current v2 schema has no separate cancelled approval state. Pending cancellation must therefore either use a proposed `decision_kind='cancel'` representation or receive an approved schema extension. It must not be silently represented as a consumed approval. The current storage `resolve` API accepts `4`; the coordinator must prohibit direct pending-to-4 human decisions.

### Execution/tool states

- Execution `0 queued -> 1 running -> 2 complete | 3 failed | 5 cancelled`.
- Execution `1/4 -> 4 uncertain` when dispatch/effect is unknown.
- Tool `0 planned -> 1 dispatched -> 2 success | 3 failure | 4 cancelled`; tool `5` means uncertain.
- Approval consumption, execution dispatch claim, and dispatch idempotency key commit atomically before external dispatch.

## Race and exactly-once matrix

| Race/crash | Required result | Side-effect rule |
|---|---|---|
| Two human approvers | One pending CAS winner; loser gets already-decided/conflict. | Loser never executes. |
| Approve vs reject | One CAS winner. | No implicit preference; no effect from loser. |
| Approve vs expiry | CAS determines winner. | Expired row cannot dispatch. |
| Cancel vs approve | One pending CAS winner; resume rechecks cancellation/owner generation. | No dispatch after cancellation wins. |
| Duplicate decision/resume | One dispatch claim; duplicate returns durable status/replay conflict. | At most one external dispatch. |
| Two clients | One session/execution owner; clients observe same continuation. | No private runtime or duplicate provider/tool work. |
| Policy bump/version drift | Current broker rejects stale grant. | No effect; do not burn a valid unconsumed decision. |
| Operation/scope drift | Digest/binding mismatch. | No effect; require new human decision. |
| Crash before claim commit | Transaction rollback; pending/allowed state remains recoverable. | No dispatch assumed. |
| Crash after claim, before effect | Mark uncertain unless a durable proof says no spawn occurred. | Never blindly replay. |
| Crash after effect, before result | Tool/execution uncertain; receipt/reconcile. | No claim of successful completion. |
| Restart with consumed/dispatched row | Reconstruct from durable state and receipt. | Never dispatch again solely from an approval row. |
| Ambiguous non-idempotent effect | Remain uncertain pending human/operator reconciliation or an approved idempotent recovery. | Exactly-once completion is not asserted. |

The contract guarantees **one approval decision**, **one dispatch claim**, and **no automatic replay of ambiguous effects**. It does not promise exactly-once completion for an external side effect whose result cannot be observed; that would be an unsafe claim.

## Persistence and restart matrix

| Durable point | Required record | Recovery behavior |
|---|---|---|
| Request commit | Approval `0`, exact binding, expiry, owner generation, outbox requested event in one transaction | Reload pending; expire if due. |
| Decision commit | State `1/2`, actor/time, outbox decision event in one transaction | `1` may resume; `2` cannot. |
| Resume claim commit | Approval `4`, execution/tool dispatch claim, dispatch key, owner generation, outbox claim event in one transaction | Never dispatch again; reconcile result. |
| Result commit | Tool/execution terminal or uncertain state, payload/receipt, outbox result event in one transaction | Rebuild transcript/status from durable state. |
| Unknown result | Execution/tool `uncertain` plus reconciliation key | Human/operator reconciliation; no blind retry. |
| Retention | Retain approval until dispatch/receipt/reconciliation policy permits deletion | Bounded sweeps must not erase recovery evidence prematurely. |

Restart recovery must use indexed, bounded queries. It must not scan an unbounded table, restore an orphaned wait with no owner, or spawn a detached task. Each pending wait has an owner, cancellation token, deadline, and a durable row; an in-memory wait is only a wake-up hint.

## Resource and lifetime contract

Existing caps retained:

- Broker audit: `1024` entries (`security/src/lib.rs:153`).
- Tool audit: `256` entries, redacted text at most `512` bytes (`security/src/tool_authorize.rs:26-30`).
- Replay buffer: `512` events (`server/src/event_cursor.rs:32-33`).
- Tool calls per assistant message: `128` (`contracts/src/lib.rs:25-27`).
- Approval/receipt/outbox sweep: at most `500` rows per call (`approvals_v2.rs:14-16`, `retention_v2.rs:15,58-105`).
- Approval resources: ordinal `0..=255`, each `1..=4096` bytes (`approvals_v2.rs:14-16,147-168`).
- Outbox payload: at most `4096` JSON bytes (`workspace.sql:293-300`).
- Read/tool output: current read path is `64 KiB` bounded; approval work must preserve the same byte budget.

Proposed coordinator caps (must be enforced, not advisory):

- `1024` pending approvals per daemon, matching the existing remote model cap (`remote_approvals.rs:18-21`).
- `16` pending approvals per session; one active approval per tool call/execution.
- `1024` owned wait registrations; overflow returns a typed capacity error.
- `4096` bytes maximum decision/event body; raw operation content is never retained in the event.
- One active execution owner per session; no detached waits or per-agent orchestration processes.

## Failure contract

Every denial, rejection, expiry, cancellation, stale binding, replay, restart ambiguity, and capacity failure must return a typed, non-secret outcome. A failed approval check must not invoke the tool closure, spawn a process, write a file, or consume a different approval. Cancellation before dispatch is terminal without execution. Cancellation after dispatch begins follows uncertain-effect rules.

## Future RED plan and command

Independent RED author proposes only:

```text
crates/server/tests/app012_approval_resume_red.rs
```

The test must use Rust-local deadlines, bounded channels, and disposable workspace fixtures. For example, each network/dispatch await is wrapped by `tokio::time::timeout(Duration::from_secs(2), ...)`; no shell deadline utility is used. Cases cover approve, reject, expiry, cancellation, duplicate decision, concurrent decision, stale binding, restart, ambiguous effect, replay, secret redaction, and resource overflow. Denied/rejected/expired cases assert absence of side effects, not only error text.

Future macOS-compatible command (no shell `timeout`/`gtimeout` wrapper):

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_approval_resume_red -- --test-threads=1
```

Current expected RED: the approval/resume journey and target test are absent. This research lane runs no Cargo, database, or heavy process.

## Ownership and integration proposal

1. **Storage/schema owner:** approve a migration/API for requester/workspace/human principal, cancellation/decision kind, dispatch claim, and reconciliation identity; add atomic approval/execution/tool/outbox transition.
2. **Security owner:** select/version one canonical operation digest; bridge durable approval evidence to `PermissionBroker`; make consumed/replay state durable.
3. **Server/runtime owner:** add authenticated human decision/resume routes, principal extraction, bounded continuation registry, restart recovery, and durable outbox publication.
4. **Tools owner:** bind canonical digest, enforce cancellation/deadline, persist dispatch claim/result, and route all resume execution through the real brokered executor.
5. **Integration owner:** wire one daemon-owned runtime, one session execution owner, and second-client observation; preserve parent APP-012 open status until the integrated journey is GREEN.
6. **Independent test/verifier:** author/freeze the future RED, run it on the integrated revision, and reject missing wiring or ambiguous replay.

Shared route/schema/runtime files are not edited by this lane. No product implementation or acceptance is claimed.

## Research validation

Required commands and results at the research base:

```sh
rtk git grep -n 'RequireHuman\|Grant\|approval\|pending\|resume' -- crates
# 1164 matching lines across 134 files

rtk shasum -a 256 crates/server/tests/app012_tool_journey_red.rs
# 945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50

rtk git diff --check
# exit 0
```

Resource observation: research commands only; no Cargo, SQLite, browser, daemon, or heavy process. Future test-local bounds are mandatory because macOS has no `timeout`/`gtimeout` assumption.

Canonical repository guard was also attempted: `rtk python3 tools/validate_repository.py` reached `validate_repository: FAIL backlog exhaustion` with 51 pre-existing backlog/ownership errors. It changed no files. This research artifact does not override that global blocker or claim acceptance.

## Remaining unknowns and stop conditions

- Schema authority for cancellation/decision kind, workspace/requester/human principal, dispatch lease, and reconciliation key is unresolved.
- Digest algorithm/version unification is unresolved; current security/tools/storage digests differ.
- Current `GrantLedger` is not durable and cannot survive restart.
- Current live `RequireHuman` path is terminal and not resumable.
- Current `ServerEvent::PermissionRequested` is not a durable request record.
- No product route/runtime wiring exists in this research branch.
- If implementation encounters unresolved schema authority, stop and submit an integration proposal. Never widen scope, weaken a frozen test, auto-approve, or replay an ambiguous effect.
