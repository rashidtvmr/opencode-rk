# APP012 approval/resume correction contract

## Claim, authority, and boundary

- Task: `APP012-APPROVAL-RESUME-CORRECTION`
- Task type: research
- Session: `ses_f2cbec9fbffeE2bN7S0GAgxxmn`
- Branch: `plan/APP012-APPROVAL-RESUME-CORRECTION`
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/plan-app012-approval-resume-correction`
- Owned artifact: this file and this task's ledger row only.
- Scope: correction contract and independent-verifier checklist. No product,
  schema, migration, route, runtime, dependency, test, database, or frozen-file
  edits.
- Candidate source revision: `1f53b01176248c1b9dc364c8add06039d4d800a1`.
- Verifier artifact: `worklog/APP012-APPROVAL-RESUME-CONTRACT-VERIFY.md`, verdict
  `REVISE BEFORE RED`. It is evidence, not authority to weaken policy.
- Frozen APP-012 read test:
  `crates/server/tests/app012_tool_journey_red.rs`.
- Frozen SHA-256:
  `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.
- No RED authorization or product acceptance is claimed. A future RED author,
  implementer, integrator, and verifier remain separate roles.

Authoritative policy: `AGENTS.md`, `PLAN.md`, `docs/TDD.md`,
`docs/SECURITY.md`, `docs/CONVERGENCE.md`, and verified source at the candidate
revision. Original contract and verifier notes are untrusted task artifacts used
only as findings to resolve. Existing protected-path, restart, and keyring
decisions remain unchanged by this research artifact.

## Contract vocabulary and invariant baseline

The durable coordinator owns approval evidence, continuation ownership, and
recovery state. Approval evidence is never a capability. Immediately before an
external effect, the coordinator reconstructs the exact operation, reruns the
trusted broker, checks origin, principal, scope, policy generation, expiry, and
digest, then invokes the concrete brokered executor only on `Allow`.

Guarantees are limited to one durable human decision, one durable dispatch claim,
no duplicate dispatch claim, no blind replay, and explicit reconciliation for an
unknown external result. Exactly-once completion of a non-idempotent external
effect is not claimed when its result cannot be observed.

Approval states remain:

| Code | Meaning | Legal transition |
|---:|---|---|
| 0 | pending | 1, 2, or expiry to 3 |
| 1 | allowed once | 4 only through the atomic resume claim |
| 2 | denied, including explicit cancellation when `decision_kind` distinguishes it | terminal |
| 3 | expired | terminal; retained under the rule below |
| 4 | consumed/dispatch claim committed | terminal approval evidence; result may be reconciling |

Human decision cannot directly set state 4. Existing `ApprovalsV2::resolve` may
accept 4 (`crates/storage/src/approvals_v2.rs:82-127`), so the future coordinator
must prohibit that API path or replace it with typed transition APIs.

## Nine required corrections

### 1. Expiry is inclusive everywhere

**Current evidence.** `Grant::covers` rejects only `expected.now > expires_at`
(`crates/security/src/app_policy.rs:152-162`). Remote approval checks use `>`
(`crates/server/src/remote_approvals.rs:218-247,283-329`). Storage sweep uses
`expires_at_us <= now_us` (`crates/storage/src/approvals_v2.rs:130-145`).
`Scope::new` has no current-time or maximum-TTL validation
(`crates/security/src/app_policy.rs:80-121`).

**Normative boundary.** One rule applies to request, decision, grant, resume,
remote, and sweep: an approval is usable only when `now < expires_at`; at
`now == expires_at` it is expired. Expiration transition uses `expires_at <= now`.
Request creation requires `created_at < expires_at <= created_at +
MAX_APPROVAL_TTL`; the coordinator constant is 15 minutes
(`900_000_000` microseconds). Reject nonpositive, overflowed, or over-limit
windows. All comparisons use the injected monotonic-to-storage time sample for
one operation; tests do not use an uncontrolled wall clock.

**Persistence/lifetime/failure.** A pending row due at the boundary is atomically
changed to state 3 with `resolved_at`; it cannot dispatch. An allowed row due at
the boundary fails resume without consuming it, then is expired by the bounded
sweep. Expiry race versus decision is one CAS winner. Clock or timestamp
validation failure is typed and side-effect-free.

**Owner and dependencies.** Security owner defines the shared predicate;
storage owner applies it to request/sweep; server coordinator applies it to
decision/resume; remote owner applies it to remote review. Depends on the
approval schema fields in correction 5.

**Resource limits.** `MAX_APPROVAL_TTL = 15 minutes`; sweep is at most 500 rows
per call, matching `crates/storage/src/approvals_v2.rs:14-16`.

**Future compiling RED assertions.**

- approval at `expires_at - 1us` can continue only after all other checks pass;
- approval at `expires_at` returns typed `Expired`, invokes no effect, and does
  not create a dispatch claim;
- approval at `expires_at + 1us` has the same result;
- request with `expires_at == created_at`, over-15-minute TTL, or integer overflow
  fails before a row or outbox event exists;
- an approve-versus-expiry barrier yields exactly one durable winner and never a
  dispatch at the boundary.

### 2. Human authority is authenticated and distinct from daemon bearer auth

**Current evidence.** `require_bearer` validates only the daemon bearer for
`/api/*` (`crates/server/src/daemon_auth.rs:150-175`). The current approval table
has optional `human_client_id`, and `ApprovalsV2::resolve` only checks presence
for mandatory approvals (`crates/storage/schema/v2/workspace.sql:222-240`,
`crates/storage/src/approvals_v2.rs:82-127`). Existing remote model binds device,
workspace, requester, expiry, and policy (`crates/server/src/remote_approvals.rs:30-64`)
but is process-local (`:198-215`).

**Normative transport/auth contract.** Bearer remains transport authentication
only. The decision and resume context must contain a trusted
`HumanPrincipal { principal_id: UUID, client_id: UUID, method, origin }` created
by an authenticated principal verifier, never copied from a body, model output,
project config, or bearer token. The concrete approved mechanisms are:

1. Local client: daemon transport must prove local IPC peer identity using the
   platform peer-credential facility (Unix peer credentials on Unix, named-pipe
   peer token on Windows), then verify a client signing key held by the existing
   platform keyring/client credential path. The signed challenge binds principal,
   client, approval ID, digest, and policy generation.
2. Paired remote client: the existing paired-device authentication verifies the
   device signing key and challenge. Remote origin is explicit and can never
   satisfy `local_only`.

The trusted verifier maps the authenticated credential to a stable principal and
client UUID. A client ID supplied without proof is invalid. No new secret storage
decision is made here; the accepted keyring/fallback policy remains the authority
for client credentials.

**Persistence/lifetime/failure.** Store principal ID, client/device ID, auth
method, origin, and decision actor/time with the approval. Principal proof is
short-lived for the request and must be revalidated on decision and resume;
stored IDs are binding evidence, not a reusable bearer credential. Missing,
invalid, revoked, or mismatched proof returns typed `Unauthenticated`, `PrincipalMismatch`,
or `Revoked` with no CAS, effect, or consumption.

**Owner and dependencies.** Server/auth owner owns principal extraction and
challenge verification; storage owner persists UUID bindings; security owner
consumes the trusted context. Depends on correction 5 and the existing accepted
keyring path; no protected-path or restart behavior changes.

**Resource limits.** Principal, client, method, and origin encodings are bounded
to 256 bytes; challenge/signature input is bounded to 4096 bytes; no credential,
token, or raw signature is logged or placed in an outbox body.

**Future compiling RED assertions.**

- valid daemon bearer with absent human proof cannot approve or resume;
- body-only principal, model-provided principal, and bearer-as-principal all fail;
- valid local peer plus valid client signature yields one decision bound to the
  exact principal and client UUID;
- a different authenticated principal or client receives typed mismatch and
  causes no effect;
- revoked client/device cannot decide or resume;
- remote paired proof is accepted only for non-local-only work.

### 3. `local_only` is mandatory on the canonical coordinator path

**Current evidence.** `Grant::local_only` exists
(`crates/security/src/app_policy.rs:125-134`), but `decide` does not check it
(`crates/security/src/app_policy.rs:288-339`). `enforce_local_only` is a separate
optional helper (`:341-356`), and `ToolAuthorizer::authorize_with_grant` does not
call it (`crates/security/src/tool_authorize.rs:138-161`). Remote approval already
rejects local-only requests (`crates/server/src/remote_approvals.rs:240-242`),
but that is not the durable canonical path.

**Normative boundary.** The coordinator receives `TransportOrigin` only from the
trusted auth context. Before a decision CAS or resume claim, it calls the
equivalent of `enforce_local_only(origin.is_local_peer(), grant, decision)` on
the same path that owns state transitions. A local-only grant requires both a
local peer proof and authenticated human principal; a daemon bearer, remote
paired device, request field, or `is_local=true` body flag is insufficient.
Remote local-only requests fail closed before state consumption or dispatch.

**Persistence/lifetime/failure.** Origin and local-only flag are immutable request
bindings. Origin failure is a typed `RemoteOrigin` result; no effect, claim,
approval consumption, or outbox decision is produced. A remote request may remain
pending for a separately eligible local decision, but remote failure cannot alter
the row.

**Owner and dependencies.** Security owner provides the mandatory gate; server
auth/coordinator owns origin context; tools owner must receive only the verified
context. Depends on correction 2 principal proof and correction 5 immutable
bindings.

**Resource limits.** Origin context is a fixed enum plus bounded principal data;
no retained request body or peer credential blob.

**Future compiling RED assertions.**

- remote bearer-only resume of a local-only operation returns `RemoteOrigin`;
- remote paired-device resume returns the same denial;
- local peer proof with a valid principal reaches broker revalidation;
- an adversarial body `local_only=false` cannot change stored policy;
- all denied cases assert zero file/process/provider effects and unchanged claim
  state.

### 4. One versioned canonical digest and explicit UUID mapping

**Current evidence.** Security uses FNV-1a-64 and a NUL-delimited string,
serialized as 16 hex characters (`crates/security/src/app_policy.rs:35-49,371-408`).
Tools uses a different four-lane FNV encoding, `par005-v1`, producing 32 bytes
(`crates/tools/src/app_services.rs:310-337`). Storage requires a 32-byte
`intent_hash` (`crates/storage/schema/v2/workspace.sql:186-199,222-240`). The
storage request API returns `last_insert_rowid()` rather than the UUID in
`approvals.id` (`crates/storage/src/approvals_v2.rs:26-80`); wire `ApprovalId` is
an opaque UUID (`crates/contracts/src/lib.rs:40-87`).

**Normative digest contract.** Use `BLAKE3-256`, represented as exactly 32 raw
bytes, with algorithm/version ID `blake3-256-v1`. The existing workspace already
declares `blake3` (`crates/storage/Cargo.toml:10`); dependency registration across
security/tools is an integration dependency, not a change in this artifact.
The domain is the exact ASCII byte string
`opencode-rk.approval.intent\0v1\0`.

Canonical encoding is a sequence of unsigned little-endian `u32` byte lengths
followed by UTF-8 or raw bytes. It contains, in order:

1. operation kind tag;
2. concrete tool name or file/process/SQL action tag;
3. canonical operation arguments;
4. normalized resource scope, including normalized path or process cwd;
5. workspace UUID and session UUID;
6. requester identity bytes.

Canonical arguments for JSON tools use recursively sorted object keys, no
insignificant whitespace, UTF-8, rejected duplicate keys, and rejected
non-finite numbers. Paths reject NUL, normalize lexical `.` and `..`, and use
slash separators. No delimiter-only encoding is permitted. Policy generation,
expiry, principal, client, and approval UUID remain separate binding columns and
are checked independently; they are not silently omitted from authorization.

Store the raw digest in `approvals.intent_hash` and `tool_calls.intent_hash`,
store `digest_algorithm_version = 1`, and expose the opaque `approvals.id` bytes
as `ApprovalId`. Never expose `pk` or `last_insert_rowid` as a wire approval ID.
Security, tools, storage, and RED fixtures call the same canonical encoder.

**Persistence/lifetime/failure.** Digest algorithm/version is immutable for a
row. Missing, malformed, unknown-version, or mismatched digest returns typed
`DigestMismatch` or `UnsupportedDigestVersion` before any effect. Algorithm
upgrade requires a new version and explicit migration/integration review; no
fallback to FNV is allowed.

**Owner and dependencies.** Security owner publishes the canonical encoder;
tools owner uses it for concrete calls; storage owner maps raw 32-byte values and
UUIDs; contracts/integration owner publishes the version constants. Depends on
correction 5 schema/API work.

**Resource limits.** Canonical input has a fixed coordinator byte budget of 64
KiB before hashing; each field is bounded by its existing tool/path/requester
limits. Digest output is always 32 bytes. Events carry digest bytes or bounded
hex, never canonical raw arguments.

**Future compiling RED assertions.**

- security and tools produce byte-identical digest for the same concrete request;
- one changed path, action, argument, scope, workspace, session, or requester
  changes the digest;
- old FNV hex and `par005-v1` values are rejected;
- storage round-trips 32 raw bytes and wire round-trips the 16-byte UUID;
- returned `ApprovalId` equals `approvals.id`, never SQLite integer `pk`;
- unknown digest version fails closed without a claim or effect.

### 5. Schema/API owns all missing bindings and UUID identity

**Current evidence.** The approvals table currently has session/tool, hash,
policy, action, state, mandatory flag, optional client, and times, but lacks
workspace path, requester text, authenticated principal, decision kind, owner
generation, dispatch nonce, and reconciliation key
(`crates/storage/schema/v2/workspace.sql:222-249`). UUID IDs exist in contracts,
but `ApprovalsV2::request` returns the integer row ID
(`crates/storage/src/approvals_v2.rs:26-80`). Executions already use owner
generation and UUID IDs (`workspace.sql:144-166`); tool rows have immutable
bindings and a 32-byte intent hash (`:186-220`).

**Normative schema/API proposal.** The storage/schema owner must publish an
additive migration/API before RED with these immutable or transaction-owned
fields, exact meanings, and bounded types:

| Field | Type/bound | Meaning |
|---|---|---|
| `approvals.id` | BLOB, 16 bytes, UUID | sole wire `ApprovalId`; generated before insert |
| `workspace_id` | BLOB, 16 bytes | workspace identity, equal to `workspace_state.workspace_id` |
| `workspace_path` | UTF-8 path, max 4096 bytes | canonical path binding; not a secret log field |
| `requester_text` | UTF-8, max 256 bytes | exact authenticated requester binding |
| `human_principal_id` | BLOB, 16 bytes | authenticated human authority identity |
| `human_client_id` | BLOB, 16 bytes | stable client/device identity |
| `decision_kind` | enum `approve`, `deny`, `cancel`, `expire` | distinguishes cancellation from denial/expiry |
| `owner_generation` | nonnegative integer | daemon/session ownership generation checked by CAS |
| `digest_algorithm_version` | integer enum | canonical digest version, currently 1 |
| `dispatch_nonce` | BLOB, 16 bytes, unique when set | one durable external dispatch/idempotency key |
| `reconciliation_key` | BLOB, 16 bytes, unique when set | durable key for unknown-effect reconciliation |
| `decision_actor_id`, `decision_at_us` | UUID/time | authenticated decision evidence |
| `retention_until_us` | time | earliest deletion time after recovery safety |

`execution_id` and `tool_call_id` must map to existing UUID-backed rows through
validated foreign keys or an explicit API mapping. If SQL keeps integer `pk`
internally, every API accepts/returns UUID and resolves it transactionally. The
schema must not fake requester, principal, workspace, or reconciliation data by
overloading `action`, `human_client_id`, or `intent_hash`.

**Persistence/lifetime/failure.** Request transaction persists all immutable
bindings and `decision_kind=none/pending`; decision transaction persists actor,
kind, and time. Claim transaction sets nonce/key and owner generation exactly
once. Mismatched or absent fields fail schema validation before a state change.
UUID collision, foreign-key mismatch, or generation conflict rolls back the
transaction and returns a typed storage error.

**Owner and dependencies.** Storage/schema owner owns DDL and typed storage API;
contracts owner owns `ApprovalId` mapping; security/server/tools consume the API.
This is an integration proposal, not a schema edit in this lane. It depends on
corrections 2, 4, 6, and 7.

**Resource limits.** Existing 16-byte UUID, 32-byte digest, 256-byte requester,
4096-byte path, and 500-row bounded sweep limits remain. No unbounded text is
added to approvals.

**Future compiling RED assertions.**

- request round-trips every listed binding and returns UUID `ApprovalId`;
- changing any workspace, requester, principal, client, generation, or operation
  field causes mismatch and no effect;
- decision kind distinguishes deny, cancel, and expiry;
- duplicate nonce/key and wrong owner generation fail without partial mutation;
- raw integer `pk` is never present as the wire approval identifier;
- schema rejects null/incorrect-length UUID, digest, nonce, or reconciliation key.

### 6. Atomic claim transaction and honest external-effect boundary

**Current evidence.** Approval request inserts only an approval row and returns a
row ID (`crates/storage/src/approvals_v2.rs:26-80`); it does not insert an outbox
request event. Execution/tool transitions are separate helpers
(`crates/storage/src/execution_v2.rs:54-209`). Existing execution state has an
owner-generation field and single-owner index
(`crates/storage/schema/v2/workspace.sql:144-166`), but no combined approval,
tool, execution, nonce, and outbox transaction exists. Receipts and outbox are
durable aids, not effect ownership (`workspace.sql:284-300`).

**Normative request transaction.** `BEGIN IMMEDIATE` validates the exact
operation, human context, expiry, and pre-serialized request event, then inserts
pending approval plus its requested outbox row. The outbox row and approval row
commit together. A failed size, quota, foreign-key, or broker baseline check
commits nothing.

**Normative resume claim pseudocode.** All reads and writes below are one SQLite
transaction; external work starts only after commit:

```text
event = serialize_redacted_claim_event(ids, digest, generation)
if utf8_bytes(event).len() > 4096: return PayloadTooLarge, mutate nothing
BEGIN IMMEDIATE
row = SELECT approval + execution + concrete tool binding
      WHERE approval.id = wire_approval_id
if row absent: rollback, NotFound
if row.state == 4: rollback, return durable result or reconciliation status
if row.state != 1: rollback, AlreadyDecided/Expired/Cancelled
if now >= row.expires_at: CAS row 1 -> 3, rollback/commit expiry only, Expired
if any binding, principal, origin, policy, digest, or owner generation differs:
    rollback, StaleBinding
rerun trusted broker on reconstructed concrete operation
if broker != Allow or local-only origin fails:
    rollback, Denied, no effect
nonce = fresh UUID; reconciliation_key = fresh UUID
CAS approval 1 -> 4, set nonce, reconciliation_key, owner_generation
CAS execution queued -> running and tool planned -> dispatched
INSERT one bounded claim outbox event containing IDs, state, digest, nonce
COMMIT
dispatch concrete operation with nonce and scoped cancellation
```

The actual implementation must ensure `event` is serialized and byte-checked
before any state mutation, and repeat the check in the transactional writer.
Approval consumption, execution/tool claim, nonce/key persistence, and claim
outbox insertion are one commit. The effect is outside the transaction. If the
process crashes before commit, no dispatch is assumed. If it crashes after claim
commit and before a confirmed result, state is uncertain and must reconcile; it
must never auto-replay solely because approval state is 4.

**Persistence/lifetime/failure.** A duplicate resume returns the stored terminal
result or `Uncertain(reconciliation_key)` and cannot invoke the executor. A known
result transaction sets tool/execution result, receipt, and result outbox. An
unknown result sets tool/execution uncertain and retains nonce/key until explicit
reconciliation. Cancellation before claim is terminal with no effect; after the
claim follows uncertain-effect rules.

**Owner and dependencies.** Storage owner implements the transaction; server
coordinator owns owner generation and continuation; tools owner consumes nonce,
performs concrete brokered dispatch, and records result; independent verifier
tests crash injection. Depends on corrections 2-5, 8, and 9.

**Resource limits.** One active execution owner per session; pending approvals max
1024 per daemon and 16 per session; wait registrations max 1024; transaction
queries are indexed and bounded; no detached task; claim event max 4096 bytes.

**Future compiling RED assertions.**

- request commit creates exactly one pending row and one request outbox event;
- two concurrent decisions have one CAS winner and no loser effect;
- claim commit atomically changes approval/tool/execution and creates one nonce
  and one claim outbox event;
- injected failure before commit leaves no state change and no dispatch;
- duplicate resume after commit invokes zero additional effects;
- crash after claim before result yields uncertain/reconciliation, never blind
  replay;
- known result is durable and observable after restart.

### 7. Expired state 3 has bounded retention and recovery protection

**Current evidence.** `ApprovalsV2::expire_sweep` changes due pending rows to
state 3 using `expires_at_us <= now_us` and a 500-row clamp
(`crates/storage/src/approvals_v2.rs:130-145`). Retention deletes only states
1, 2, and 4 (`crates/storage/src/retention_v2.rs:84-105`) and backlog counts the
same states (`:129-147`). Thus expired state 3 is not retained or bounded by the
current retention path.

**Normative retention rule.** Expiry always sets state 3, `decision_kind=expire`,
and `resolved_at_us`. State 3 is retained as audit/recovery evidence for 30 days
after resolution, or longer while a linked execution, receipt, outbox event, or
reconciliation is unresolved. Deletion is eligible only when
`now >= retention_until_us`, no linked execution/tool/reconciliation is active,
and the row is not needed by the configured audit policy. State 3 is included in
the same bounded terminal sweep and backlog query as states 1, 2, and 4. State 4
with unresolved uncertainty is never deleted by ordinary approval retention.

Retention deletes approval resources through the existing cascade only after the
dependency check. It may delete at most 500 rows per call, ordered by retention
time and UUID/primary key. No sweep deletes recovery evidence early to satisfy a
quota.

**Persistence/lifetime/failure.** Expired pending approvals cannot be resurrected
or dispatched. A failed dependency query leaves the row. Missing retention data
is fail-closed and non-deletive. Restart recovery indexes pending, expired, and
uncertain rows with bounded queries; it never scans an unbounded table or starts
an orphan waiter. Existing restart policy remains: pending rows are reloaded and
expired, consumed/unknown rows are reconciled, and no blind replay occurs.

**Owner and dependencies.** Storage/retention owner implements state-3 sweep and
backlog; server recovery owner supplies owner registration; integration owner
checks restart behavior. Depends on correction 5 fields and correction 6
reconciliation state.

**Resource limits.** 30-day minimum retention; 500 deletion rows per call;
pending caps from correction 6; indexed queries only.

**Future compiling RED assertions.**

- exact-expiry sweep creates state 3 and `decision_kind=expire`;
- retention backlog includes state 3;
- state 3 younger than 30 days is not deleted;
- state 3 with active reconciliation is not deleted after 30 days;
- eligible state 3 deletion removes its resources and at most 500 rows;
- restart reloads pending/expired/uncertain rows with no automatic effect.

### 8. Resume reauthorizes the concrete file/tool operation

**Current evidence.** The live server initially authorizes generic
`OperationIntent::Tool` (`crates/server/src/lib.rs:1388-1437,1607-1645`).
`ToolExecutor::execute_authorized` only brokers `read`; every non-read name falls
through to unbrokered `execute` (`crates/tools/src/executor.rs:136-147`).
`run_if_allowed` correctly invokes no effect for deny/human gate
(`crates/security/src/tool_authorize.rs:188-195`), but the fallback bypasses it.

**Normative boundary.** Approval stores a concrete invocation and canonical
digest. Resume reconstructs a typed concrete operation, not generic
`OperationIntent::Tool`:

- file read/write/delete/mkdir: exact normalized path, action, offset/limit or
  bounded content metadata; call the corresponding brokered file operation;
- process/shell: exact executable, argv vector, and cwd as
  `OperationIntent::Process`; never concatenate a shell command string;
- SQL: exact statement/database as `OperationIntent::Sql` and rerun the SQL
  broker;
- `echo` or another no-side-effect tool: typed adapter with exact name/args and
  broker decision;
- MCP/extension tool: typed server/tool/resource capability with a registered
  concrete adapter; absent adapter or generic fallback is `UnsupportedOperation`.

Immediately before the effect, the coordinator recomputes canonical digest,
checks all immutable bindings, applies local-only and principal checks, calls the
trusted broker, and passes the concrete operation to a broker-enforced executor.
`execute_authorized` must reject unsupported non-read paths rather than calling
`execute`. Deny, human gate, stale digest, protected path, or unsupported
operation invokes no closure, process, file write, or provider call.

The existing protected-path baseline remains mandatory: wildcard permission does
not bypass `crates/security/src/platform_matrix.rs:215-249`. Approval evidence
cannot lift a mandatory protected-path deny.

**Persistence/lifetime/failure.** The concrete invocation is immutable after
request. Argument/path drift is stale binding and cannot retarget or silently
burn an approval. Tool result, cancellation, and uncertainty use correction 6
states and reconciliation key. Raw arguments, protected paths containing
secrets, environment values, and tokens are excluded from logs/outbox.

**Owner and dependencies.** Security owner defines concrete broker intents;
tools owner removes the unbrokered fallback and implements typed adapters;
server/integration owner wires resume to that path. Depends on corrections 2-4,
6, and the existing protected-path policy.

**Resource limits.** Existing read output max 64 KiB
(`crates/tools/src/file_ops.rs:13,229-249`); arguments, paths, argv, and SQL use
their existing bounded fields; process lifetime uses scoped cancellation and
timeouts; no unbounded output or inherited environment.

**Future compiling RED assertions.**

- changed file path, action, offset, or limit fails digest/broker checks and
  leaves the file unchanged;
- non-read tool cannot reach generic `execute` without a broker decision;
- denied write/delete/process asserts no file/process/provider side effect;
- protected path remains denied under wildcard permissions and under approval;
- shell argv is passed as structured arguments, never a shell-concatenated string;
- unsupported generic tool returns typed failure with no dispatch.

### 9. Pending cap and exact 4096-byte outbox admission

**Current evidence.** Remote approvals define `MAX_PENDING_APPROVALS = 1024`, but
`register` checks `g.requests.len()` including decided records
(`crates/server/src/remote_approvals.rs:18-21,198-215`). Existing SQL enforces
`event_outbox.payload_json <= 4096` bytes
(`crates/storage/schema/v2/workspace.sql:293-300`). `writer_v2` has a matching
private 4096-byte check (`crates/storage/src/writer_v2.rs:13-17,355-382`), while
generic storage exposes a 64 KiB constant/check (`crates/storage/src/lib.rs:54,671`)
and therefore cannot define the approval contract alone.

**Normative quota rule.** Count only active durable pending rows (`state=0`), not
resolved/expired rows retained for audit. Enforce max 1024 per daemon/workspace
and max 16 per session, with one active approval per tool call/execution. Before
admitting a request, bounded expiry work may mark due rows state 3; if active
count remains at cap, return typed `ApprovalCapacity` with no row, event, or
effect. Retained terminal rows do not consume pending capacity.

**Normative payload rule.** The approval coordinator owns
`MAX_OUTBOX_PAYLOAD_BYTES = 4096`. Serialize JSON deterministically, measure
UTF-8 byte length, permit exactly 4096, and reject 4097 or more before any state
mutation. Never truncate, retry with a lossy representation, or rely only on a
later SQL constraint. Every request, decision, claim, result, and reconciliation
event preflights this rule and the transactional writer repeats it. Payloads
contain bounded IDs, state, digest/version, and redacted summaries only; raw
arguments, paths, credentials, and environment values never enter outbox JSON.

**Persistence/lifetime/failure.** Oversize event or quota failure rolls back the
whole request/decision/claim transaction. A retained resolved row is not counted
as pending but remains subject to correction 7. Queue publication is durable
through `event_outbox`; transient event bus notifications are only wake-up hints.

**Owner and dependencies.** Storage/writer owner exposes the strict 4096-byte
validator; server/coordinator owner applies it before each transaction; retention
owner bounds terminal rows; remote owner changes cap accounting to pending-only.
Depends on corrections 5-7 and atomic claim semantics in correction 6.

**Resource limits.** 1024 active pending per daemon/workspace, 16 per session,
one per tool call, 1024 owned waits, 500-row sweeps, 4096 UTF-8 bytes per outbox
payload. Exact byte count, not character count, is authoritative.

**Future compiling RED assertions.**

- 1024 active pending requests succeed and request 1025 fails typed with no row;
- resolved and retained rows do not consume the active pending quota;
- session request 17 fails typed with no side effect;
- payload lengths 4095, 4096, and 4097 bytes yield accept, accept, reject;
- rejected 4097-byte request/decision/claim leaves all state and outbox counts
  unchanged;
- event body inspection proves no raw path, args, token, or environment value.

## Cross-correction ownership DAG

```text
auth/principal proof (server/auth)
        +-> immutable bindings and UUID API (storage/schema/contracts)
        +-> canonical digest v1 (security -> tools/storage)
        +-> baseline broker + local-only gate (security)
        +-> request transaction + bounded outbox (storage/coordinator)
        +-> authenticated decision route (server)
        +-> atomic claim transaction (storage/coordinator)
        +-> concrete brokered dispatch (security/tools)
        +-> result/uncertain reconciliation (tools/storage/server)
        +-> indexed restart and state-3 retention (storage/server)
        +-> independent RED author and verifier
```

No leaf owner may edit shared schema, route registry, central runtime, frozen
tests, or accepted policy without an integration proposal. The parent APP-012
remains open until the real installed journey is wired and independently verified.

## Independent verifier checklist

1. Confirm candidate revision and this correction artifact are present; confirm
   no source/schema/test/route/runtime file changed.
2. Confirm frozen APP-012 read-test SHA-256 remains
   `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.
3. Confirm expiry tests use `now < expires_at` for every grant, remote, resume,
   decision, and sweep path, including exact equality.
4. Confirm daemon bearer is transport-only and every human decision/resume has a
   trusted principal, stable client/device ID, valid proof, and origin.
5. Confirm local-only is enforced in the canonical coordinator immediately before
   decision/claim, not through a caller convention or request flag.
6. Confirm security/tools/storage use one BLAKE3-256 v1 length-prefixed domain
   encoding, raw 32-byte storage, and UUID `ApprovalId`, with no FNV fallback.
7. Confirm schema/API round-trips workspace, requester, principal, client,
   decision/cancellation, generation, nonce, reconciliation key, and digest
   version without overloading unrelated fields.
8. Confirm request and claim transactions include their outbox event; resume
   atomically binds approval consumption, execution/tool ownership, nonce, and
   generation before external dispatch.
9. Confirm state 3 is expired at equality, retained for 30 days or until recovery
   dependencies settle, included in bounded backlog/sweep, and never blindly
   replayed.
10. Confirm resume reconstructs concrete file/process/SQL/tool operations and
    reruns the broker; no non-read fallback reaches unbrokered `execute`.
11. Confirm active pending-only caps of 1024 daemon/workspace and 16 session,
    bounded waits, exact 4096-byte outbox admission, and reject-not-truncate
    behavior before mutation.
12. Run the future compiling RED on a disposable fixture only, freeze its hash,
    then run an independent implementation/verifier lane. Do not turn this
    research completion into RED authorization or APP-012 acceptance.

## Required future RED matrix

The independent RED author must compile and capture failures for: approve,
reject, exact expiry, cancellation, duplicate decision, concurrent decision,
stale path/tool/args/scope/requester/workspace binding, policy generation bump,
principal/client mismatch, local-only remote attempt, restart pending,
restart consumed, crash before claim commit, crash after claim before result,
ambiguous effect, replay, protected path, concrete non-read broker enforcement,
secret redaction, pending/session quota overflow, and outbox lengths 4095/4096/4097.
Every denied/rejected/expired/stale/capacity case asserts absence of file,
process, provider, approval-consumption, and outbox side effects as applicable.
All tests use disposable DB/workspace fixtures, bounded channels, Rust-local
deadlines, no live credentials, no inherited secrets, and no live user database.

## Read-only validation record

Commands required for this research lane:

```sh
rtk git grep -n 'RequireHuman\|Grant\|approval\|pending\|resume' -- crates
rtk shasum -a 256 crates/server/tests/app012_tool_journey_red.rs
rtk git diff --check
rtk python3 tools/validate_repository.py
rtk python3 tools/convergence_gate.py
```

Expected and recorded boundaries:

- grep is read-only and provides source inventory; no Cargo, SQLite, daemon,
  browser, provider, or live-auth process is run.
- frozen APP-012 hash remains the value recorded above.
- diff check must pass.
- repository/convergence guards may report pre-existing backlog or ownership
  findings; such findings do not authorize source edits and are reported exactly.
- resource observation: documentation/grep/hash/validation only; no resource-
  heavy process started.

## Handoff status

All nine verifier corrections are explicit: inclusive expiry, authenticated
principal versus bearer, mandatory local-only gate, one digest/domain and UUID
mapping, schema/API ownership and missing fields, atomic claim transaction,
state-3 retention, concrete broker revalidation, and pending/4096-byte bounds.
This artifact is a research correction candidate only. Independent RED author,
implementer, integrator, and verifier must establish compiling RED, GREEN, wiring,
and acceptance separately.
