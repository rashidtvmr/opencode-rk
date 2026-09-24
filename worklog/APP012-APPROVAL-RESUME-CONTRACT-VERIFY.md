# APP012-APPROVAL-RESUME-CONTRACT-VERIFY

## Claim

- Task: `APP012-APPROVAL-RESUME-CONTRACT-VERIFY`
- Session: `ses_f2ccb8a9fffeioF1m83OP7KByU`
- Branch: `plan/APP012-APPROVAL-RESUME`
- Candidate revision: `3f7405eed88dc568ad601249c8f2f74e44653465`
- Contract reviewed: `worklog/APP012-APPROVAL-RESUME-CONTRACT.md`
- Scope: verification only. No product, schema, route, runtime, test, or contract edits.
- Frozen APP-012 read test SHA-256: `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`

## Verdict

**REVISE BEFORE RED.** The contract is a useful source-backed design boundary and its
external-effect honesty is strong, but it is not admissible unchanged. Security and
state-boundary corrections below are required before RED freeze. RED authoring is
otherwise feasible after those corrections; this lane does not claim product
acceptance or implementation.

## Source-grounding matrix

| Element | Result | Evidence and correction |
|---|---|---|
| Broker baseline and mandatory human gate | PASS | `crates/security/src/lib.rs:61-77,206-240,256-300`; `Deny` and `RequireHuman` survive permission rules. Preserve baseline-first ordering. |
| Grant scope and replay | PARTIAL | `crates/security/src/app_policy.rs:68-181,245-339`; scope binds workspace/session/requester/expiry/policy and the ledger is bounded, but `GrantLedger` is process-local. Durable coordinator must replace it. |
| Human-only side-effect boundary | PASS | `crates/security/src/tool_authorize.rs:188-195`; `run_if_allowed` does not invoke the effect closure for deny or human gate. Future resume must retain this boundary. |
| Approval states and pending CAS | PARTIAL | `crates/storage/src/approvals_v2.rs:18-23,82-145`; `resolve` is a pending-only CAS, but it accepts pending -> 4. Coordinator must reserve 4 for atomic dispatch claim, as the contract states. |
| Approval state schema | FAIL | `crates/storage/schema/v2/workspace.sql:222-249` has no decision kind, workspace/requester/principal binding, owner generation, dispatch key/lease, or reconciliation key. Contract correctly lists the gap, but RED must not assume these fields exist. |
| Execution/tool state | PARTIAL | `workspace.sql:144-220`; `crates/storage/src/execution_v2.rs:54-209`. State CAS helpers exist, but no approval-consumption plus dispatch-claim transaction and no owner-generation predicate on transitions. Add both to required schema/API work. |
| Receipts and outbox | PARTIAL | `workspace.sql:284-300`; `crates/storage/src/writer_v2.rs:355-382`. Both are durable aids, not effect ownership. Outbox SQL cap is 4096 bytes; writer-side `MAX_EVENT_PAYLOAD_BYTES` is 64 KiB, so coordinator must validate the stricter 4096-byte bound before insert. |
| Approval retention | FAIL | `crates/storage/src/retention_v2.rs:84-105` sweeps states 1, 2, and 4 only. State 3 expired rows are not swept. Add an explicit expired-row retention/reconciliation rule before claiming bounded approval growth. |
| Daemon authentication | PASS | `crates/server/src/daemon_auth.rs:150-175`; bearer authenticates the daemon request only. It is not human authority. |
| Current routes and stream | PASS | `crates/server/src/lib.rs:271-335,1126-1255,1429-1437,1639-1645`; no approval decision/resume route; fresh broker; human requirement becomes a terminal tool output. |
| Event notification | PASS | `crates/server/src/event_bus.rs:11-35,78-116`; `PermissionRequested(String)` is bounded transient notification, not a durable request. |
| Turn lifecycle vocabulary | PASS | `crates/server/src/turn_service.rs:65-80,303-357,423-465`; in-memory approval/uncertain semantics support the proposed vocabulary only, not restart durability. |
| Remote approval reference | PARTIAL | `crates/server/src/remote_approvals.rs:18-21,198-338`; binding, one decision, revoke, consume, and no blind replay are useful semantics. It caps `g.requests.len()`, not pending rows specifically, and all state is process-local. Correct the source characterization. |
| Opaque IDs | PARTIAL | `crates/contracts/src/lib.rs:40-87`; UUID IDs exist. `ApprovalsV2::request` returns SQLite `pk` (`approvals_v2.rs:26-79`), so the integration must define the mapping and never expose a row integer as the wire approval ID. |

## Security and binding findings

1. **Local-only enforcement is not on the canonical path.** `Grant::local_only`
   exists (`app_policy.rs:125-134`), but `decide` (`:288-339`) does not check it;
   `enforce_local_only` is a separate caller obligation (`:341-355`), and
   `ToolAuthorizer::authorize_with_grant` (`tool_authorize.rs:138-161`) does not
   call it. Required correction: canonical resume must authenticate origin and
   reject a local-only grant before allow, with a test proving no effect.

2. **Expiry boundary differs.** `Grant::covers` rejects only `now > expires_at`
   (`app_policy.rs:160-162`); remote approval uses the same `>` comparison
   (`remote_approvals.rs:246-247,324-329`), while storage expiry sweep uses
   `expires_at_us <= now_us` (`approvals_v2.rs:136-143`). Required correction:
   choose one boundary, preferably `now >= expiry`, and apply it to grant checks,
   remote checks, resume checks, and sweep. A row due at the exact expiry must not
   dispatch.

3. **Scope construction lacks future/max-TTL validation.** `Scope::new` validates
   nonempty/non-wildcard workspace and requester plus requester bytes
   (`app_policy.rs:80-121`), but receives no current time and does not require a
   future bounded expiry. Coordinator/storage validation must enforce a positive,
   bounded window.

4. **Human identity is not currently authenticated or fully bound.** The daemon
   bearer middleware only validates the daemon token (`daemon_auth.rs:159-174`).
   `human_client_id` is an optional 16-byte column (`workspace.sql:232-239`), and
   `ApprovalsV2::resolve` only requires it when `mandatory_human=1`
   (`approvals_v2.rs:92-127`). Required correction: decision route must derive a
   trusted human principal and stable client/device identity from an approved
   authentication mechanism; request body, model output, bearer alone, or project
   config cannot supply authority.

5. **Canonical digest is unresolved and current encoding is not a sufficient
   contract.** Security emits 16 hex FNV bytes over a delimiter string
   (`app_policy.rs:35-49,371-426`); tools emits 32 FNV-derived bytes with a
   different `par005-v1` encoding (`app_services.rs:310-337`); storage requires a
   32-byte hash (`workspace.sql:228`, `tool_calls:196`). Required correction:
   specify one algorithm, encoding version, and unambiguous length-prefixed
   encoding, then use it identically in security/tools/storage. FNV comments call
   the current value binding-only; do not advertise collision-resistant identity.

6. **Concrete operation revalidation must occur at dispatch.** The live server
   first authorizes generic `OperationIntent::Tool` (`lib.rs:1388-1437,1607-1645`).
   `ToolExecutor::execute_authorized` only takes the concrete brokered file path
   for `read`; other tool names fall through to `execute` (`tools/src/executor.rs:
   136-147`). Required correction: resumed execution must reconstruct the exact
   concrete operation, check canonical digest and broker immediately before the
   effect, and prove that non-read tools cannot bypass the broker.

## State, race, and effect matrix

| Scenario | Required contract result | Verification |
|---|---|---|
| Request | Pending 0 plus exact binding and requested outbox event in one transaction | Required; current `ApprovalsV2::request` inserts only the row and has no outbox transaction. |
| Two decisions | One pending CAS winner; loser conflict/status; no effect | Source CAS supports this (`approvals_v2.rs:109-127`); durable decision actor/principal still missing. |
| Approve versus reject/expiry/cancel | One CAS winner; exact expiry boundary; cancellation state explicit | Cancellation is absent from current schema. Do not encode cancel as consumed. |
| Resume | Authenticate owner/generation; revalidate all bindings and broker; claim once | Required; current execution/tool helpers do not provide the combined claim. |
| Claim transaction | Approval 1 -> 4, execution/tool dispatch claim, idempotency key, and outbox claim event commit before external effect | Missing schema/API. This is a hard RED/implementation prerequisite. |
| Duplicate resume | Return durable outcome or reconciliation status; never dispatch solely from consumed approval | Required and honest. RED must distinguish duplicate replay from uncertain result. |
| Crash before claim commit | Roll back; no dispatch assumed | Valid SQLite transaction expectation, but future test must use disposable DB and injected failure. |
| Crash after claim before effect | Uncertain unless durable proof says no dispatch | Correct at-most-one claim boundary; never claim exactly-once external completion. |
| Crash after effect before result | Tool/execution uncertain plus reconciliation key | Required; current schema has uncertain states but no reconciliation identity. |
| Policy/scope/digest drift | No effect; stale decision remains unconsumed or is terminally invalidated by explicit rule | Contract needs exact durable result for stale decision. Avoid silently burning approval. |
| Restart | Indexed bounded recovery; pending expiry; unconsumed continuation only; consumed/unknown reconciliation; no blind replay | Required. Existing recovery indexes are only execution/attempt/tool indexes (`workspace.sql:165,185,212`). |
| Two clients | One daemon/session execution owner; second client observes durable state | Runtime has bounded single-owner concepts (`runtime_wiring.rs:14-20,155-157`), but approval continuation is not wired. |

The contract's distinction is correct: guarantee one decision and one dispatch
claim, not exactly-once completion of a non-idempotent external effect whose result
is unobservable. Preserve this wording.

## State-code matrix

| Domain | Current codes | Contract handling | Result |
|---|---|---|---|
| Approval | 0 pending, 1 allowed-once, 2 denied, 3 expired, 4 consumed | Human decision may produce 1 or 2; sweep produces 3; only atomic resume may produce 4 | PASS after exact-expiry and state-3 retention corrections |
| Execution | 0 queued, 1 running, 2 complete, 3 failed, 4 uncertain, 5 cancelled | Contract lists queued/running/terminal/uncertain semantics | PASS; require owner-generation CAS and claim identity |
| Provider attempt | 0 prepared, 1 dispatched, 2 success, 3 failure, 4 uncertain, 5 abandoned | Contract should name 5 abandoned if it tests attempt recovery | PARTIAL; source `workspace.sql:170-185` confirms code 5 is abandoned |
| Tool | 0 planned, 1 dispatched, 2 success, 3 failure, 4 cancelled, 5 uncertain | Contract lists these states | PASS; current `finish_tool` allows any nonterminal state and has no owner predicate |

## Resource and lifetime matrix

| Resource | Source bound | Contract disposition |
|---|---:|---|
| Broker audit | 1024, `security/src/lib.rs:153,228-240` | PASS |
| Tool authorization audit | 256 entries, 512 bytes text, `security/src/tool_authorize.rs:26-30,164-184` | PASS |
| Replay event buffer | 512, `server/src/event_cursor.rs:16,33,163-196` | PASS |
| Tool calls per assistant message | 128, `contracts/src/lib.rs:25-27` | PASS |
| Approval/resource sweep | 500 rows, `approvals_v2.rs:14-16,130-145`; retention `:15,62-105` | PASS for bounded calls; expired-state retention gap remains |
| Approval resource | 256 ordinals, 4096 bytes, `approvals_v2.rs:147-168` | PASS |
| Outbox payload | SQL 4096 bytes, `workspace.sql:293-300` | PASS as target; writer precheck must match |
| Read output | 64 KiB, `tools/src/file_ops.rs:13,229-249`; executor clamp `executor.rs:181-184` | PASS |
| Pending approvals | Remote reference constant 1024, `remote_approvals.rs:18-21` | PARTIAL: implementation caps all retained requests, not pending only |
| Per-session approvals/waits | Proposed 16/1024 | UNPROVEN proposal; RED must assert typed overflow only after owner defines schema/runtime cap |
| Queue/process lifetime | One owner, bounded queues, no detached waits | PASS as invariant; no approval coordinator exists to verify |

## RED readiness matrix

| RED case | Required assertion |
|---|---|
| Approve | Matching authenticated principal, exact digest/scope, one continuation, one dispatch claim |
| Reject | Terminal non-success; zero process/file/provider effect |
| Exact expiry | `now == expiry` fails closed; no dispatch |
| Cancellation | Explicit cancel state; no dispatch before claim; post-claim follows uncertain rules |
| Duplicate decision | One winner; second typed conflict/status; no second outbox decision |
| Concurrent decision | Barrier race; exactly one CAS winner |
| Stale binding | Path/tool/args/scope/requester/workspace/session drift; no effect; approval not silently retargeted |
| Policy bump | Broker revalidation rejects stale generation; no effect; durable unconsumed outcome defined |
| Local-only/remote | Remote or bearer-only attempt denied; no effect |
| Restart pending | Reload, bounded expiry/recovery, owner registration; no orphan detached waiter |
| Restart consumed | No automatic dispatch; status/reconciliation returned |
| Ambiguous effect | Uncertain state; no blind replay; explicit reconciliation key |
| Replay | Duplicate resume returns durable result/reconciliation, not a second dispatch |
| Secret redaction | Logs/outbox/errors contain no raw args, paths, tokens, environment values |
| Resource overflow | 1024 daemon / 16 session / wait cap behavior typed and side-effect-free, once those caps are authoritative |

The proposed command is macOS-portable and has required dev dependencies
(`crates/server/Cargo.toml:26-29`):

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_approval_resume_red -- --test-threads=1
```

The target is currently absent, so current expected RED is a missing test target,
not a valid compiling RED. The independent RED author must add the target, run it,
capture a compiling behavioral failure, freeze its hash, then hand it to a separate
implementer/verifier. Rust-local `tokio::time::timeout` and disposable fixtures
remain mandatory; no shell `timeout` assumption.

## Required corrections before RED freeze

1. Harmonize expiry as `now >= expires_at`; enforce future and maximum TTL.
2. Require canonical authenticated human principal plus stable client/device ID;
   bearer remains transport authentication only.
3. Make local-only enforcement mandatory in the canonical coordinator path.
4. Define one versioned, unambiguous digest encoding and algorithm; map storage
   `intent_hash` and wire `ApprovalId` explicitly.
5. Add schema/API owner decision for workspace, requester, principal, decision
   kind/cancellation, owner generation, dispatch claim/key, and reconciliation key.
6. Define one transaction covering approval consumption, execution/tool claim,
   idempotency key, and claim outbox event; external effect remains outside it.
7. Add retention/reconciliation for expired state 3; never delete recovery evidence
   before its policy permits.
8. Require concrete broker authorization on resumed tool dispatch, not generic tool
   authorization or an unbrokered fallback.
9. Correct remote-approval cap wording and add bounded outbox validation at 4096.

## Validation

- `git rev-parse HEAD` -> `3f7405eed88dc568ad601249c8f2f74e44653465`.
- `git grep -n 'RequireHuman\|Grant\|approval\|pending\|resume' -- crates` -> 1164 matching lines across 134 files.
- `shasum -a 256 crates/server/tests/app012_tool_journey_red.rs` -> frozen hash `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.
- `git diff --check` -> exit 0 before this worklog edit.
- `python3 tools/convergence_gate.py` -> blocked by 91 pre-existing ledger findings; no source change made.
- No Cargo, SQLite, browser, daemon, or live-provider process run in this verification lane.

## Remaining unknowns

- Contract owner must publish the corrections above before RED freeze.
- Schema authority and migration/API design remain unresolved.
- Human authentication mechanism and local/remote principal proof remain unresolved.
- No current route/coordinator/continuation/recovery implementation exists.
- No exactly-once external effect claim is admissible without an observable idempotency/reconciliation protocol.
