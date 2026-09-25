# APP012-APPROVAL-PREWIRE-VERIFY

## Claim

- Task: `APP012-APPROVAL-PREWIRE-VERIFY`
- Type: independent contract verification
- Session: `ses_f28c48567ffeZOwdbuoDf8CamV`
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/verify-app012-approval-prewire`
- Branch: `verify/APP012-APPROVAL-PREWIRE`
- Candidate revision: `61b095902849f2e08ab2093c27c1d3e96003b93d`
- Contract under review: `worklog/APP012-APPROVAL-RESUME-PREWIRE-CORRECTION.md`
- Scope: read-only audit plus this verifier artifact and its ledger row. No
  product, schema, route, runtime, Cargo, dependency, test, or frozen-file edits.
- Frozen APP012 read-test SHA-256:
  `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`

## Verdict

**ACCEPT-SCOPED.** The correction satisfies R1-R6 as a proposal-only prewire
boundary. It separates current seams from future APIs, classifies existing-seam
REDs versus blocked API-dependent work, removes unsupported keyring/device-signature
claims, and preserves the frozen APP012 hash. This is not product, RED, GREEN, or
APP012 acceptance.

The proposed seam is not present in product code and must not be treated as an
implementation. The correction states this explicitly at lines 61-65 and
repeats the absence of coordinator/routes at lines 487-526. Any future prewire
must implement real validation, persistence, broker checks, and bounded lifetime;
compile-only structs or no-op methods would violate the stated boundary.

## R1-R6 audit

| Requirement | Evidence | Result |
|---|---|---|
| R1 exact prewire seam | `APP012-APPROVAL-RESUME-PREWIRE-CORRECTION.md:59-320` publishes proposed module paths, auth/origin types, digest types, coordinator request/result/error signatures, storage request/error APIs, route paths, and caller rules. It explicitly labels them proposed, not current (`:61-65`). | PASS scoped; implementation still blocked | 
| R2 keyring/device-signature honesty | `...PREWIRE-CORRECTION.md:322-351` removes the prior keyring and signing-key assumptions. Current `crates/providers/src/auth_store.rs:1-7,62-79` is planning-only and caller-supplied capability state. Current `crates/server/src/remote_revocation.rs:141-193,216-271` has opaque challenge/credential plus epoch checks, not signature verification. | PASS | 
| R3 digest reconciliation | `...PREWIRE-CORRECTION.md:353-408` maps `blake3-256-v1` exactly to storage version `1` and raw 32-byte BLAKE3, separates UUID `ApprovalId`, rejects legacy FNV/par005 values, and states that current `OperationDigest(String)` remains legacy-only. | PASS scoped; encoder/migration not landed | 
| R4 expiry transaction semantics | `...PREWIRE-CORRECTION.md:410-446` uses `now_us < expires_at_us`, marks equality expired, limits expiry transaction to durable `0 -> 3` CAS/metadata, and keeps resume claim/effect outside the transaction. | PASS | 
| R5 decision kinds | `...PREWIRE-CORRECTION.md:448-485` includes both `None` and `Pending`, stable values, explicit cancel state 5, state mapping, and retention requirement. | PASS | 
| R6 missing route/coordinator and RED owner | `...PREWIRE-CORRECTION.md:487-526` explicitly records no current coordinator/routes, names the required integration seams, assigns one future server RED file, and forbids fake/`#[path]`/ignored tests. Current `crates/server/src/lib.rs:271-335` has no approval routes. | PASS | 

## Existing-seam REDs

Manifest A is compile-feasible in principle against current public symbols. No
Manifest A test file exists in this revision, so no compile failure is being
misreported as RED.

- A1, expiry: `crates/security/src/app_policy.rs:72-181,219-243,288-338`
  exposes `Scope`, `Grant`, `ExpectedScope`, `PolicyDeny`, `Grant::covers`, and
  `decide`; equality currently passes because the check is `expected.now >
  expires_at` (`:160-162`). A test can call the public seam and assert the
  existing typed `Grant::covers` failure path at a later boundary. `decide`
  itself returns `AppDecision` with a redacted reason, so a test must not claim
  that current `decide` exposes a typed `Expired` value.
- A2, state-3 retention: `crates/storage/src/retention_v2.rs:60-153` exposes
  `RetentionV2::sweep_resolved_approvals` and `retention_backlog`; public
  `SchemaV2` is re-exported by `crates/storage/src/lib.rs:21-23,39-42`. The
  current SQL only selects states 1, 2, 4 (`retention_v2.rs:97-100,141-144`),
  so a disposable fixture can compile and exercise the missing state-3 behavior.
- A3, non-read broker gate: `crates/tools/src/executor.rs:45-147` exposes
  `ToolCall`, `ToolExecutor`, and `execute_authorized`; security exposes
  `PermissionBroker`, `PermissionSet`, `PermissionRule`, and `RuleEffect`
  (`crates/security/src/lib.rs:101-161`). Current non-read calls fall through to
  unbrokered `execute` (`executor.rs:138-146`), giving a real existing seam for a
  side-effect assertion.
- A4, outbox admission/error: `crates/storage/src/writer_v2.rs:99-105,355-382`
  exposes `V2Writer::append_outbox_event` and checks the v2 4096-byte bound;
  `SchemaV2` and `StorageError` are public. The distinction is real:
  `writer_v2.rs:16,364-365` uses 4096 while `crates/storage/src/lib.rs:53-79`
  formats `EventPayloadTooLarge` with the older public 64 KiB constant. The
  correction correctly classifies 4097 rejection as regression/GREEN, not a
  missing-behavior RED (`...PREWIRE-CORRECTION.md:528-553`).

No Manifest A test was authored, edited, or run in this verifier lane. Cargo was
not run, per task scope.

## API-dependent blockers

Manifest B remains correctly blocked. Current source grep finds no product
symbols for `HumanPrincipal`, `TransportOrigin`, `ApprovalCoordinator`,
`CanonicalDigest`, `DigestAlgorithmVersion`, `DecisionKind`, `claim_resume`,
`approval_auth`, `approval_digest`, or `approval_coordinator`. The only current
approval digest implementations are legacy FNV variants:

- `crates/security/src/app_policy.rs:35-60` uses FNV-1a-64 hex;
- `crates/tools/src/app_services.rs:310-355` uses `par005-v1` four-lane FNV;
- `crates/storage/schema/v2/workspace.sql:222-240` has no versioned digest or
  proposed bindings;
- `crates/storage/src/approvals_v2.rs:26-80` returns SQLite `last_insert_rowid()`
  rather than wire `ApprovalId`.

The correction keeps B1-B9 blocked at
`...PREWIRE-CORRECTION.md:555-573`, including auth/origin, coordinator,
canonical digest, schema/UUID, atomic claim, durable caps/restart, outbox
integration, keyring durability, and device signature. It does not invent a
current route, coordinator, keyring backend, signature verifier, or passing RED.

## Seam honesty and residual implementation gate

The code blocks at `...PREWIRE-CORRECTION.md:67-320` are proposed signatures,
not product code. The correction explicitly requires real validation and rejects
empty compile-only bodies (`:61-65`). The current router has only session/model/
turn routes (`crates/server/src/lib.rs:271-335`), and current bearer middleware
only authenticates transport (`crates/server/src/daemon_auth.rs:150-175`).
Therefore no invented/stub seam is counted as landed.

One future implementation review remains mandatory: the proposed digest example
currently shows a `ConcreteOperation::Tool` minimum (`:129-145`), while the
behavioral contract requires typed file/process/SQL/tool adapters
(`APP012-APPROVAL-RESUME-CORRECTION.md:456-515`). The implementation lane must
expand that seam before enabling R8/B3/B2 REDs; this does not invalidate the
proposal-only R1-R6 correction or authorize a RED now.

## Frozen integrity and tree scope

- `shasum -a 256 crates/server/tests/app012_tool_journey_red.rs` matches the
  frozen value above.
- `git diff --name-only 61b0959^ 61b0959` contains only
  `tasks/completion/claims.json` and
  `worklog/APP012-APPROVAL-RESUME-PREWIRE-CORRECTION.md`.
- No `crates/`, `tests/`, schema, route, runtime, Cargo, dependency, or frozen
  test file changed in the candidate commit.
- `git diff --check` passes.

## Validators

- `python3 tools/validate_repository.py`: FAIL, inherited repository-wide
  backlog-exhaustion errors (51 errors). No controller or verifier input was
  edited.
- `python3 tools/convergence_gate.py`: BLOCKED, inherited ledger findings
  (95 on this worktree after this verifier claim). No product conclusion follows.
- No Cargo, SQLite, daemon, browser, provider, keyring, credential, or live-auth
  process was run.

## Remaining boundaries

1. Serialized integration lane must land the real prewire, including concrete
   operation variants, storage migration/API, coordinator ownership, routes, and
   continuation lifetime.
2. Auth/security review must establish real principal/origin proof; opaque
   keyring or device challenge strings remain insufficient.
3. Independent RED author must add only compiling behavioral tests after prewire,
   freeze hashes, and prove first behavioral failures.
4. APP012 parent remains open until approval, resume, denial, restart, brokered
   effect, ambiguity/reconciliation, and installed journey pass independently.
