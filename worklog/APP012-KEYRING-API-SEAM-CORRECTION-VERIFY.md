# APP012-KEYRING-API-SEAM-CORRECTION-VERIFY

## Scope

- Task: `APP012-KEYRING-API-SEAM-CORRECTION-VERIFY`
- Session: `ses_f2b72911cffed5MyTRY0HbGHlI`
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/verify-app012-keyring-api-seam-correction`
- Branch: `verify/APP012-KEYRING-API-SEAM-CORRECTION`
- Candidate correction: `55174dc83edf9df197d102522d2889c9af3d6408`
- Prior independent audit: `beffcb555323cf0dde5e75ee83ca1de9335b996f`
- Owned files: this worklog and this task's `tasks/completion/claims.json` row only
- No Cargo, product, test, dependency, lockfile, native-store, credential, or runtime-wiring changes performed.

## Decision: B — REJECT bootstrap authorization

The correction is materially better: it records the bootstrap problem, defines an
exact candidate API, and makes the prior `f92facb` deadlock explicit. It does
**not** authorize A.

`docs/TDD.md:21-48` and `:58-66` require this order:

1. author tests;
2. establish a compiling RED that executes and fails behaviorally;
3. freeze test source and command manifest;
4. implement behavior.

`docs/TDD.md:45-48` expressly rejects missing imports, compile failures, tests
that never execute, and disabled tests. No cited policy clause grants a worker
or correction lane an implementation-before-RED exception. The correction
itself says the same at `worklog/APP012-KEYRING-API-SEAM-CORRECTION.md:76-116`:
the controller must record a checkpoint and may choose a strict or checkpoint
route. That sentence is a request for authority, not authority granted by the
repository policy.

A is therefore not accepted:

- no `crates/providers/src/lib.rs` edit;
- no `crates/providers/src/keyring_persistence.rs` edit;
- no compile-only behavioral substitute;
- no implementation, test, Cargo, lockfile, target, adapter, or caller work;
- no implementation authorization and no RED authorization.

The correction lane is complete as a **REVISE/REJECT verification result**. APP012
remains blocked at the TDD governance boundary.

### Candidate A surface (for controller decision only; not authorized)

If a trusted controller later amends authority, the narrow A candidate would be
limited to:

- `crates/providers/src/lib.rs`: one module export,
  `pub mod keyring_persistence;`.
- `crates/providers/src/keyring_persistence.rs`: declarations/signatures only
  for the public names `MAX_PROVIDER_ID_BYTES`, `MAX_SECRET_BYTES`,
  `MAX_DATA_DIR_BYTES`, `MAX_SERVICE_LABEL_BYTES`, `MAX_ACCOUNT_LABEL_BYTES`,
  `MAX_IN_FLIGHT`, `MAX_PENDING`, `OPERATION_DEADLINE`, `SHUTDOWN_GRACE`,
  `RawBackendError`, `RawCredentialBackend`, `BackendHandle`,
  `CanonicalDataDir`, `SecretInput`, `LoadedSecret`, `CredentialIdentity`,
  `derive_identity`, `StoreError`, `LoadOutcome`, `Presence`, `DeleteOutcome`,
  `LifecycleState`, `StoreLimits`, `ResourceSnapshot`, `ShutdownReceipt`,
  `KeyringService`, and `KeyringStore`.

A may not claim any operation outcome. It may not mention `keyring::Entry`, do
I/O, select a backend, read environment/config/SQLite, add `keyring` or target
features, add a test, add `keyring_adapter`, use `todo!()`/`unimplemented!()`,
fabricate success, or wire a caller. A bounded
`cargo check -p opencode-rk-providers` would prove names/signatures only, not
behavior or acceptance. The controller must explicitly grant this exception;
this verifier does not.

## Nine-group audit

The correction addresses each prior blocker in contract text. Group 1 remains a
live governance blocker. Groups 2–9 are conditional contracts, not executed
proof.

### 1. RED lifecycle order — unresolved authority blocker

- Correction evidence: `APP012-KEYRING-API-SEAM-CORRECTION.md:64-116`.
- It correctly identifies absent symbols and refuses a compile-failing RED.
- It proposes a controller checkpoint, but does not create or record one.
- Its “checkpoint route” would deliberately implement behavior before RED.
  That conflicts with binding `docs/TDD.md:33-48` unless a higher authority
  changes policy.
- Disposition: **REJECT pending test-owner/integration-authority decision**.

### 2. K10 adapter testability — addressed provisionally

- Correction evidence: `:722-793`.
- K10 is split into K10a retained `EntryOnly` mock and K10b `EntryFactory`
  adapter mapping.
- The retained entry can receive `MockCredential::set_error`; the adapter
  factory preserves fresh production entries while enabling controlled test
  injection.
- Disposition: textually resolved; no dependency, target, native, or test
  evidence exists. K10a remains explicitly non-durable.

### 3. K06 `BadEncoding` mapping — addressed provisionally

- Correction evidence: `:795-801`.
- K06-app uses payload-free `RawBackendError::CorruptEncoding`; K10b directly
  maps synthetic `keyring::Error::BadEncoding(Vec<u8>)`, drops bytes, and
  checks fixed projections.
- Disposition: textually resolved; adapter mapping and redaction remain
  unexecuted.

### 4. Secret ownership / `spawn_blocking` — addressed provisionally

- Correction evidence: `:212-231`, `:477-495`, API `:382-386`.
- `SecretInput(String)` is owned, non-`Clone`, and moved into `save`; the
  admitted blocking task owns the string until join settles.
- The contract permits exactly one bounded caller-owned copy and disclaims
  zeroization/backend copies.
- Disposition: textually resolved; cancellation, post-start drain, and no-call
  behavior still require executable tests.

### 5. Executor/lifecycle/resource semantics — addressed provisionally

- Correction evidence: `:497-578`.
- Private validated limits, fixed two permits, bounded pending count, disjoint
  counters, native-start marker, owner-held join handles, timeout/cancel drain,
  fixed shutdown grace, concurrent/idempotent shutdown, post-shutdown rejection,
  panic/cancel mapping, and no replay are specified.
- Disposition: textually resolved; K09 must prove actual joins, counters,
  cancellation side effects, and shutdown states. No implementation exists.

### 6. Service ownership / canonical namespace — addressed with stated ceiling

- Correction evidence: `:199-206`, `:437-475`, API `:354-379`.
- `CanonicalDataDir::from_canonical` is a bounded, redacted capability
  constructor; `KeyringStore` retains derived labels only; service/view
  ownership is separated.
- The trusted startup canonicalization precondition remains explicit; it is not
  a proof against a lying caller.
- Platform-specific “unsupported or ambiguous” path rules still need
  implementation and target evidence.
- Disposition: contract ceiling recorded; no runtime proof.

### 7. Production orchestration/caller seam — addressed as future work

- Correction evidence: `:841-876`, lane order `:896-929`.
- Proposed future owners: `crates/server/src/daemon.rs`,
  `crates/cli/src/onboarding.rs`,
  `crates/cli/src/daemon_client.rs::creds_configured`,
  `crates/providers/src/responses.rs`; `StoreBackend::resolve`, File0600,
  config, SQLite, and implicit environment fallback are excluded.
- Broker authorization and target backend selection are required but exact
  broker call-site symbols/integration proposal remain future authority.
- Disposition: no current caller or daemon ownership exists; no runtime claim.

### 8. Diagnostic/logging boundary — addressed with dependency ceiling

- Correction evidence: `:606-612`.
- Guarantee is application-owned fixed-code redaction. Linux dependency debug
  output is disclosed; global suppression is not claimed. A target filter would
  need a separate observability owner and test.
- Disposition: textually resolved; no logger/filter/receipt evidence.

### 9. Loaded-secret / zeroization — addressed with explicit ceiling

- Correction evidence: `:489-495`, `:841-870`.
- `LoadedSecret::with_exposed` is lifetime discipline only; callers must not
  clone, retain, log, serialize, store in a client, or return it.
- No zeroization, malicious-callback prevention, or backend-copy control is
  claimed.
- Disposition: ceiling recorded; provider caller refactor and real scope proof
  remain absent.

## Exact future RED imports and failure cause

These are future imports only. They do not authorize a RED now.

### K01–K09 dependency-free seam

`crates/providers/tests/keyring_persistence_red.rs` must import exactly the
candidate public surface:

```rust
use opencode_rk_providers::keyring_persistence::{
    BackendHandle,
    CanonicalDataDir,
    DeleteOutcome,
    KeyringService,
    LifecycleState,
    LoadOutcome,
    Presence,
    RawBackendError,
    RawCredentialBackend,
    ResourceSnapshot,
    SecretInput,
    StoreError,
    StoreLimits,
};
```

The test owns a deterministic `DurableFakeBackend`; it is injected through
`BackendHandle`, not a production default. It must not import `keyring`, touch
native storage, environment, filesystem credentials, SQLite, or a real
credential. The first valid failure must be an **executed behavioral assertion**
after the real dependency-free orchestration has admitted and completed the
operation, for example the K01 canary assertion:

```rust
assert_eq!(value, "synthetic-canary");
```

The failure must not be a missing symbol/import, compile error, zero tests,
fixture panic, `#[ignore]`, or test-side fake of the service. Current tree has
no admitted seam or real orchestration, so this RED cannot yet be authored
under the binding order.

The proposed future command manifest is:

```text
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120 cargo test -p opencode-rk-providers --test keyring_persistence_red -- --nocapture --test-threads=1
```

Freeze the source SHA-256 only after that command compiles, executes, and fails
behaviorally. No keyring RED hash exists now.

### K10 dependency/adapter boundary

K10 is separate from K01–K09. K10a owns
`crates/providers/tests/keyring_mock_api.rs` and imports:

```rust
use keyring::{
    mock,
    set_default_credential_builder,
    Entry,
    Error,
};
use keyring::mock::MockCredential;
use keyring::credential::{CredentialBuilderApi, CredentialPersistence};
```

It must serialize process-global mock setup, retain the `Entry`, downcast its
credential, inject a synthetic one-shot error, assert
`CredentialPersistence::EntryOnly`, and avoid restart/native claims.

K10b is a private adapter mapping test in
`crates/providers/src/keyring_adapter.rs`, using the approved `EntryFactory`
seam. It must map `BadEncoding(Vec<u8>)` to payload-free
`RawBackendError::CorruptEncoding` without formatting or retaining bytes. It is
not caller or persistence evidence.

## Remaining platform/runtime evidence

No evidence below exists on `55174dc`; all remains future work:

1. Controller authority decision resolving the RED-before-behavior conflict.
2. Exact dependency-free seam and orchestration implementation only after that
   decision.
3. Compiling, executing, frozen K01–K09 RED source hash and command manifest.
4. K10a retained-entry mock API/error test and K10b adapter mapping test.
5. Exact `keyring = "=3.6.3"` target features, lockfile resolution, supported
   target registration, and unsupported-target failure.
6. Daemon-owned service, broker grants, onboarding save-before-commit,
   startup presence, provider scoped-load refactor, and no-fallback journey.
7. Application redaction scans and resource/cancellation/security regressions.
8. Independent disposable native A/B/C receipts on the exact integrated
   revision:
   - macOS: `apple-native`, Keychain A/B/C;
   - Linux: `linux-native-sync-persistent` + `crypto-rust`, active Secret
     Service A/B/C;
   - Windows GNU: `windows-native`, actual GNU build and Generic Credential
     Manager A/B/C.
9. Lock/hash, target triple, feature resolution, availability, independent
   process outcomes, bounded cleanup, and redaction evidence per receipt.
10. Independent integrated verifier acceptance. Module compilation, mock GREEN,
    fake GREEN, and unit GREEN cannot satisfy parent acceptance.

The existing unrelated frozen file
`crates/server/tests/app012_tool_journey_red.rs` remains untouched with
SHA-256:

```text
945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50
```

## Validation and landing

Allowed read-only checks:

```text
rtk shasum -a 256 crates/server/tests/app012_tool_journey_red.rs
945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50

rtk git diff --check
PASS

rtk git status --short --branch
Expected during landing: owned worklog plus own ledger row only.
```

No Cargo command, product/test/dependency edit, native keyring call, credential
access, RED authoring, implementation, or runtime wiring was performed.

Verification result: **B — REJECT**. Required owner decision: trusted
controller/integration authority must either record an explicit policy
amendment/checkpoint with scope, owner, and test-freeze authority, or supply a
policy-compliant pre-existing real seam. Until then, do not implement or
freeze a behavioral RED.
