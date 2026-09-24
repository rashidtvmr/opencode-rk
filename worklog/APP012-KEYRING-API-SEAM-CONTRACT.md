# APP012 keyring API seam contract

## Claim and boundary

- Task: `APP012-KEYRING-API-SEAM-CONTRACT`
- Type: research
- Session: `ses_f2b9b3e85ffe5ZlwMKaE4KodzM`
- Branch: `plan/APP012-KEYRING-API-SEAM`
- Owned file: this worklog and this task row in `tasks/completion/claims.json`
- No product source, test, Cargo manifest, lockfile, verifier, controller, or
  policy edits.

This is the compilable seam proposal required before the independent keyring RED.
It is not implementation, dependency acceptance, RED, GREEN, native platform
proof, runtime wiring, or APP012 acceptance.

## Source evidence and current boundary

Authoritative repository evidence:

- `Cargo.toml:18-40`: workspace edition 2021, Rust 1.85, Tokio available with
  `sync`, `time`, and multithread runtime features. No `keyring` dependency.
- `crates/providers/Cargo.toml:9-20`: providers currently has no `keyring`
  dependency. Adding it is a later serialized dependency lane.
- `crates/providers/src/lib.rs:4-59`: no `keyring_persistence` module is wired.
- `crates/providers/src/auth_store.rs:1-8,92-123,268-276,366-373`: existing
  `auth_store` is planning-only. It validates 128-byte provider IDs and 64 KiB
  secrets, but does no keyring I/O. Its `StoreBackend::resolve` permits a
  `File0600` fallback and is forbidden on the APP012 keyring-or-deny path.
- `crates/providers/src/auth_profile.rs:15-43,128-159,221-245`:
  `AuthProvenance::Keyring` and status projections are secret-free. They do not
  persist or load a credential.
- `crates/cli/src/onboarding.rs:147-215,326-405,548-570`: `SecretString` is
  redacted, while `MemoryAccountStore` and the onboarding commit path remain
  in-memory. There is no production keyring caller.
- `crates/cli/src/daemon_client.rs:775-793`: `creds_configured` checks only
  non-empty environment variables.
- `crates/providers/src/config.rs:147-150` and
  `crates/providers/src/responses.rs:516-523`: provider requests currently
  read environment variables only.
- `crates/security/src/credentials.rs:91-93`: zeroization is explicitly not
  implemented. The seam must not claim memory erasure.
- `worklog/APP012-KEYRING-CONTRACT-CORRECTION.md:117-164`: exact 3.6.3
  feature/API matrix; baseline uses `set_password` and `get_password`.
- `worklog/APP012-KEYRING-CONTRACT-CORRECTION.md:166-200`: domain-separated
  data-directory/provider identity derivation.
- `worklog/APP012-KEYRING-CONTRACT-CORRECTION.md:231-281`: v3 mock is
  `EntryOnly`; durable reconstructed-store proof requires an application-owned
  shared backend state and native A/B/C receipts.
- `worklog/APP012-KEYRING-CONTRACT-CORRECTION.md:283-316`: owned blocking
  calls, two-operation bound, bounded admission, five-second deadline, drain,
  no replay, and same-entry ordering requirements.
- `worklog/APP012-KEYRING-CONTRACT-CORRECTION.md:318-338`: scope-drop and
  redaction requirements; no application zeroization guarantee.
- `worklog/APP012-KEYRING-CONTRACT-CORRECTION.md:379-401`: K01-K10 RED
  taxonomy and independent RED ownership.
- `worklog/APP012-KEYRING-CONTRACT-CORRECTION-VERIFY.md:41-71,73-102`:
  exact package source verification. `Entry::delete_credential`,
  `set_default_credential_builder(mock::default_credential_builder())`, and
  `CredentialPersistence::EntryOnly` are the valid v3.6.3 facts.
- `crates/server/tests/app012_tool_journey_red.rs`: unrelated frozen APP012
  tool RED, SHA-256
  `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`; it
  must remain byte-identical.

Exact local dependency source checked:
`$HOME/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/keyring-3.6.3`.
Relevant source: `src/lib.rs:322-377,395-445,488-512`,
`src/error.rs:15-59`, and `src/mock.rs:185-235`. Native dependency feature
selection remains a later accepted change: macOS `apple-native`, Linux
`linux-native-sync-persistent` plus `crypto-rust`, Windows `windows-native`.
Bare 3.6.3 is not product persistence because its fallback is mock.

## Exact public module and types

### Module path

The implementation module is:

```text
crates/providers/src/keyring_persistence.rs
opencode_rk_providers::keyring_persistence
```

The serialized integration lane adds `pub mod keyring_persistence;` to
`crates/providers/src/lib.rs`. This worklog does not edit that shared file.

### Raw synchronous backend

The API seam must expose one object-safe, synchronous, native-operation trait.
The trait has no `keyring::Entry` in its signature, so orchestration compiles
before dependency acceptance and the native adapter remains replaceable only at
the approved backend boundary.

```rust
use std::sync::Arc;

pub trait RawCredentialBackend: Send + Sync + 'static {
    fn set_password(
        &self,
        service: &str,
        account: &str,
        password: &str,
    ) -> Result<(), RawBackendError>;

    fn get_password(
        &self,
        service: &str,
        account: &str,
    ) -> Result<String, RawBackendError>;

    fn delete_credential(
        &self,
        service: &str,
        account: &str,
    ) -> Result<(), RawBackendError>;
}

pub type BackendHandle = Arc<dyn RawCredentialBackend>;
```

The three methods are the only raw operations. `exists` is implemented by a
bounded `get_password` followed by immediate drop of the returned value; the
backend does not get a separate probe that could have different durability
semantics. Native calls are synchronous and run only inside the service's owned
`spawn_blocking` tasks.

Raw errors are fixed, payload-free, and safe to copy or format:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RawBackendError {
    Missing,
    Locked,
    Unavailable,
    CorruptEncoding,
    InvalidIdentity,
    Oversize,
    Ambiguous,
}
```

The production adapter maps exact 3.6.3 errors without formatting or retaining
them:

| `keyring::Error` | `RawBackendError` | Rule |
|---|---|---|
| `NoEntry` | `Missing` | Missing is not a backend failure. |
| `NoStorageAccess(_)` | `Locked` | Fixed application code; attached OS detail discarded. |
| `PlatformFailure(_)` | `Unavailable` | Fixed application code; attached OS detail discarded. |
| `BadEncoding(_)` | `CorruptEncoding` | Raw bytes discarded immediately. |
| `Invalid(_, _)` | `InvalidIdentity` | Attribute/reason text discarded. |
| `TooLong(_, _)` | `Oversize` | Attribute text and limit discarded. |
| `Ambiguous(_)` | `Ambiguous` | Matching credentials never escape. |
| future non-exhaustive variant | `Unavailable` | Fail closed. |

`Locked` is the semantic classification of v3 `NoStorageAccess`; an individual
platform can report an unavailable store through that same native variant. The
application code for both classifications is `keyring_unavailable`. No raw
error source is exposed.

### Application errors and outcomes

The application error carries no path, provider text, secret, OS text, raw
encoding bytes, or source error:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum StoreError {
    InvalidDataDirectory,
    InvalidProvider,
    EmptySecret,
    SecretTooLarge,
    KeyringLocked,
    KeyringUnavailable,
    CorruptEncoding,
    InvalidIdentity,
    BackendOversize,
    Ambiguous,
    DeadlineExceeded,
    CapacityExceeded,
    ShuttingDown,
}

impl StoreError {
    pub const fn code(self) -> &'static str;
}
```

Required stable codes:

| Variant | Code |
|---|---|
| `InvalidDataDirectory` | `keyring_data_dir_invalid` |
| `InvalidProvider` | `provider_invalid` |
| `EmptySecret` | `secret_empty` |
| `SecretTooLarge` | `secret_too_large` |
| `KeyringLocked` | `keyring_unavailable` |
| `KeyringUnavailable` | `keyring_unavailable` |
| `CorruptEncoding` | `keyring_corrupt_encoding` |
| `InvalidIdentity` | `keyring_identity_invalid` |
| `BackendOversize` | `keyring_oversize` |
| `Ambiguous` | `keyring_ambiguous` |
| `DeadlineExceeded` | `keyring_timeout` |
| `CapacityExceeded` | `keyring_capacity` |
| `ShuttingDown` | `keyring_shutdown` |

Missing is a typed result, not an error:

```rust
#[derive(Debug, Eq, PartialEq)]
pub enum LoadOutcome {
    Present(LoadedSecret),
    Missing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Presence {
    Present,
    Missing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeleteOutcome {
    Deleted,
    AlreadyMissing,
}
```

`save` returns `Result<(), StoreError>` and means stored or overwritten final
state. It makes no cross-platform atomicity claim. `load` returns
`Present(LoadedSecret)` or `Missing`. `delete` maps raw `Missing` to
`AlreadyMissing`; native `delete_credential` remains non-idempotent. `exists`
returns `Presence` without returning a secret.

`LoadedSecret` has a private `String` field, no `Clone`, no `Serialize`, no
`AsRef<str>`, no `Deref`, and no public conversion to an owned string. Its
only exposure method is:

```rust
impl LoadedSecret {
    pub fn with_exposed<R>(&self, use_secret: impl FnOnce(&str) -> R) -> R;
}

impl std::fmt::Debug for LoadedSecret {
    // Exact output contract: "LoadedSecret(<redacted>)".
}

impl std::fmt::Display for LoadedSecret {
    // Exact output contract: "[REDACTED]".
}
```

The caller invokes the provider request inside `with_exposed`; it drops the
loaded value at the end of that authorized scope. This is a lifetime rule, not
a zeroization guarantee. The store's `save` accepts `&str` only for the
duration of the call and never clones it into store state. CLI onboarding can
bridge its existing `SecretString` through its exposed closure without making
the provider crate depend on the CLI crate.

### Namespace identity

Identity labels are hash-derived and contain no raw path or provider ID:

```rust
#[derive(Clone, Eq, PartialEq)]
pub struct CredentialIdentity {
    // Private service/account labels.
}

impl CredentialIdentity {
    pub fn service(&self) -> &str;
    pub fn account(&self) -> &str;
}

pub fn derive_identity(
    canonical_data_dir: &std::path::Path,
    provider_id: &str,
) -> Result<CredentialIdentity, StoreError>;
```

The `canonical_data_dir` precondition is trusted startup ownership. The helper
does not silently canonicalize a path or resolve a symlink. The startup owner
must canonicalize first, then pass an absolute, non-empty, UTF-8 path. The
store retains only derived labels, not the path.

Exact derivation:

```text
service = "opencode-rk-v1"
namespace = lower_hex(SHA-256("opencode-rk-v1\0" || canonical_data_dir))[0..32]
provider_tag = lower_hex(SHA-256("opencode-rk-provider\0" || provider_id))[0..32]
account = "p:" || provider_tag || ":d:" || namespace
```

The 32-character slices are the first 16 digest bytes rendered as lowercase
hex. Provider validation is non-empty, at most 128 UTF-8 bytes, and no NUL.
The helper rejects invalid data-directory identity before any backend call.
The fixed service and derived account labels are checked against the native
platform's length limits by the production adapter and map rejection to
`InvalidIdentity` or `Oversize`.

`CredentialIdentity` debug/display output may contain only the fixed service
and derived hex labels. No raw path or provider string is allowed in any
diagnostic surface. `service()` and `account()` are needed by a durable fake to
key the exact identity; they do not expose sensitive material.

### Service ownership and reconstructed stores

The long-lived owner is `KeyringService`; store instances are disposable views.
The injected `BackendHandle` and operation runtime are held behind `Arc`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoreLimits {
    pub operation_deadline: std::time::Duration,
    pub max_pending: usize,
}

impl StoreLimits {
    pub const fn production() -> Self;
    pub fn bounded(
        operation_deadline: std::time::Duration,
        max_pending: usize,
    ) -> Result<Self, StoreError>;
}

impl Default for StoreLimits {
    fn default() -> Self { Self::production() }
}

pub struct KeyringService { /* BackendHandle + owned runtime */ }

impl KeyringService {
    pub fn from_backend(backend: BackendHandle) -> Self;
    pub fn from_backend_with_limits(
        backend: BackendHandle,
        limits: StoreLimits,
    ) -> Result<Self, StoreError>;
    pub fn store(
        &self,
        canonical_data_dir: &std::path::Path,
    ) -> Result<KeyringStore, StoreError>;
    pub fn resources(&self) -> ResourceSnapshot;
    pub async fn shutdown(&self) -> Result<ShutdownReceipt, StoreError>;
}

pub struct KeyringStore { /* Arc<KeyringService> + namespace labels */ }

impl KeyringStore {
    pub fn from_backend(
        backend: BackendHandle,
        canonical_data_dir: &std::path::Path,
    ) -> Result<Self, StoreError>;

    pub async fn save(
        &self,
        provider_id: &str,
        secret: &str,
    ) -> Result<(), StoreError>;

    pub async fn load(
        &self,
        provider_id: &str,
    ) -> Result<LoadOutcome, StoreError>;

    pub async fn delete(
        &self,
        provider_id: &str,
    ) -> Result<DeleteOutcome, StoreError>;

    pub async fn exists(
        &self,
        provider_id: &str,
    ) -> Result<Presence, StoreError>;

    pub fn identity_for(
        &self,
        provider_id: &str,
    ) -> Result<CredentialIdentity, StoreError>;
}
```

`KeyringStore::from_backend` is the compact RED constructor. It must retain the
caller-supplied `Arc` backend, so a reconstructed store can use the same
durable backend handle. Production runtime wiring should construct one
`KeyringService`, retain it as the daemon-owned owner, and create views with
`service.store(canonical_data_dir)`. `shutdown` is called by that daemon owner
before service teardown. A store cannot substitute a file, environment, config,
SQLite, or in-memory fallback when the backend fails.

The minimal fixed production bounds are:

```text
MAX_PROVIDER_ID_BYTES = 128
MAX_SECRET_BYTES = 64 * 1024
MAX_IN_FLIGHT = 2 per KeyringService
MAX_PENDING = 2 per KeyringService
OPERATION_DEADLINE = 5 seconds, covering admission and result wait
same-entry serialization = fixed lock stripes, not an unbounded identity map
```

`StoreLimits::bounded` may only lower the production deadline and pending cap;
it rejects a deadline above five seconds, zero, or a pending cap above two.
The native in-flight cap remains two and is not caller-configurable. A bounded
pending counter must be checked before waiting; an unbounded Tokio semaphore
wait list is not acceptable.

### Resource observation and cancellation

The runtime exposes only bounded, non-secret counters:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceSnapshot {
    pub in_flight: usize,
    pub pending: usize,
    pub draining: usize,
    pub max_in_flight: usize,
    pub max_pending: usize,
    pub operation_deadline: std::time::Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShutdownReceipt {
    pub drained: usize,
}
```

Each admitted operation owns one blocking task and one permit. The service
starts `tokio::task::spawn_blocking` only after bounded admission. The same
operation future owns its `JoinHandle`; if its deadline expires, it returns
`DeadlineExceeded`, moves the handle into the service-owned bounded drain set,
and does not replay save or delete. If the future is cancelled after native
start, its drop guard performs the same ownership transfer. Cancellation before
admission releases the pending slot and performs no backend call.

The service-owned drain monitor retains every started handle until completion.
The monitor itself is owner-held and joined by `shutdown`; it is not a detached
task. `resources()` must show in-flight, pending, and draining counts during a
slow fake operation. `shutdown` waits for all started calls and returns a
bounded shutdown failure if its own drain deadline is exceeded. The API makes
no claim that an OS keyring call can be interrupted at five seconds. Ambiguous
save/delete side effects are never automatically retried.

Same-entry access is serialized by a fixed number of lock stripes selected from
the derived account label. This avoids an unbounded per-identity lock map while
ensuring Windows/Linux keyring access for one entry has deterministic ordering.

## Production adapter boundary

The adapter is real code, not a test stub, but it lands only after exact
dependency and target-feature acceptance. Its public shape is:

```rust
pub struct KeyringBackend;

impl KeyringBackend {
    pub const fn new() -> Self;
}

impl RawCredentialBackend for KeyringBackend {
    // Entry::new(service, account)
    // set_password(password)
    // get_password()
    // delete_credential()
}
```

Each method constructs `keyring::Entry::new(service, account)` inside the
owned blocking task and maps its result to `RawBackendError`. The adapter uses
exact 3.6.3 names. It does not use `delete_password`, `set_default_mock!`, or
`MockEntryBuilder`. It never calls `set_secret`/`get_secret` in the baseline
UTF-8 contract. The adapter has no file fallback and no environment read.

The pre-RED seam can be real and non-stub without the dependency: all public
types, validation, namespace derivation, bounded runtime, cancellation/drain
owner, and `KeyringStore` orchestration run against an injected
`RawCredentialBackend`. After dependency acceptance, `KeyringBackend` is the
real native adapter. The durable fake is test infrastructure only; it is not a
default backend and cannot be selected implicitly by a production constructor.

## Small compile-usage example

This example is the intended API compile check. `DurableFakeBackend` is a RED
fixture implementing the public synchronous trait over shared state. The fake
must key its state by the exact `service()` and `account()` labels it receives,
not provider ID alone. Its state survives store reconstruction because the
caller retains the `Arc`:

```rust
use std::{path::Path, sync::Arc};
use opencode_rk_providers::keyring_persistence::{
    BackendHandle, DeleteOutcome, KeyringStore, LoadOutcome, Presence,
};

let backend: BackendHandle = Arc::new(DurableFakeBackend::new());
let root = Path::new("/canonical/disposable/app012");

let first = KeyringStore::from_backend(backend.clone(), root)?;
first.save("openai", "synthetic-canary").await?;
drop(first);

let second = KeyringStore::from_backend(backend.clone(), root)?;
match second.load("openai").await? {
    LoadOutcome::Present(secret) => {
        secret.with_exposed(|value| assert_eq!(value, "synthetic-canary"));
    }
    LoadOutcome::Missing => panic!("expected durable fake value"),
}
assert_eq!(second.exists("openai").await?, Presence::Present);
assert_eq!(second.delete("openai").await?, DeleteOutcome::Deleted);
assert_eq!(second.delete("openai").await?, DeleteOutcome::AlreadyMissing);
```

The example is only a compile-usage check and must not be used as native
durability evidence. The RED must also compile against the production adapter
surface after dependency integration, and K10 must exercise the retained-entry
3.6.3 mock API/error mapping. K01-K09 may use the shared durable fake to prove
application lifecycle and executor semantics, but passing those cases with an
unwired fake alone is not acceptance.

## Exact future RED imports and K01-K10 boundary

The independent RED imports the seam from the real providers crate, not a local
module copy:

```rust
use opencode_rk_providers::keyring_persistence::{
    BackendHandle,
    DeleteOutcome,
    KeyringService,
    KeyringStore,
    LoadOutcome,
    Presence,
    RawBackendError,
    RawCredentialBackend,
    ResourceSnapshot,
    StoreError,
};
```

The RED may additionally import `KeyringBackend` only after the dependency
adapter is accepted and wired. It must not import a test-only backend from the
production module. K01-K10 assertions:

| ID | Seam assertion | Fake/native boundary |
|---|---|---|
| K01 | Save, drop store A, reconstruct B with same `BackendHandle`, load same value. | Durable shared fake; proves orchestration, not OS durability. |
| K02 | Same identity overwrite returns new value only. | Durable fake plus production adapter final-state test. |
| K03 | Missing is `LoadOutcome::Missing` and `Presence::Missing`; no fallback write. | Durable fake; assert backend call trace and zero file/config/DB effects. |
| K04 | Present delete is `Deleted`; absent delete is `AlreadyMissing`. | Durable fake and retained native mock operation mapping. |
| K05 | Locked/unavailable maps to `keyring_unavailable`; no plaintext fallback. | Inject raw errors, then verify production adapter has no fallback path. |
| K06 | Bad UTF-8 maps to `CorruptEncoding`; raw bytes absent from every output. | Raw error fixture plus exact 3.6.3 `BadEncoding` mapping. |
| K07 | Empty, NUL/invalid, and oversized input call backend zero times. | Instrumented bounded fake; validation before admission. |
| K08 | Namespace separates canonical roots/providers and leaks no raw labels. | Exact derived identity assertions. |
| K09 | Pending cap, five-second-or-lower deadline, cancellation, draining, and shutdown counters are observable. | Slow owned fake; no detached task or replay. |
| K10 | Exact 3.6.3 retained-entry mock builder and `MockCredential::set_error` mapping. | `set_default_credential_builder(mock::default_credential_builder())`; not restart proof. |

K01-K09 do not authorize claiming native persistence. A verifier must inspect
that runtime wiring selects `KeyringBackend` on supported targets and run the
disposable native A/B/C receipt. The v3 mock is explicitly `EntryOnly`; it
cannot prove a newly reconstructed store can retrieve a value.

## Dependency and implementation order

Exact order, with ownership split:

1. **API seam implementation.** Add the module types, raw trait, validation,
   namespace derivation, redacted loaded-secret wrapper, bounded service/store
   orchestration, and owner-held drain logic. Compile this layer against the
   existing workspace dependencies with no keyring adapter or fallback.
2. **Independent compile/API verification.** Compile a usage harness against
   `opencode_rk_providers::keyring_persistence`; verify object safety,
   `Send + Sync + 'static` backend ownership, async signatures, redacted
   formatting, exact outcome/error codes, and namespace vectors. This is API
   verification, not behavioral acceptance.
3. **RED authoring and freeze.** Independently author
   `crates/providers/tests/keyring_persistence_red.rs`, using the exact imports
   above and a shared-state durable fake. Establish compiling RED for the
   missing behavior, run it, freeze its SHA-256 and command manifest. Do not
   edit the existing frozen APP012 tool RED.
4. **Dependency/native adapter.** In a serialized integration lane add exact
   `keyring = "=3.6.3"` with target feature rows, regenerate `Cargo.lock`,
   wire `KeyringBackend`, and compile K10. Native target selection must never
   silently select the mock for a product target.
5. **Runtime wiring.** Separately wire onboarding commit, startup
   `creds_configured`, and provider request loading. Preserve env compatibility
   only as an explicitly separate source. APP012 save/restart failure is
   keyring-or-deny: do not call `StoreBackend::resolve`, write File0600, write
   config/SQLite, or silently replay a save/delete.
6. **Platform receipts.** Independent verifier runs disposable process A/B/C
   native proof: macOS Keychain, Linux Secret Service persistent combo, and
   Windows GNU Credential Manager. Receipt includes target, exact package and
   features, lock hash, backend availability, independent process evidence,
   final state, cleanup, resource observations, and redaction scan. Unavailable
   native backend is a blocker, not a pass.

## Validation performed

Commands run in this research lane:

```text
rtk python3 tools/convergence_gate.py
CONVERGENCE BLOCKED; 93 pre-existing ledger/backlog findings.

rtk git grep -n 'keyring\|credential\|api_key\|onboard' -- crates Cargo.lock Cargo.toml
PASS for source orientation; current product remains dependency-free and env-only.

rtk shasum -a 256 crates/server/tests/app012_tool_journey_red.rs
945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50

rtk git diff --check
PASS before commit.

rtk python3 tools/validate_repository.py
FAIL expected repository-wide backlog/protection findings; no source or policy
changes were made by this lane.
```

No Cargo command, product test, dependency edit, keyring call, real credential
access, RED authoring, native receipt, or runtime wiring was performed. No
acceptance claim is made.

## Remaining blockers

- The API seam still requires an implementation lane and independent compile
  verification before RED authoring.
- Dependency/feature/lockfile acceptance is separate and serialized.
- The frozen keyring RED does not yet exist; its hash must be produced by the
  independent RED author.
- Native A/B/C receipts and runtime caller wiring remain open.
- Zeroization remains intentionally unclaimed; only scope-drop and redaction
  guarantees are valid.
