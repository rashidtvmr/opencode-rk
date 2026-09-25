# APP012-APPROVAL-RESUME-CORRECTION-VERIFY

## Claim

- Task: `APP012-APPROVAL-RESUME-CORRECTION-VERIFY`
- Type: independent contract verification (research)
- Session: `ses_f296640c1ffesCceFQZuKG9ZN4`
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/verify-app012-approval-resume-correction`
- Branch: `verify/APP012-APPROVAL-RESUME-CORRECTION`
- Candidate revision: `6d2f97f90478abf1dd484e3865d8aa0fd72630bc`
- Correction under review: `worklog/APP012-APPROVAL-RESUME-CORRECTION.md` (commit `6d2f97f`)
- Original contract: commit `3f7405e`; original verifier: commit `1f53b01`, verdict `REVISE BEFORE RED`
- Owned artifact: this file and this task's ledger row only.
- Scope: independent verification. No product, schema, route, runtime, Cargo, test, database, or frozen-file edits.
- Frozen APP-012 read test SHA-256:
  `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`

## Verdict

**REVISE.** The correction is source-grounded and faithfully addresses the nine
`REVISE BEFORE RED` items from `1f53b01`; its external-effect honesty (one
decision, one claim, no blind replay, no exactly-once claim for unobservable
effects) is correct and should be preserved. It is **not admissible as an
acceptance-for-RED authorization yet**, because the REDs it enumerates cannot
compile against any existing public seam, the contract does not publish the exact
prewire seam (module paths and signatures) required before a compiling RED, and
correction 2/3 cite a "keyring" and "device/clientsignature" path that does not
exist as accepted product code. No BLOCK: no source, schema, route, test, or
policy was changed and no safeguarded behavior is weakened.

## Nine-correction verification

| # | Correction | Source check | Result |
|---|---|---|---|
| 1 | Inclusive expiry `now < expires_at`; equality expired | `app_policy.rs:160` uses `expected.now > expires_at` (equality allowed); `approvals_v2.rs:139` uses `expires_at_us <= now` (equality expired); `remote_approvals.rs:246,328` use `now_ms > expires_at_ms`. Correction harmonizes to equality-expired; new `MAX_APPROVAL_TTL` is not present in source. | PASS (design) with prewire |
| 2 | Daemon bearer is transport-only; human decision needs trusted principal | `daemon_auth.rs:150-175` `require_bearer` validates only the daemon token. No `HumanPrincipal` or principal type exists (`grep principal` -> 0 files). Local client proof names the platform keyring path; product keyring is research-only and its blackbox RED is BLOCKED (`4814357`). Remote device signature verification absent (`pair.rs` has no crypto; `remote_revocation.rs` is challenge/epoch/watermark only). | REVISE: "existing keyring/client-credential path" and "device signing key" are not existing seams |
| 3 | `local_only` mandatory on canonical coordinator path | `enforce_local_only` exists (`app_policy.rs:344`), `decide` does not call it, `authorize_with_grant` does not call it (`tool_authorize.rs:138-161`). No coordinator and no `is_local_peer()`/`TransportOrigin`. | PASS (design) with prewire |
| 4 | One versioned digest, BLAKE3-256 v1, raw 32 bytes, explicit UUID mapping | `app_policy.rs:48` FNV-1a-64 hex; `app_services.rs:310-337` `par005-v1` 32 bytes; storage requires 32-byte `intent_hash`; `approvals_v2.rs:79` returns `last_insert_rowid`. `blake3` declared in `crates/storage/Cargo.toml` only; absent from `crates/security/Cargo.toml` and `crates/tools/Cargo.toml`. Public `Grant.digest: OperationDigest(String)` not addressed. | PASS (design) with prewire; internal ID/version mismatch (see R3) |
| 5 | Schema/API owns missing bindings and UUID identity | `workspace.sql:222-249` has no workspace_path, requester_text, principal, decision_kind, owner_generation, dispatch_nonce, reconciliation_key, digest_algorithm_version, retention_until_us. Contract flags as integration proposal. | PASS (design); hard prewire before RED |
| 6 | Atomic claim transaction; effect outside commit | `approvals_v2.rs:26-80` request inserts only a row, no outbox; `execution_v2.rs` transitions are separate; no combined approval+tools+execution+nonce+outbox transaction. Pseudocode is correct and prevents blind replay. | PASS (design) with prewire; editorial ambiguity (see R4) |
| 7 | state 3 retention and recovery protection | `retention_v2.rs:91-105` sweeps states 1,2,4 only; `:136-146` backlog counts 1,2,4. `expire_sweep` (`approvals_v2.rs:136-143`) sets `state=3` only, no `resolved_at_us`, no `decision_kind`. Contract captures both gaps. | PASS (design) with prewire |
| 8 | Resume reauthorizes concrete operation | `executor.rs:136-147` `execute_authorized` falls through to unbrokered `execute` for any name other than `read`; `FileAction::Read` is the only brokered file path. Correction correctly requires typed adapters and broker revalidation. | PASS (design); partly testable on existing seam |
| 9 | Pending-only cap and exact 4096-byte outbox admission | `remote_approvals.rs:207` caps `g.requests.len()` (all, not pending); SQL `workspace.sql:293-300` enforces `<= 4096` bytes; `writer_v2.rs:16,364` private 4096 check; `lib.rs:54,671` generic 64 KiB check on `recent_events`. | PASS (design) with prewire |

Overall: all nine verifier corrections are addressed with correct source citations
(the citation in correction 4 for `crates/storage/Cargo.toml:10` was not line-pinned
but `blake3.workspace = true` is present).

## Compile-readiness of the proposed REDs (reject compile-fail paths)

The correction labels its assertions "Future compiling RED assertions". Independent
check of every proposed assertion against existing public symbols:

Symbols the REDs need and that do **not** exist anywhere in `crates/`:
`HumanPrincipal`, `TransportOrigin`, `PrincipalMismatch`, `UnsupportedDigestVersion`,
`DigestMismatch`, `reconciliation_key`, `dispatch_nonce`, `decision_kind`,
`owner_generation`, `retention_until_us`, `MAX_APPROVAL_TTL`, `ApprovalCapacity`,
`StaleBinding`, `UnsupportedOperation`, `MAX_OUTBOX_PAYLOAD_BYTES`,
`is_local_peer`, verify_signature, signing_key, peer_credential, ucred.
Every `grep -c` over `crates/` returns 0 files.

What can actually compile today versus what cannot:

- Compiles today, fails for cause (valid compiling RED on existing seams):
  - correction 1 security boundary: `app_policy::{decide, Grant, Scope, ExpectedScope, PolicyDeny}` plus `PermissionBroker`, `OperationIntent`, `FileAction` are public; a test asserting equality-expiry is denied compiles and currently fails.
  - correction 1 storage boundary: `ApprovalsV2::expire_sweep` is public; equality already expires, but `resolved_at_us`/`decision_kind` assertions need new columns and fail to compile.
  - correction 7 retention: `RetentionV2::sweep_resolved_approvals` and `retention_backlog` are public; a state-3-not-deleted/state-3-backlog test compiles and currently fails.
  - correction 8 non-read fallback: `ToolExecutor::execute_authorized`, `PermissionBroker`, `ToolCall` are public; a non-read-denied test compiles and currently fails.
  - correction 9 outbox byte bound via `V2Writer::append_outbox_event` compiles; 4097 rejection already holds, so only the durable-coordinator part fails.
- Cannot compile today (compile-fail path, must be prewired first):
  - correction 2 principal/auth REDs: no principal type, no challenge/signature verifier.
  - correction 3 coordinator local-only REDs: no coordinator and no origin type.
  - correction 4 digest REDs: security/tools have no `blake3` dependency; no versioned encoder type; `OperationDigest` is a 16-hex `String`.
  - correction 5 schema/UUID REDs: request returns `i64`, new columns absent.
  - correction 6 atomic claim REDs: no combined transaction API.
  - correction 9 durable pending-cap REDs: no durable coordinator.

Conclusion: the proposed RED matrix is **not compilable against existing public
seams**. A compiling RED requires an explicit prewire lane that publishes the public
types, module paths and signatures first. Per the "reject compile-fail RED paths"
rule, this must be enumerated in the contract before RED is authorized.

## Ownership DAG, failure, persistence, lifetime, resource bounds

- Ownership DAG is coherent and correctly forbids leaf edits to shared schema,
  route registry, central runtime, frozen tests, and accepted policy.
- Failure/persistence/lifetime/resource bounds are present per correction
  (typed errors, rollback-on-failure, 500-row sweeps, 1024 pending, 16/session,
  1024 waits, 64 KiB read, 4096-byte outbox, 30-day retention).
- No-blind-replay invariant is explicit and correct: state 4 alone never triggers a
  replay; unknown effect goes to reconciliation.
- Gap: the DAG names owners but not concrete modules/signatures, so no single-file
  owner can be assigned a compiling RED yet.
- Gap: the caps 16/session and 1024/workspace are new normative bounds, not derived
  from an existing durable constant (remote 1024 is process-local). They are labelled
  normative, which is acceptable, but a durable coordinator API is a prewire.

## Frozen hash, read-only diff, validators

- `shasum -a 256 crates/server/tests/app012_tool_journey_red.rs` ->
  `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50` (matches claim;
  identical at `3f7405e`).
- `git diff --name-only 3f7405e 6d2f97f` -> only
  `tasks/completion/claims.json`, `worklog/APP012-APPROVAL-RESUME-CONTRACT-VERIFY.md`,
  `worklog/APP012-APPROVAL-RESUME-CORRECTION.md`.
- `git diff 1f53b01 6d2f97f -- crates/ tests/ web/ config/ sources/ scripts/` -> empty.
- Commit `6d2f97f` touches only `tasks/completion/claims.json` and
  `worklog/APP012-APPROVAL-RESUME-CORRECTION.md`. No schema/product/test change.
- `git diff --check` -> exit 0.
- `python3 tools/validate_repository.py` -> FAIL `backlog exhaustion` with 51
  pre-existing errors (accepted/unknown stories in the exhaustion ledger, stale
  ownership gaps). Unrelated to this lane; no source change made.
- `python3 tools/convergence_gate.py` -> total=93 pre-existing off-plan ledger
  findings; exit 0. No source change made.

## Required revisions before RED authorization

R1. Publish the exact prewire seam before any RED: module paths and public
    signatures for the coordinator, `HumanPrincipal`, `TransportOrigin`,
    decision/resume request and result types, typed error variants
    (`Expired`, `Unauthenticated`, `PrincipalMismatch`, `Revoked`, `RemoteOrigin`,
    `DigestMismatch`, `UnsupportedDigestVersion`, `StaleBinding`,
    `UnsupportedOperation`, `ApprovalCapacity`), the digest encoder, and the
    storage typed API. Without this every enumerated RED is a compile-fail path.

R2. Correct the source characterization in corrections 2 and 3. There is no
    accepted product "platform keyring/client credential path" (keyring is a
    research seam; its blackbox RED is BLOCKED at `4814357`) and no product
    device/client signature verification (`pair.rs` is opaque-challenge only;
    `remote_revocation.rs` has no signature check). Either mark these as NEW
    prewire (with owner and acceptance gate) or name an existing transport proof.

R3. Reconcile the digest identifiers: the contract uses the string ID
    `blake3-256-v1` and integer `digest_algorithm_version = 1`. State the single
    mapping (version 1 == `blake3-256-v1`) and state whether the existing public
    `Grant.digest: OperationDigest(String)` and its hex `as_str` are replaced or
    superseded, including how existing `app_policy` unit tests move.

R4. Clarify correction 6 pseudocode: "rollback/commit expiry only" should state
    that the transaction commits the `1 -> 3` expiry CAS while every other effect
    and the dispatch are skipped, so the expiry is durable without a claim.

R5. Add the missing `pending`/`none` variant to the `decision_kind` enum in the
    correction 5 table; the persistence paragraph already writes
    `decision_kind=none/pending` for the request transaction.

R6. State explicitly that there is currently no approval decision/resume route and
    no coordinator module in `crates/server/src` (route table has none), so the RED
    target file and its imports can be assigned to a single owner.

## Conditional RED plan (only after R1-R6)

If the revisions land, a minimal compiling RED is feasible with these exact
single-file owners (one owned file per lane, shared `lib.rs`/schema prewired first
by an integration lane):

- Prewire (integration, one lane): add `pub mod` lines and public type stubs with
  real signatures; add the schema migration; add `blake3` to
  `crates/security/Cargo.toml` and `crates/tools/Cargo.toml`.
- Security expiry + local-only RED:
  `crates/security/tests/app012_approval_expiry_red.rs`; imports
  `opencode_rk_security::app_policy::{decide, Grant, Scope, ExpectedScope, PolicyDeny}`,
  `opencode_rk_security::{OperationIntent, FileAction, PermissionBroker, SecurityPolicy, PermissionSet}`.
- Storage retention state-3 RED:
  `crates/storage/tests/app012_approval_retention_red.rs`; imports
  `opencode_rk_storage::{ApprovalsV2, RetentionV2, SchemaV2, StorageError}`.
- Storage schema/UUID/claim RED:
  `crates/storage/tests/app012_approval_claim_red.rs`; imports the new typed API
  from R1.
- Tools concrete-dispatch RED:
  `crates/tools/tests/app012_concrete_dispatch_red.rs`; imports
  `opencode_rk_tools::executor::{ToolExecutor, ToolCall}`,
  `opencode_rk_tools::file_ops`, `opencode_rk_security::PermissionBroker`.
- Server coordinator/outbox RED:
  `crates/server/tests/app012_approval_resume_red.rs`; imports
  `opencode_rk_server::...` coordinator, `opencode_rk_storage`, `opencode_rk_security`.

Command shape for each (macOS-portable, bounded, disposable fixture):
`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 180 cargo test -p <crate> --test <target> -- --test-threads=1`

No implementation acceptance is granted by this artifact.

## Validation record

```sh
git rev-parse HEAD                     # 6d2f97f90478abf1dd484e3865d8aa0fd72630bc
shasum -a 256 crates/server/tests/app012_tool_journey_red.rs
git diff --name-only 3f7405e 6d2f97f
git diff 1f53b01 6d2f97f -- crates/ tests/ web/ config/ sources/ scripts/
git diff --check
python3 tools/validate_repository.py
python3 tools/convergence_gate.py
```

- No Cargo, SQLite, browser, daemon, provider, or live-auth process was run in this
  verification lane; all checks are read-only source/hash/validator inspection.

## Remaining unknowns

- Prewire seam (R1) is undefined; no compiling RED is possible before it.
- Keyring and device-signature proof are unaccepted product gaps (R2).
- Schema authority, migration/API design, and restart recovery wiring remain open.
- No approval coordinator, route, or continuation exists; the parent APP-012 stays
  open until the real installed journey is wired and independently verified.