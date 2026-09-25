# APP012 approval and resume prewire correction

## Claim and boundary

- Task: `APP012-APPROVAL-RESUME-PREWIRE-CORRECTION`
- Session: `ses_f29539c31ffeQiDGOmqt5SKUiE`, continued after lawful reclaim by
  orchestrator session `ses_f3c4de578ffelQv59xDXmOs03B` when the original
  worker exhausted its step budget after writing into the wrong worktree.
- Branch: `plan/APP012-APPROVAL-RESUME-PREWIRE-CORRECTION`
- Candidate source revision: `d8599168b05fdf40a3841b398d9256afaf5b090a`
- Owned artifact: this file plus this task's ledger row.
- Scope: contract correction and RED classification only.
- Forbidden in this lane: Cargo, product, schema, dependency, route, runtime,
  test, frozen-file, database, credential, keyring, browser, and provider edits.
- Parent status: APP012 remains open. This artifact grants no implementation,
  integration, or acceptance authorization.
- Frozen APP012 read test SHA-256:
  `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.

## Verdict

**CORRECTED CONTRACT. RED authorization remains conditional.**

The prior correction correctly described the desired state, but its RED matrix
mixed existing public seams with APIs that do not exist. This artifact publishes
the exact proposed prewire seam, records which tests can exercise current code,
and marks every API-dependent RED blocked until the prewire lands. No compile
failure is called a behavioral RED. No keyring, device signature, or bearer
credential is treated as human authority.

## Source ledger

| Fact | Evidence | Classification |
|---|---|---|
| `OperationDigest` is a transparent `String`; `of` emits FNV-1a-64 hex | `crates/security/src/app_policy.rs:35-60` | Existing legacy compatibility seam |
| Grant expiry currently accepts equality | `crates/security/src/app_policy.rs:152-181`, especially `:160-162` | Existing bug candidate |
| `enforce_local_only` is a separate caller obligation | `crates/security/src/app_policy.rs:341-356` | Existing helper, not canonical enforcement |
| `decide` does not receive transport origin | `crates/security/src/app_policy.rs:288-338` | Existing incomplete seam |
| `ToolAuthorizer::authorize_with_grant` calls `decide` without origin | `crates/security/src/tool_authorize.rs:138-161` | Existing incomplete seam |
| Non-read `execute_authorized` falls through to `execute` | `crates/tools/src/executor.rs:136-147` | Existing behavioral RED seam |
| Tool digest is a separate four-lane FNV `par005-v1` encoding | `crates/tools/src/app_services.rs:310-355` | Existing conflicting compatibility seam |
| Storage requires 32-byte `intent_hash` but request returns SQLite row ID | `crates/storage/schema/v2/workspace.sql:223-240`; `crates/storage/src/approvals_v2.rs:26-80` | Existing partial v2 seam |
| Expiry CAS sets only state 3 | `crates/storage/src/approvals_v2.rs:130-145` | Existing partial v2 seam |
| Retention sweeps states 1, 2, 4, not state 3 | `crates/storage/src/retention_v2.rs:84-106`; backlog `:140-147` | Existing behavioral RED seam |
| v2 outbox writer and SQL both enforce 4096 bytes | `crates/storage/src/writer_v2.rs:13-17,355-383`; `crates/storage/schema/v2/workspace.sql:293-300` | Existing enforcement, regression only |
| Public `MAX_EVENT_PAYLOAD_BYTES` is 64 KiB for the older path | `crates/storage/src/lib.rs:53-79,671` | Existing separate v1 bound |
| Bearer middleware authenticates the daemon request only | `crates/server/src/daemon_auth.rs:150-175` | Existing transport auth |
| No approval route or coordinator is registered | `crates/server/src/lib.rs:1-76,271-335` | Missing new integration seam |
| Human gate is currently terminal tool output | `crates/server/src/lib.rs:1429-1437,1639-1645` | Existing incomplete caller |
| Remote approvals are process-local, bind opaque fields, and cap retained requests | `crates/server/src/remote_approvals.rs:18-21,118-170,198-280` | Reference semantics only |
| Pairing uses opaque challenge and epoch/revocation checks, not signature verification | `crates/server/src/remote_revocation.rs:1-8,97-110,141-193,274-317` | Existing remote lifecycle, not human proof |
| Credential storage is planning-only | `crates/providers/src/auth_store.rs:1-7,62-90,328-382` | Existing planned seam |
| Keyring persistence RED is blocked by absent production seam | commit `4814357`, `worklog/APP012-KEYRING-BLACKBOX-RED-SEAM.md` | Explicit blocked research decision |

The remote module's `credential: &str` enrollment argument is not a verified
device signature. It is opaque credential state in an in-memory registry. The
daemon bearer likewise proves transport access, not human identity.

## R1. Exact prewire seam

The following is the minimum additive seam that must be published by the
serialized integration lane before API-dependent RED authoring. These are
proposed public paths and signatures, not current symbols. The prewire must
implement real validation and typed failures, not empty bodies or compile-only
types.

### Authentication and origin

File: `crates/server/src/approval_auth.rs`, registered by `crates/server/src/lib.rs`.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApprovalAuthError {
    Missing,
    Invalid,
    Unauthenticated,
    OriginUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HumanPrincipal {
    pub stable_id: [u8; 16],
    pub client_id: [u8; 16],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportOrigin {
    Local,
    Remote { device_id: String },
}

pub trait ApprovalAuthenticator: Send + Sync {
    fn authenticate(
        &self,
        request: &axum::http::Request<axum::body::Body>,
    ) -> Result<(HumanPrincipal, TransportOrigin), ApprovalAuthError>;
}
```

`HumanPrincipal` must be derived from an accepted authenticated mechanism. It
must never be read from JSON, model output, project configuration, or the
daemon bearer alone. `TransportOrigin::Local` must be derived from the actual
transport and cannot be selected by the caller. Stable IDs are bounded opaque
identifiers. The authenticator owns no secret beyond the request lifetime.

### Digest seam

File: `crates/security/src/approval_digest.rs`, registered by
`crates/security/src/lib.rs`.

```rust
pub const APPROVAL_DIGEST_ALGORITHM: &str = "blake3-256-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DigestAlgorithmVersion {
    Blake3_256V1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalDigest(pub [u8; 32]);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DigestError {
    UnsupportedDigestVersion,
    NonCanonicalOperation,
    InputTooLarge,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConcreteOperation {
    Tool {
        name: String,
        arguments_json: String,
        scope: String,
    },
}

pub fn digest_operation(
    operation: &ConcreteOperation,
) -> Result<CanonicalDigest, DigestError>;
```

`ConcreteOperation` must carry the exact tool name, canonical JSON arguments,
scope, and all effect-bearing fields. A generic `OperationIntent::Tool` with
only `name` and `description` is not enough for resume revalidation.

### Coordinator seam

File: `crates/server/src/approval_coordinator.rs`, registered by
`crates/server/src/lib.rs`.

```rust
 pub struct ApprovalCoordinator;

pub struct PresentedBinding {
    pub operation: ConcreteOperation,
    pub digest: CanonicalDigest,
    pub policy_generation: u64,
    pub workspace: std::path::PathBuf,
    pub requester: String,
    pub expires_at_us: i64,
}

pub struct DecisionRequest {
    pub approval_id: ApprovalId,
    pub decision: DecisionKind,
    pub principal: HumanPrincipal,
    pub origin: TransportOrigin,
    pub presented: PresentedBinding,
    pub now_us: i64,
}

pub struct ResumeRequest {
    pub approval_id: ApprovalId,
    pub principal: HumanPrincipal,
    pub origin: TransportOrigin,
    pub now_us: i64,
}

pub enum DecisionResult {
    Pending,
    Approved,
    Denied,
    Cancelled,
    Expired,
    Conflict,
}

pub enum ResumeResult {
    Claimed { dispatch_nonce: [u8; 32] },
    AlreadyClaimed { reconciliation_key: [u8; 32] },
}

impl ApprovalCoordinator {
    pub async fn decide(
        &self,
        request: DecisionRequest,
    ) -> Result<DecisionResult, ApprovalCoordinatorError>;

    pub async fn resume(
        &self,
        request: ResumeRequest,
    ) -> Result<ResumeResult, ApprovalCoordinatorError>;
}
```

The exact public error type is:

```rust
pub enum ApprovalCoordinatorError {
    Expired,
    Unauthenticated,
    PrincipalMismatch,
    Revoked,
    RemoteOrigin,
    DigestMismatch,
    UnsupportedDigestVersion,
    StaleBinding,
    UnsupportedOperation,
    ApprovalCapacity,
    Conflict,
    NotPending,
    NotApproved,
    Storage(StorageError),
}
```

The implementation must not expose raw arguments, paths, credentials, or
provider responses through these errors.

### Storage seam

The serialized storage owner must extend
`crates/storage/src/approvals_v2.rs` with typed methods, or publish an additive
`crates/storage/src/approval_store_v2.rs` and re-export it from
`crates/storage/src/lib.rs`:

```rust
pub struct StoredDecisionRequest {
    pub approval_id: ApprovalId,
    pub decision: DecisionKind,
    pub principal_id: [u8; 16],
    pub client_id: [u8; 16],
    pub origin_is_local: bool,
    pub digest_algorithm_version: u8,
    pub digest: [u8; 32],
    pub policy_generation: i64,
    pub now_us: i64,
}

pub struct StoredResumeRequest {
    pub approval_id: ApprovalId,
    pub principal_id: [u8; 16],
    pub client_id: [u8; 16],
    pub origin_is_local: bool,
    pub broker_generation: u64,
    pub digest_algorithm_version: u8,
    pub digest: [u8; 32],
    pub now_us: i64,
}

pub fn decide(
    connection: &mut rusqlite::Connection,
    request: &StoredDecisionRequest,
) -> Result<DecisionRecord, ApprovalStorageError>;

pub fn claim_resume(
    connection: &mut rusqlite::Connection,
    request: &StoredResumeRequest,
) -> Result<DispatchClaim, ApprovalStorageError>;

pub fn expire_due(
    connection: &mut rusqlite::Connection,
    now_us: i64,
    limit: usize,
) -> Result<usize, ApprovalStorageError>;

pub enum ApprovalStorageError {
    Expired,
    PrincipalMismatch,
    Revoked,
    RemoteOrigin,
    DigestMismatch,
    UnsupportedDigestVersion,
    StaleBinding,
    Conflict,
    NotPending,
    NotApproved,
    Capacity,
    Sqlite(rusqlite::Error),
}
```

`DecisionRequest` and `ResumeRequest` are server-layer inputs. The storage
layer receives the dependency-safe `StoredDecisionRequest` and
`StoredResumeRequest` above; it must not depend on the server crate. The
coordinator maps authenticated server inputs to these storage values only
after validating the concrete operation and canonical digest.

The storage methods must accept the opaque `ApprovalId`, not expose the SQLite
`pk` as a wire ID. They must return typed conflict, expiry, stale-binding,
unsupported-version, and capacity failures. `StorageError::Sqlite` alone is
not an adequate approval API.

### Route seam and caller

The integration owner must register these routes in
`crates/server/src/lib.rs::router_with_auth` after publishing the coordinator:

```text
POST /api/approvals/{id}/decision
POST /api/approvals/{id}/resume
```

The handlers derive `(HumanPrincipal, TransportOrigin)` through
`ApprovalAuthenticator`, parse only bounded request fields, call the same
daemon-owned `ApprovalCoordinator`, and serialize typed status. The request
body cannot supply authority. `RequireHuman` in the live turn caller must
create a durable pending approval and suspend the owned continuation instead
of returning the current terminal string.

## R2. Authority correction: no keyring or device-signature assumption

The phrase "existing keyring/client credential path" is removed. Current
`auth_store` is a planning boundary: it opens no backend, performs no I/O, and
retains no credential bytes (`crates/providers/src/auth_store.rs:1-7`). Its
`StoreBackend::resolve` accepts a caller boolean (`:62-90`), which is not proof
of a keyring. The blackbox audit at commit `4814357` explicitly concludes that
no admitted production persistence seam can host a compiling credential
durability RED.

The phrase "device signing key" is also removed. `remote_revocation` issues
opaque `pair-{seq}` challenges and checks epochs/revocation, but has no
signature algorithm or verification API (`crates/server/src/remote_revocation.rs:141-193,216-274`).
`remote_approvals` binds an opaque `device_id` and digest under a mutex; it does
not authenticate a human principal (`crates/server/src/remote_approvals.rs:218-280`).

Therefore:

1. Bearer authentication remains daemon transport authentication only.
2. Human principal authentication is a **new auth integration prewire**, owned
   by the server auth owner, with an independent security review before RED.
3. Device signature verification is a separate **blocked new requirement**. It
   cannot be represented as a passing RED using the opaque challenge seam.
4. Keyring persistence remains the separate blocked APP012 research lane. No
   approval/resume test may import `keyring`, access the native keychain, or
   treat a caller-supplied credential string as authority.

Acceptance gate for this item: an approved authenticator publishes a real
principal/origin proof and bounded lifetime. Until then, principal, remote
approval authority, and device-signature REDs are Manifest B, not RED.

## R3. One digest representation and legacy migration

### Canonical representation

Version 1 has exactly this mapping:

| Identifier | Representation |
|---|---|
| Algorithm string | `blake3-256-v1` |
| Storage version | integer `1` |
| Digest bytes | raw BLAKE3 output, exactly 32 bytes |
| Wire ID | `ApprovalId` UUID bytes, independent of digest |
| Text display | lowercase hex only for redacted display, never a second storage format |

The canonical encoder is domain-separated and length-prefixed. It hashes the
algorithm version, operation variant, and every effect-bearing field with a
fixed unsigned big-endian byte length before each byte field. JSON tool
arguments use one canonical serializer and reject duplicate keys, non-finite
numbers, and over-budget input. No delimiter join, map iteration order, or
description-only generic tool intent is accepted.

The storage migration adds `digest_algorithm_version INTEGER NOT NULL` with
version `1` meaning exactly `blake3-256-v1`; `intent_hash BLOB` remains exactly
32 raw bytes. The version and bytes are validated together before decision or
resume.

### Legacy behavior and fail-closed migration

The current security value is FNV-1a-64 hex in `OperationDigest(String)`
(`crates/security/src/app_policy.rs:35-49`). The tools value is a different
32-byte four-lane FNV encoding tagged `par005-v1`
(`crates/tools/src/app_services.rs:310-337`). They are not aliases and cannot
be silently converted.

The prewire must supersede `OperationDigest` for durable approval/resume. The
existing type may remain as a legacy compatibility API for its existing unit
tests, but it cannot construct a new durable approval or satisfy
`claim_resume`. Existing grants and rows are handled as follows:

- Rows without version `1`, rows with old 16-character FNV text, and rows whose
  bytes do not have the exact 32-byte v1 length are `UnsupportedDigestVersion`.
- No automatic hash reinterpretation, FNV-to-BLAKE3 conversion, or fallback
  comparison is allowed.
- Existing pending legacy rows remain retained for audit and are made
  non-dispatchable. They may be explicitly expired by the bounded expiry job;
  they are never silently approved or reused.
- Existing consumed/allowed legacy rows do not trigger replay or automatic
  resume. They return a durable unsupported/reconciliation status.
- A migration may convert a row only when it can reconstruct the exact
  concrete operation and verify the old binding under an independently
  approved migration rule. Otherwise it marks the row legacy-invalid and
  fails closed. No silent grant reuse is permitted.

This is a schema and security prewire requirement, so digest REDs are blocked
until `CanonicalDigest`, `DigestError`, the version column, and the concrete
operation constructor are public.

## R4. Expiry commit and rollback semantics

All approval eligibility checks use one boundary: **usable iff
`now_us < expires_at_us`**. Equality is expired. This applies to grant checks,
remote checks, decision checks, resume checks, and expiry selection. The
current mismatches are visible at `app_policy.rs:160-162`,
`remote_approvals.rs:246-247,324-329`, and
`approvals_v2.rs:136-143`.

The expiry transaction has one permitted durable effect:

```sql
BEGIN IMMEDIATE;
UPDATE approvals
SET state = 3,
    decision_kind = 5,
    resolved_at_us = :now_us
WHERE state = 0
  AND expires_at_us <= :now_us
  AND pk IN (bounded due selection);
COMMIT;
```

This is **commit expiry only**. It commits the `0 -> 3` expiry CAS and its
decision metadata. It does not claim an execution, consume an approval, insert
a dispatch claim, invoke a broker, invoke a tool, or enqueue an external
effect. If any SQL or validation step fails before commit, the transaction
rolls back and the approval remains pending. If the CAS loses a race, the
caller returns a typed conflict or already-resolved status and performs no
effect. A decision at equality cannot approve; either the expiry CAS wins or
the caller observes an expired/conflict result.

Resume has the separate `1 -> 4` claim transaction. Its approval, execution,
tool claim, dispatch nonce, and claim outbox event commit before the external
effect. A crash after commit and before the effect is a durable claim with no
automatic replay. An ambiguous effect is marked uncertain with a reconciliation
key; exactly-once completion of an unobservable external effect is not claimed.

## R5. Decision kind and state mapping

The prewire publishes one versioned enum. Values are explicit and stable:

```rust
#[repr(u8)]
pub enum DecisionKind {
    None = 0,
    Pending = 1,
    Approve = 2,
    Deny = 3,
    Cancel = 4,
    Expire = 5,
}
```

`None` means the caller supplied no decision. It is accepted only as request
normalization and legacy read compatibility; a new durable request is stored as
`Pending`, never as an unclassified approval. `Pending` is therefore an
explicit decision/status kind, not an omitted enum arm.

The migrated durable mapping is:

| `ApprovalState` storage | `DecisionKind` storage | Meaning |
|---:|---:|---|
| `0 pending` | `1 pending` | request exists, no human decision |
| `1 allowed_once` | `2 approve` | approved, not yet claimed for resume |
| `2 denied` | `3 deny` | explicit denial |
| `3 expired` | `5 expire` | expiry CAS committed |
| `4 consumed` | `2 approve` | approved claim consumed; result tracked by execution |
| `5 cancelled` | `4 cancel` | explicit cancellation, never dispatchable |

The migration must extend the current `state BETWEEN 0 AND 4` contract
(`workspace.sql:231`) to admit state 5, or publish an equally explicit
terminal cancellation mapping in an approved schema decision. It must not
encode cancel as consumed. Retention must include state 5 after its policy
window. A decision request carrying `none` or `pending` cannot approve, deny,
cancel, or consume an existing row.

## R6. Missing route/coordinator and RED owner

There is currently no `approval_coordinator` module in `crates/server/src`, no
approval decision route, and no resume route. The route table in
`crates/server/src/lib.rs:271-335` ends with turn routes. The live turn path
creates a fresh broker (`:1253-1255`) and converts `RequireHuman` to terminal
text (`:1429-1437,1639-1645`). `remote_approvals` is not a durable server
coordinator.

The serialized integration owner must first publish:

1. `crates/server/src/approval_auth.rs` and its registration;
2. `crates/security/src/approval_digest.rs` and its registration;
3. typed storage APIs and migration;
4. `crates/server/src/approval_coordinator.rs` and its caller;
5. the two route registrations and continuation ownership in
   `crates/server/src/lib.rs`.

Only after those seams compile may the independent RED owner write exactly one
server test file:

```text
crates/server/tests/app012_approval_resume_red.rs
```

Its imports must use the published paths:

```text
opencode_rk_server::approval_auth::{HumanPrincipal, TransportOrigin}
opencode_rk_server::approval_coordinator::{
    ApprovalCoordinator, ApprovalCoordinatorError, DecisionRequest, ResumeRequest,
}
opencode_rk_security::approval_digest::{CanonicalDigest, DigestAlgorithmVersion}
opencode_rk_storage::approvals_v2::{DecisionKind, ApprovalState}
```

Its caller must exercise `router_with_auth` or the real daemon-owned
coordinator, not a test-local fake, `#[path]` shim, or direct mocked success.
The current absence of this target is not a RED result. It is blocked until the
prewire publishes the symbols above.

## Manifest A: compiling-now existing-seam checks

Manifest A contains only behavior that can be expressed against current public
APIs. The test files are not authored in this lane. Each target has one owner;
shared `lib.rs` and schema registration remain integration-owned. Commands are
the eventual bounded commands after the independent RED author creates the
target. A missing target today is not evidence of RED.

| ID | Single-file RED owner | Existing imports/seam | Intended failure |
|---|---|---|---|
| A1 expiry | `crates/security/tests/app012_approval_expiry_red.rs` | `app_policy::{decide, Grant, Scope, ExpectedScope, PolicyDeny}`, `OperationIntent`, `PermissionBroker`, `SecurityPolicy`, `PermissionSet` | At `now == expires_at`, `Grant::covers` currently allows equality because it checks only `now > expiry`; expected typed `Expired` denial fails. |
| A2 state-3 retention | `crates/storage/tests/app012_approval_retention_red.rs` | `ApprovalsV2`, `RetentionV2`, `SchemaV2` | A resolved state-3 row is not swept and is absent from `retention_backlog`; expected bounded retention fails. Pending state 0 remains untouched. |
| A3 non-read broker gate | `crates/tools/tests/app012_concrete_dispatch_red.rs` | `ToolExecutor`, `ToolCall`, `PermissionBroker`, `PermissionSet`, `PermissionRule`, `RuleEffect` | A denied non-read `echo` or shell call currently reaches unbrokered `execute`; expected no process/output side effect fails. |
| A4 outbox bound/error | `crates/storage/tests/app012_outbox_bound_red.rs` | `V2Writer::append_outbox_event`, `SchemaV2`, `StorageError` | The v2 writer rejects 4097 bytes today, so that admission assertion is a GREEN regression, not a RED. A current behavioral RED is the typed error display: `StorageError::EventPayloadTooLarge` formats the public 64 KiB constant while the v2 outbox bound is 4096. The test asserts the v2 rejection and exact error-bound distinction separately. |

The intended focused command shape is:

```text
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 180 cargo test -p <crate> --test <target> -- --test-threads=1
```

The command is not run here because these test targets are not authored and the
lane forbids test changes and Cargo resource use. A valid RED receipt requires
the independent test author to add one target, run it against the real seam,
capture a behavioral failure, freeze its hash, and hand it to a separate
implementer. Compile failures or an already-green outbox assertion do not count.

## Manifest B: blocked API-dependent slices

These slices must not be represented by compile-fail RED. Each is blocked by a
specific missing authority or prewire seam.

| ID | One-file owner after prewire | Blocker and unblock decision |
|---|---|---|
| B1 local-only origin | `crates/server/tests/app012_approval_auth_red.rs` | No `HumanPrincipal`, trusted origin extractor, or accepted human auth mechanism. Unblock only after auth owner publishes `approval_auth` and security review accepts its proof. The existing `enforce_local_only` helper is not a canonical route. |
| B2 decision/resume coordinator | `crates/server/tests/app012_approval_resume_red.rs` | No coordinator, typed request/result/error API, durable continuation, or route. Unblock after R1 seam and real caller are wired. |
| B3 canonical digest | `crates/security/tests/app012_approval_digest_red.rs` | No versioned encoder, `CanonicalDigest`, concrete operation, or security/tools dependency seam. Unblock after exact v1 mapping and migration decision are accepted. |
| B4 schema and opaque ID | `crates/storage/tests/app012_approval_schema_red.rs` | Current schema lacks bindings, decision kind, owner generation, dispatch nonce, reconciliation key, version, and cancellation state; request returns SQLite `pk`. Unblock after migration and typed API are published. |
| B5 atomic claim | `crates/storage/tests/app012_approval_claim_red.rs` | No combined approval/execution/tool/nonce/outbox transaction. Unblock after storage owner publishes `claim_resume` with `BEGIN IMMEDIATE` semantics. |
| B6 durable cap and restart | `crates/server/tests/app012_approval_capacity_red.rs` | Remote `MAX_PENDING_APPROVALS` caps all retained requests in process-local state, not durable pending rows. Unblock after durable coordinator publishes workspace/session caps, recovery, and owner lifetime. |
| B7 outbox integration | `crates/server/tests/app012_approval_resume_red.rs` | Coordinator cannot currently insert a claim event. Unblock after it depends on the v2 writer bound and validates 4096 bytes before the claim transaction commits. |
| B8 keyring durability | separate `crates/providers/tests/keyring_persistence_red.rs` lane | `4814357` records no admitted production persistence seam. Unblock requires controller-approved backend and caller-owned secret lifetime; never infer it from approval auth. |
| B9 device signature | separate auth/security lane | No signature verifier or signing-key proof exists. Opaque challenge and epoch checks are insufficient. Unblock requires a separately accepted cryptographic transport contract. |

No B target may import an invented module, test a fake coordinator against
itself, use `#[ignore]`, or claim a compile failure as RED.

## Observable contract and state transitions

### Request

The coordinator authenticates the caller, constructs the exact concrete
operation, computes version-1 digest, validates future bounded expiry, and
performs one SQLite immediate transaction. It inserts state `pending`, kind
`pending`, opaque UUID `ApprovalId`, all binding fields, and a bounded request
outbox event. It does not execute an effect. Capacity failure is typed and
side-effect-free.

### Decision

Only an authenticated principal matching the stored principal and binding may
win the pending CAS. `approve` becomes `allowed_once`; `deny` becomes `denied`;
`cancel` becomes `cancelled`; due expiry becomes `expired`. One winner is
durable. A loser receives conflict/status and cannot emit a second decision or
effect. `remote_only` or local-only mismatch is denied before mutation.

### Resume

Resume accepts only `allowed_once`, matching principal/origin, live policy
generation, current broker generation, exact concrete digest, and
`now < expires_at`. The claim transaction atomically consumes the approval,
claims execution/tool rows, stores a 32-byte dispatch nonce and reconciliation
key, and appends a bounded outbox claim event. The broker authorizes the
concrete operation immediately before the external effect. Non-read tools have
no unbrokered fallback.

### Restart and ambiguity

Pending rows reload and expire within bounded sweeps. Consumed rows never cause
automatic replay. A pre-effect crash after claim returns a durable claim or
reconciliation status. An effect whose outcome is unknown becomes uncertain;
manual or idempotent reconciliation is required. No exactly-once claim is made
for an unobservable non-idempotent effect.

## Ownership, lifetime, and resource bounds

| Resource | Owner/lifetime | Bound or failure rule |
|---|---|---|
| Principal and origin | Authenticated request; copied as bounded identifiers into durable binding | Never body/model supplied; reject unauthenticated or mismatched identity |
| Coordinator | One daemon-owned service; joined on shutdown | No detached approval waiter; cancellation drops owned continuation |
| Pending approvals | Durable workspace coordinator | Proposed cap 1024 workspace, 16 session; typed `ApprovalCapacity`, no eviction |
| Remote reference | `RemoteApprovalStore` process-local only | Existing 1024 cap covers all retained requests, not a durable pending guarantee |
| Digest | 32 raw bytes plus version 1 | Reject every unsupported or malformed legacy value; no fallback |
| Dispatch claim | SQLite transaction owner | 32-byte nonce; one claim CAS; duplicate resume returns status |
| Outbox payload | v2 writer/schema | 4096 UTF-8 bytes, valid JSON; reject before commit |
| Approval sweep | Storage owner | Maximum 500 rows per call, existing `MAX_SWEEP_ROWS`; include state 3 and new state 5 after retention |
| Tool output | Existing executor | Existing file read cap 64 KiB at `crates/tools/src/file_ops.rs:13,229-249`; no unbounded retained result |
| Audit/error text | Security/tool authorizer | Existing 1024 broker entries and 256 tool entries; raw secrets and args excluded |

The 1024 workspace and 16 session approval caps are proposed normative bounds,
not current constants. They remain blocked until the durable coordinator owner
publishes and independently tests them.

## Frozen and validation record

Read-only evidence for this correction:

```text
git rev-parse HEAD
# d8599168b05fdf40a3841b398d9256afaf5b090a

shasum -a 256 crates/server/tests/app012_tool_journey_red.rs
# 945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50

git diff --check
# PASS

python3 tools/validate_repository.py
# FAIL: inherited 51-error backlog-exhaustion reconciliation gate

python3 tools/convergence_gate.py
# BLOCKED: total=94 inherited/off-plan ledger findings on this later branch
```

No Cargo, SQLite, daemon, browser, provider, keyring, credential, or live
authentication process is authorized or run in this research lane. The frozen
APP012 test remains byte-identical. The repository validator and convergence
gate failures are inherited authority blockers, not acceptance evidence and not
permission to edit their controller-owned inputs.

## Remaining blockers

1. Auth owner must publish and verify trusted principal/origin proof.
2. Integration owner must publish the coordinator, storage transaction, route,
   digest, migration, and continuation seams above.
3. Independent RED author must write only compiling behavioral targets after
   prewire, freeze each hash, and record the first failing assertion.
4. Storage owner must decide the cancellation state migration and retention
   treatment for state 5.
5. Parent APP012 remains open until the real installed journey covers approval,
   resume, restart, denial, and independent verification on one integrated
   revision.
