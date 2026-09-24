# APP012 keyring API seam correction

## Claim, boundary, verdict

- Task: `APP012-KEYRING-API-SEAM-CORRECTION`
- Type: research and contract correction
- Session: `ses_f2b5ec88bffe51XYIMn0lgMD1h`
- Branch: `plan/APP012-KEYRING-API-SEAM-CORRECTION`
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/plan-app012-keyring-api-seam-correction`
- Owned artifact: this file and this task's completion-ledger row only
- User-approved route: no product, test, dependency, Cargo, native-store, or
  runtime-caller edits

Verdict: **REVISE resolved as a contract proposal, not implementation
authorization.** The nine groups from `beffcb5` are resolved below. A second
independent verifier must accept this artifact and the bootstrap checkpoint
before any implementation or behavioral RED lane starts. No native persistence,
runtime wiring, zeroization, atomicity, or acceptance claim is made.

## Evidence ledger

### Authority and current source

| Evidence | Verified fact | Classification |
|---|---|---|
| `docs/TDD.md:21-41,43-65` | RED must compile, fail behaviorally, freeze, then implementation; compile failure and non-executed tests are invalid | Binding policy |
| `docs/SECURITY.md:9-32,49-60,74-82` | Broker authorization, no plaintext fallback, no secret logging, scoped cancellation, bounded growing resources | Binding policy |
| `PLAN.md:61-85,105-120,162-179,227-239` | One daemon runtime, native Rust behavior, independent verifier, explicit resource bounds | Binding policy |
| `crates/providers/src/lib.rs:4-59` | No keyring persistence module is registered | Current gap |
| `crates/providers/Cargo.toml:9-20` | No `keyring` dependency | Current gap |
| `crates/providers/src/auth_store.rs:62-80,394-430` | Planning-only `StoreBackend::resolve` permits `File0600` fallback | Forbidden APP012 path |
| `crates/cli/src/daemon_client.rs:775-791` | `creds_configured` checks only non-empty environment variables | Current compatibility gap |
| `crates/providers/src/config.rs:147-150` | Provider config reads environment credentials | Current compatibility gap |
| `crates/providers/src/responses.rs:506-550` | `OpenAiResponsesClient` retains `api_key: String` | Caller refactor required |
| `crates/cli/src/onboarding.rs:155-199,326-405,548-570` | `SecretString` is redacted; account commit is in memory | Caller refactor required |
| `crates/security/src/credentials.rs:34-93` | Ordinary credential values have no zeroization guarantee | Explicit ceiling |
| `6dcfe19` | Original seam had API, namespace, executor, and ownership ambiguities | Corrected below |
| `beffcb5` | Independent verifier identified nine REVISE groups | All nine addressed below |
| `f92facb` | No honest compiling behavioral RED existed without an admitted seam | Bootstrap checkpoint required |
| `worklog/APP012-KEYRING-CONTRACT-CORRECTION.md` at `41f37f0` plus its independent verifier | Accepted 3.6.3 API/feature matrix, durable-fake boundary, native A/B/C boundary, no-fallback rule, and no-zeroization ceiling | Preserved and tightened below |
| `crates/server/tests/app012_tool_journey_red.rs` | Existing unrelated APP012 RED; SHA-256 `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50` | Frozen and untouched |

### Exact keyring 3.6.3 source

Source root:
`$HOME/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/keyring-3.6.3`.

- `src/lib.rs:322-377,395-445,488-512`: `Entry::new`,
  `set_password`, `get_password`, `delete_credential`, `get_credential`.
- `src/mock.rs:16-35,185-235`: `mock::default_credential_builder`,
  `MockCredential::set_error`, `CredentialPersistence::EntryOnly`.
- `src/error.rs:15-56,61-100`: `NoEntry`, `NoStorageAccess`,
  `PlatformFailure`, `BadEncoding(Vec<u8>)`, `TooLong`, `Invalid`,
  `Ambiguous`; error is non-exhaustive.
- `src/credential.rs:99-104,137-173`: native deletion is not idempotent;
  `EntryOnly` differs from native durable persistence.
- `src/keyutils_persistent.rs:67-129`: dependency debug logging can format raw
  errors. Application redaction cannot be advertised as global dependency-log
  suppression.
- Tokio 1.53.1 `src/task/blocking.rs:82-120,220-225`: `spawn_blocking`
  closures are `'static`; started blocking work cannot be aborted reliably and
  runtime shutdown can wait for it.

## 1. TDD bootstrap: explicit authority checkpoint

### Problem

The current tree has no public module, backend trait, identity value, service
constructor, or operation symbols. `f92facb` correctly declined to author a
behavioral RED: importing future symbols would be a compile failure, which
`docs/TDD.md:43-48` rejects. The original order in `6dcfe19` also put the full
seam implementation before RED.

### Honest resolution

The repository policy does not grant a worker permission to invent a compiling
RED against absent symbols. The controller must record this exact
**APP012-KEYRING-BOOTSTRAP checkpoint** before either the seam or RED lane:

1. A second independent verifier reviews this file, the API fragment below,
   ownership split, and the no-fallback rules. It must explicitly accept the
   checkpoint; this lane cannot self-authorize it.
2. The controller may authorize one dependency-free bootstrap consisting only
   of the public module export and the exact public type/interface surface below.
   It may not add `keyring`, target features, a lockfile entry, native calls,
   fallback behavior, caller wiring, or a test.
3. The bootstrap owner runs only a bounded `cargo check -p
   opencode-rk-providers`. It reports symbols and signatures, not behavior.
4. The controller then chooses one of two explicit routes:
   - **Strict route:** refuse implementation before RED. The task remains
     blocked because the current tree has no real orchestration against which a
     compiling behavioral RED can run. The controller must first admit a
     pre-existing real implementation or revise the authority contract.
   - **Checkpoint route:** explicitly authorize the dependency-free
     orchestration owner to implement real injected-backend behavior, still
     without native keyring, fallback, caller wiring, or test edits. This is a
     recorded controller exception to the literal RED-before-behavior order,
     not permission inferred from this note. It is the minimum exception needed
     to produce the requested executable RED.
5. Only on the checkpoint route, and only after that real orchestration
   implementation compiles, may the independent RED owner author
   `crates/providers/tests/keyring_persistence_red.rs`. The RED must compile and
   fail at a behavioral assertion against that real orchestration, then its
   source hash and command are frozen by the trusted controller. After the RED
   hash is frozen, all further behavior changes follow the normal
   RED-before-GREEN sequence; no worker may alter the frozen tests.
6. If the controller refuses step 4 because `docs/TDD.md` permits no
   implementation-before-RED exception, the honest result is **BLOCKED**: no
   compiling behavioral RED can exist on the current tree. The controller must
   first revise the process authority or admit a pre-existing real
   implementation. A compile-only generic test, `#[ignore]` test, local fake
   pretending to be production, or `todo!()` body is not a workaround.

This checkpoint is a governance decision, not an implementation authorization
from this lane. No implementation authorization exists until the second
independent verifier accepts it.

### Smallest admitted dependency-free compile seam

The bootstrap may register this export in `crates/providers/src/lib.rs` and add
one owned `crates/providers/src/keyring_persistence.rs`:

```rust
pub mod keyring_persistence;
```

`keyring_adapter` and unsupported-target cfg are later dependency-owned
registration. The `keyring_persistence.rs` file may contain the exact public
declarations and real method signatures below. It must not mention
`keyring::Entry`, perform I/O, select a backend, read an environment variable,
write a file or database, or claim an operation result. Shared `lib.rs` is a
separate integration owner; no leaf lane edits it.

The code block below is a signature-accurate API fragment, not a copy-paste
implementation: function bodies are intentionally omitted from this research
artifact. The bootstrap's compile check must use bodies that contain no
placeholder, `todo!()`, `unimplemented!()`, fabricated success, or hidden
fallback. A successful bootstrap compile proves only that the public names and
types are admitted; it proves no behavior.

## 2. Corrected public API fragment

The following is the exact target surface. Paths and field layouts marked
private are intentional. Names in this fragment are not permission to edit
product files in this lane.

```rust
use std::{
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

pub const MAX_PROVIDER_ID_BYTES: usize = 128;
pub const MAX_SECRET_BYTES: usize = 64 * 1024;
pub const MAX_DATA_DIR_BYTES: usize = 1024;
pub const MAX_SERVICE_LABEL_BYTES: usize = 32;
pub const MAX_ACCOUNT_LABEL_BYTES: usize = 96;
pub const MAX_IN_FLIGHT: usize = 2;
pub const MAX_PENDING: usize = 2;
pub const OPERATION_DEADLINE: Duration = Duration::from_secs(5);
pub const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

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

#[derive(Eq, PartialEq)]
pub struct CanonicalDataDir {
    path: PathBuf,
}

impl CanonicalDataDir {
    pub fn from_canonical(path: PathBuf) -> Result<Self, StoreError>;
}

impl std::fmt::Debug for CanonicalDataDir {
    // Exact output is a fixed redacted capability label; no path bytes.
}

pub struct SecretInput(String);

impl TryFrom<String> for SecretInput {
    type Error = StoreError;

    fn try_from(value: String) -> Result<Self, Self::Error>;
}

impl SecretInput {
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}

impl std::fmt::Debug for SecretInput {
    // Exact output: "SecretInput(<redacted>)".
}

impl std::fmt::Display for SecretInput {
    // Exact output: "[REDACTED]".
}

#[derive(Eq, PartialEq)]
pub struct LoadedSecret(String);

impl LoadedSecret {
    pub fn with_exposed<R>(&self, use_secret: impl FnOnce(&str) -> R) -> R;
}

impl std::fmt::Debug for LoadedSecret {
    // Exact output: "LoadedSecret(<redacted>)".
}

impl std::fmt::Display for LoadedSecret {
    // Exact output: "[REDACTED]".
}

#[derive(Clone, Eq, PartialEq)]
pub struct CredentialIdentity {
    service: String,
    account: String,
}

impl std::fmt::Debug for CredentialIdentity {
    // Exact output contains only the fixed service and derived hex labels.
}

impl CredentialIdentity {
    pub fn service(&self) -> &str;
    pub fn account(&self) -> &str;
}

pub fn derive_identity(
    data_dir: &CanonicalDataDir,
    provider_id: &str,
) -> Result<CredentialIdentity, StoreError>;

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
    InvalidLimits,
    TaskCancelled,
    TaskPanicked,
    TaskJoinFailed,
    ShutdownDeadlineExceeded,
}

impl StoreError {
    pub const fn code(self) -> &'static str;
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleState {
    Running,
    Draining,
    Stopped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoreLimits {
    operation_deadline: Duration,
    max_pending: usize,
}

impl StoreLimits {
    pub const fn production() -> Self;
    pub fn bounded(
        operation_deadline: Duration,
        max_pending: usize,
    ) -> Result<Self, StoreError>;
    pub const fn operation_deadline(&self) -> Duration;
    pub const fn max_pending(&self) -> usize;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceSnapshot {
    pub state: LifecycleState,
    pub in_flight: usize,
    pub pending: usize,
    pub draining: usize,
    pub max_in_flight: usize,
    pub max_pending: usize,
    pub operation_deadline: Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShutdownReceipt {
    pub drained: usize,
    pub completed: usize,
    pub failed: usize,
}

pub struct KeyringService {
    // Private owner of BackendHandle, admission state, permits, and drain set.
}

impl KeyringService {
    pub fn new(backend: BackendHandle) -> Self;
    pub fn with_limits(
        backend: BackendHandle,
        limits: StoreLimits,
    ) -> Result<Self, StoreError>;
    pub fn store(
        &self,
        data_dir: &CanonicalDataDir,
    ) -> Result<KeyringStore, StoreError>;
    pub fn resources(&self) -> ResourceSnapshot;
    pub async fn shutdown(&self) -> Result<ShutdownReceipt, StoreError>;
}

pub struct KeyringStore {
    // Private service owner handle plus only derived namespace labels.
}

impl KeyringStore {
    pub fn identity_for(
        &self,
        provider_id: &str,
    ) -> Result<CredentialIdentity, StoreError>;

    pub async fn save(
        &self,
        provider_id: &str,
        secret: SecretInput,
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
}
```

### Stable codes

`StoreError::code()` returns only these strings. No variant carries a path,
provider, secret, OS error, `keyring::Error`, panic payload, `JoinError`, or
`BadEncoding` bytes.

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
| `InvalidLimits` | `keyring_limits_invalid` |
| `TaskCancelled` | `keyring_task_cancelled` |
| `TaskPanicked` | `keyring_task_panicked` |
| `TaskJoinFailed` | `keyring_task_join_failed` |
| `ShutdownDeadlineExceeded` | `keyring_shutdown_timeout` |

`Debug` and `Display` for `StoreError`, `LoadedSecret`, `SecretInput`, identity,
resource snapshots, and shutdown receipts are application-owned projections.
They must never include raw backend values. The exact identity labels are
derived values only; raw path and provider text are absent.

## 3. Identity and validation contract

`CanonicalDataDir::from_canonical` is a capability constructor, not a
canonicalizer. The trusted data-directory owner first calls the platform
canonicalization operation, then passes the result. The constructor rejects:

- empty path;
- relative path;
- non-UTF-8 path;
- path byte length above `MAX_DATA_DIR_BYTES`;
- `.` or `..` components, parent traversal, repeated separators, or a trailing
  separator except the filesystem root;
- unsupported or ambiguous prefix/component forms for the target platform.

It performs no filesystem access and does not silently resolve symlinks. The
constructor stores the path only until `derive_identity`; `KeyringStore` stores
only derived labels. `CanonicalDataDir` has no public path accessor, `Display`,
or raw-path `Debug` output. This is a caller capability with an explicit
trusted-startup precondition, not proof that a malicious caller cannot lie
about a path.

Provider IDs are non-empty UTF-8, at most 128 bytes, and contain no NUL. Secret
input is non-empty and at most 64 KiB. Validation occurs before identity/backend
construction and before admission. Invalid input produces zero backend calls.

The exact versioned domain-separated identity is:

```text
service = "opencode-rk-v1"
namespace = lower_hex(SHA-256("opencode-rk-v1\0" || canonical_data_dir))[0..32]
provider_tag = lower_hex(SHA-256("opencode-rk-provider\0" || provider_id))[0..32]
account = "p:" || provider_tag || ":d:" || namespace
```

Each 32-character slice is the first 16 digest bytes rendered as lowercase
hex. The expected service/account lengths are checked against the fixed API
bounds and the actual native backend. Privacy means raw path/provider labels
are absent. It does not claim resistance to dictionary guessing of known
paths/providers or cryptographic collision proof from finite tests.

## 4. Secret ownership and lifetime

`SecretInput` is owned, non-`Clone`, and constructed from one caller-owned
`String`. A CLI `SecretString` caller performs exactly one bounded
`to_owned()` inside its broker-authorized save command, then moves that
`SecretInput` into `save`. The service does not clone the input. The admitted
`spawn_blocking` closure owns the one `String` until the backend call and join
settle. On pre-start cancellation it is dropped without a backend call. On
timeout or post-start cancellation the owner-held drain handle retains it until
completion, then drops it. The backend may make its own copies; the application
does not promise zeroization or control those copies.

`LoadedSecret` owns the `String` returned by the backend without an additional
application clone. `with_exposed` is a lifetime discipline only. The caller
must use the borrowed value only in the authorized provider request scope and
must not clone, retain, log, serialize, put it in a client field, or return it
from the callback. No type-level mechanism can prevent a malicious callback
from violating that rule. Stronger memory erasure requires a separate accepted
dependency/API decision and memory proof.

## 5. State machine, admission, and resource accounting

### States and counters

The service has one owner in the daemon and one private synchronized lifecycle:

```text
Running --shutdown begins--> Draining --all started calls joined--> Stopped
Running --post-start timeout/cancel--> Running with a draining handle
Draining --post-shutdown operation--> ShuttingDown result
Stopped --any operation--> ShuttingDown result
```

Counters are disjoint and exact:

- `pending`: admitted requests that have not crossed the native-start marker;
  bounded by `max_pending`, checked and incremented atomically before waiting.
- `in_flight`: started native calls whose caller still owns the join handle;
  bounded by `MAX_IN_FLIGHT`.
- `draining`: started native calls whose caller returned by timeout or
  cancellation and whose join handle is now held by the service drain owner;
  disjoint from `in_flight`.
- `in_flight + draining <= MAX_IN_FLIGHT`; a draining call retains its native
  permit until join completion.
- Total admitted but not fully settled operations are bounded by
  `max_pending + MAX_IN_FLIGHT`, at most four in production. No semaphore wait
  list is used as a queue bound.

Admission order is atomic under one lifecycle lock: reject non-`Running`,
validate provider/identity/secret, reserve a pending slot, acquire one of two
native permits, then start one owned `spawn_blocking` task. A pending slot is
released exactly once on pre-start cancellation, capacity rejection, or native
start. A native-start marker transitions pending to in-flight before the
backend call. The marker and cancellation flag are checked so cancellation
before the marker cannot call the backend.

Operation deadline is `StoreLimits::operation_deadline`, covering bounded
pending admission and result wait. A deadline before the native-start marker
returns `DeadlineExceeded`, cancels the queued task, releases pending, and
performs no backend call. A deadline after the marker returns
`DeadlineExceeded`, transfers the still-owned `JoinHandle` to the service drain
set, retains the permit and secret until join, and never replays `save` or
`delete`. A caller future dropped before start has the same no-side-effect
rule. A caller future dropped after start transfers ownership to the drain set.

The service owns every started handle until completion. No detached Tokio task,
unbounded queue, replay, or OS-call interruption claim exists. A slow fake must
show the counters and eventual decrement after the blocking call ends, not only
theoretical semaphore capacity.

`StoreLimits` fields are private. `production()` is five seconds and pending
cap two. `bounded()` rejects zero deadline, a deadline above five seconds, zero
pending, or pending above two with `InvalidLimits`. `MAX_IN_FLIGHT` is fixed and
not caller-configurable.

### Shutdown, join errors, and panics

`shutdown(&self)` is daemon-owned, idempotent, and concurrent-safe:

1. The first caller atomically changes `Running` to `Draining`, closes new
   admission, rejects/cancels pending requests with `ShuttingDown`, and waits
   for every started handle.
2. Concurrent callers await the same in-progress drain result. They do not
   start another drain or replay an operation.
3. If all handles join within `SHUTDOWN_GRACE`, state becomes `Stopped` and all
   callers receive the same `ShutdownReceipt`. `drained` counts started calls
   whose handles were joined; `completed` counts normal task results;
   `failed` counts fixed-code task failures.
4. If the grace expires, all callers receive `ShutdownDeadlineExceeded`; state
   remains `Draining`, no handle is dropped, and the daemon must retain the
   service owner and call shutdown again. A later call continues waiting with a
   new fixed grace window. Dropping the service while `in_flight + draining >
   0` violates the owner contract and is a verifier failure, not permission to
   detach handles.
5. Once `Stopped`, every operation returns `ShuttingDown` and makes zero
   backend calls. No transition returns to `Running`.

`JoinError::is_panic()` maps to `TaskPanicked` without formatting the panic
payload. A cancelled join maps to `TaskCancelled`; any other join failure maps
to `TaskJoinFailed`. These fixed outcomes are observable only through the
application error/receipt. They never expose a source chain, panic text, OS
detail, secret, or raw `keyring::Error`.

## 6. Raw mapping and outcome matrix

The application backend trait has no `keyring` type. Its sole operations are
the three synchronous calls in the API fragment. `exists` calls
`get_password`, then immediately drops the returned value. Missing is typed,
not an error. Native delete remains non-idempotent; the application maps raw
`Missing` to `AlreadyMissing`.

| Raw 3.6.3 error | `RawBackendError` | Application result |
|---|---|---|
| `NoEntry` | `Missing` | `LoadOutcome::Missing` or `DeleteOutcome::AlreadyMissing` |
| `NoStorageAccess(_)` | `Locked` | `KeyringLocked`, code `keyring_unavailable` |
| `PlatformFailure(_)` | `Unavailable` | `KeyringUnavailable` |
| `BadEncoding(_)` | `CorruptEncoding` | `CorruptEncoding`, attached bytes immediately dropped |
| `Invalid(_, _)` | `InvalidIdentity` | `InvalidIdentity` |
| `TooLong(_, _)` | `Oversize` | `BackendOversize` |
| `Ambiguous(_)` | `Ambiguous` | `Ambiguous` |
| future non-exhaustive variant | `Unavailable` | `KeyringUnavailable`, fail closed |

The adapter never formats or retains a `keyring::Error`. `BadEncoding(Vec<u8>)`
is matched as `BadEncoding(_)`; the bytes are dropped in the mapping function.
The app-level K06 test injects only payload-free `RawBackendError::CorruptEncoding`
and asserts fixed redaction. The direct adapter test separately constructs a
synthetic `BadEncoding(Vec<u8>)`, proves the mapping, and scans application
projections for the synthetic marker.

Application-owned logs, status, telemetry, receipts, errors, `Debug`, and
`Display` contain fixed codes only. This is an application-log guarantee. The
3.6.3 Linux dependency may emit raw error text at debug level through its own
`log` target; this contract does not claim global dependency-log suppression.
If release policy requires that stronger guarantee, a separate observability
owner must configure and test a target filter such as `keyring=off` without
weakening application redaction.

## 7. K01-K09 RED and compile usage

### Future RED imports

The independent K01-K09 test imports only the dependency-free public seam:

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

The test-owned `DurableFakeBackend` is a deterministic injected backend, not a
production default. It owns `Arc<Mutex<HashMap<(String, String), String>>>`
keyed by the exact `CredentialIdentity::service()` and `account()` labels,
plus bounded call/error state. It has no filesystem, network, environment,
SQLite, native keyring, or real credential access. It returns only fixed
`RawBackendError` values and records bounded call counts.

The compile usage after the bootstrap and real orchestration implementation is:

```rust
use std::sync::Arc;

let backend: BackendHandle = Arc::new(DurableFakeBackend::new());
let service = KeyringService::with_limits(
    backend.clone(),
    StoreLimits::production(),
)?;
let canonical_root = std::fs::canonicalize(disposable_fixture_root)
    .map_err(|_| StoreError::InvalidDataDirectory)?;
let root = CanonicalDataDir::from_canonical(canonical_root)?;

let first = service.store(&root)?;
first
    .save(
        "openai",
        SecretInput::try_from(String::from("synthetic-canary"))?,
    )
    .await?;
drop(first);

let second = service.store(&root)?;
match second.load("openai").await? {
    LoadOutcome::Present(secret) => {
        secret.with_exposed(|value| assert_eq!(value, "synthetic-canary"));
    }
    LoadOutcome::Missing => panic!("expected durable fake value"),
}
assert_eq!(second.exists("openai").await?, Presence::Present);
assert_eq!(second.delete("openai").await?, DeleteOutcome::Deleted);
assert_eq!(second.delete("openai").await?, DeleteOutcome::AlreadyMissing);
service.shutdown().await?;
```

The fixture must use a disposable directory created by the test and must
canonicalize it before capability construction. The shown `?` assumes the
test returns `Result<(), StoreError>` only after mapping fixture I/O before the
seam; the final RED should use a local fixture error type rather than adding
I/O variants to `StoreError`.

This test must compile and fail behaviorally, for example at K01's exact
canary assertion, against the real dependency-free orchestration. The RED
author must not replace the service with a test fake, make the assertion
conditional, use `#[ignore]`, or claim native persistence. No keyring import is
allowed in this file.

The frozen RED command manifest is:

```text
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120 cargo test -p opencode-rk-providers --test keyring_persistence_red -- --nocapture --test-threads=1
```

The controller records the source SHA-256 only after this command compiles and
executes a behavioral failure. A compile failure, zero tests, panic from a
missing fixture, or failure caused by an absent import is not RED.

### K01-K09 scenarios and owners

| ID | Behavioral assertion | Required owner/boundary |
|---|---|---|
| K01 | Save, drop view A, reconstruct view B from same daemon service/backend state, load exact value | RED author plus real injected orchestration |
| K02 | Overwrite same identity; only new final value loads | RED author; no atomicity claim |
| K03 | Missing load/presence are typed; no fallback write or call | RED author; assert fake trace and fixture absence |
| K04 | Present delete is `Deleted`; absent delete is `AlreadyMissing` | RED author; native delete remains non-idempotent |
| K05 | Locked/unavailable maps to fixed `keyring_unavailable`; no fallback | RED author; no `StoreBackend::resolve` |
| K06-app | Payload-free corrupt outcome is fixed and app projections omit marker | RED author; does not claim adapter mapping |
| K07 | Empty, oversize, NUL/invalid provider inputs make zero backend calls | RED author; validate before admission |
| K08 | Canonical roots/providers separate; labels contain no raw path/provider | RED author; finite fixture only |
| K09 | Pending cap, timeout, cancellation, drain, shutdown, post-shutdown state and actual join are observable | RED author plus slow fake |

K09 must coordinate a slow backend with deterministic channels, not wall-clock
sleep alone. It must observe `in_flight`, `pending`, `draining`, eventual join,
and backend call count. It must prove cancellation before native start has no
call and cancellation after start drains without replay.

## 8. K10 split: direct EntryOnly mock and approved adapter factory

K10 is not part of K01-K09's dependency-free test file. It has two separately
owned single-file tests with distinct claims.

### K10a: direct retained-entry 3.6.3 mock, non-durable

Owner: `crates/providers/tests/keyring_mock_api.rs`, one dependency/native test
lane after target feature integration. Exact imports:

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

The test runs in its own serialized process/test target because
`set_default_credential_builder` is process-global and has no reset API. It
calls `set_default_credential_builder(mock::default_credential_builder())`,
retains the `Entry`, downcasts `entry.get_credential()` to
`MockCredential`, calls `set_error` with a synthetic fixed error, and checks
the next retained-entry method result. It may test set/get/delete mapping on
that same entry. It must assert `CredentialPersistence::EntryOnly` and must
not reconstruct an entry or claim restart durability. It must never mutate a
developer keychain or use a real credential.

### K10b: approved adapter `EntryFactory`, mapping only

Owner: `crates/providers/src/keyring_adapter.rs`, adapter implementation plus
its private unit test. This file is separate from K10a and from the K01-K09
RED. The private test seam is:

```rust
pub(crate) trait EntryFactory: Send + Sync + 'static {
    fn make(
        &self,
        service: &str,
        account: &str,
    ) -> Result<std::sync::Arc<keyring::Entry>, keyring::Error>;
}

pub struct KeyringBackend {
    factory: std::sync::Arc<dyn EntryFactory>,
}

impl KeyringBackend {
    pub fn new() -> Self;
    #[cfg(test)]
    pub(crate) fn with_entry_factory(
        factory: std::sync::Arc<dyn EntryFactory>,
    ) -> Self;
}
```

Production `DefaultEntryFactory::make` calls exactly
`keyring::Entry::new(service, account)` and wraps the fresh entry in `Arc`.
Each backend operation constructs a fresh production entry through this
factory, then calls exactly `set_password`, `get_password`, or
`delete_credential`. The test factory returns a retained `Arc<Entry>` created
with `Entry::new_with_credential(Box::new(MockCredential::default()))`, so the
test can downcast the retained entry and inject one-shot errors without making
the production backend a mock or retaining entries across operations.

K10b maps exact 3.6.3 errors to `RawBackendError`, including synthetic
`BadEncoding(Vec<u8>)`. It consumes the payload without formatting or retaining
it and asserts only the fixed `CorruptEncoding` result. It does not test
restart, native availability, caller wiring, or application orchestration.

### K06 split

K06-app belongs to the dependency-free K01-K09 RED and injects
`RawBackendError::CorruptEncoding`. K10b directly proves the `BadEncoding` to
`CorruptEncoding` adapter mapping. A direct raw error test must not be used as
the application redaction test, and the app fake must not pretend it can carry
`Vec<u8>` payloads.

## 9. Dependency, feature, and target contract

Dependency integration is serialized and owned separately from the bootstrap,
RED, orchestration, and runtime wiring. It must add exactly version `3.6.3`
with no default features in target-specific tables:

```toml
[target.'cfg(target_os = "macos")'.dependencies]
keyring = { version = "=3.6.3", default-features = false, features = ["apple-native"] }

[target.'cfg(target_os = "linux")'.dependencies]
keyring = { version = "=3.6.3", default-features = false, features = ["linux-native-sync-persistent", "crypto-rust"] }

[target.'cfg(target_os = "windows")'.dependencies]
keyring = { version = "=3.6.3", default-features = false, features = ["windows-native"] }
```

Bare 3.6.3 is forbidden for product builds because its fallback can be the
mock. Linux `linux-native` alone is not restart proof; only the synchronous
persistent Secret Service plus keyutils combination is eligible. `4.2.0` is
forbidden because its declared MSRV exceeds workspace Rust 1.85. The
dependency lane must record the generated `Cargo.lock` package and feature
resolution.

The adapter module is target-gated:

```rust
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub(crate) mod keyring_adapter;

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
compile_error!("APP012 keyring persistence is unsupported on this target");
```

Unsupported targets fail explicitly. They never silently select the mock.
Windows GNU requires an actual `x86_64-pc-windows-gnu` build and receipt; docs.rs
MSVC metadata is not proof.

## 10. Daemon ownership and caller refactors

These are future integration contracts, not current behavior:

1. `crates/server/src/daemon.rs` owns one `KeyringService` for the daemon's
   canonical data directory. Trusted startup canonicalizes the directory,
   constructs `CanonicalDataDir`, selects a supported `KeyringBackend`, and
   retains the service until successful shutdown and drain. A store is a
   disposable view; it retains no raw path.
2. Onboarding at `crates/cli/src/onboarding.rs` validates provider and secret,
   obtains a broker grant bound to provider, canonical data directory,
   operation, expiry, and policy version, then sends one bounded save command
   to the daemon. Account metadata commits only after save succeeds. Save
   failure leaves no File0600/config/SQLite fallback and does not commit the
   account. Existing `SecretString` makes one owned bounded copy for the
   command, never logs or serializes it.
3. Startup status at `crates/cli/src/daemon_client.rs::creds_configured` is
   refactored to request broker-authorized `exists` for each configured
   provider. `Present` permits the authenticated setup path; `Missing` routes
   to `Setup`; `KeyringLocked` or `KeyringUnavailable` denies with the fixed
   code. Environment checks remain a separately labeled compatibility source
   and cannot satisfy APP012 persistence or restart proof.
4. Provider requests in `crates/providers/src/responses.rs` remove the
   long-lived `api_key: String` from the keyring-backed client path. New request
   methods accept `&LoadedSecret`, invoke `with_exposed`, and await the request
   while the loaded value remains in the authorized scope. The value is not
   cloned into a client, stream metadata, transcript, config, SQLite, or
   environment. Streaming callers retain the loaded value until the stream's
   authenticated request scope ends. `from_env` may remain as an explicitly
   separate compatibility source, never as a keyring fallback.
5. `crates/providers/src/auth_store.rs::StoreBackend::resolve` is not called by
   this path. No File0600 writer, config writer, SQLite writer, or implicit
   environment fallback is allowed after a keyring failure.
6. Every save/load/presence call crosses the trusted policy broker. Ordinary
   permission `*` cannot bypass human-only grants or mandatory system
   protection. Broker decisions carry no secret bytes in logs or receipts.

## 11. Native platform evidence boundary

Native acceptance requires independent disposable process A/B/C receipts on the
exact integrated revision:

- macOS: `apple-native`, Keychain save in process A, load/overwrite in B,
  load/delete/missing in C;
- Linux: `linux-native-sync-persistent` plus `crypto-rust`, active Secret
  Service and unlocked collection, same A/B/C sequence;
- Windows GNU: `windows-native`, actual GNU build and Generic Credential
  Manager A/B/C sequence.

Each receipt includes target triple, exact keyring package/features, lock hash,
backend availability, independent process evidence, final outcomes, bounded
cleanup, and redaction scan. Unavailable backend is a blocker, not a pass. No
live developer keychain mutation is authorized. The 3.6.3 mock is limited to
K10a/K10b operation and mapping tests; it is never restart evidence.

## 12. Ordered lanes and single-file ownership

1. **This correction lane**: this worklog and its ledger row only. No
   implementation authorization.
2. **Second independent verifier**: verifies this file, the checkpoint, exact
   signatures, ownership, and frozen unrelated APP012 hash. It does not edit
   product or tests.
3. **Bootstrap seam owner**: one file
   `crates/providers/src/keyring_persistence.rs`; contract-only declarations
   and module export proposal. `crates/providers/src/lib.rs` is owned by the
   serialized integrator, not this leaf.
4. **Dependency-free orchestration owner**: same single product module only
   after the controller records the bootstrap exception. Implements injected
   backend behavior, identity, validation, bounded executor, state machine,
   drain, and redacted outcomes. No native adapter, Cargo, caller, or test
   edits.
5. **Independent K01-K09 RED owner**: one new file
   `crates/providers/tests/keyring_persistence_red.rs`; durable fake and
   behavioral assertions only. It freezes its own source hash after compiling
   and failing behaviorally. It does not edit product or Cargo files.
6. **Dependency and module integration owner**: target-specific dependency
   rows, lockfile, `lib.rs` registration, and target cfg. It does not edit the
   frozen RED.
7. **K10a owner**: one file `crates/providers/tests/keyring_mock_api.rs`, direct
   retained EntryOnly mock only, serialized process-global builder setup.
8. **K10b adapter owner**: one file `crates/providers/src/keyring_adapter.rs`,
   production EntryFactory adapter and private mapping test. No application
   fake or fallback.
9. **Runtime wiring owners**: onboarding, daemon startup/presence, provider
   request scope, and broker call sites. Each shared file needs an integration
   proposal; no hidden leaf edit.
10. **Independent verifier/platform owner**: frozen RED, negative/security and
    resource suites, caller journey, redaction scan, and native A/B/C receipts.
    It issues ACCEPT or REJECT. No lane accepts itself.

The existing `crates/server/tests/app012_tool_journey_red.rs` remains
byte-identical and unrelated. No keyring lane may claim APP012 parent
completion from module compilation, mock GREEN, fake GREEN, or unit GREEN.

## 13. Verification manifest for this research lane

Expected allowed checks after writing this artifact:

```text
rtk shasum -a 256 crates/server/tests/app012_tool_journey_red.rs
945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50

rtk git diff --check
PASS

rtk git grep -n 'keyring\|credential\|api_key\|onboard' -- crates Cargo.lock Cargo.toml
Read-only orientation; current product remains dependency-free and env/in-memory for this path.
```

No Cargo build, product test, dependency edit, native keyring call, credential
access, or test edit is authorized or run by this lane. No keyring RED hash is
fabricated. The frozen unrelated APP012 hash must remain unchanged.

### Executed read-only receipts

```text
rtk git diff --check
PASS

rtk shasum -a 256 crates/server/tests/app012_tool_journey_red.rs
945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50

rtk git status --short
M tasks/completion/claims.json
?? worklog/APP012-KEYRING-API-SEAM-CORRECTION.md
```

Read-only source grep confirmed current product remains without a keyring
dependency or keyring persistence caller; matches are limited to existing
credential planning, environment compatibility, and unrelated security
surfaces. No Cargo command, test, native call, or user-database access ran.

## Remaining acceptance gaps

- Second independent verifier acceptance of this correction and the explicit
  bootstrap checkpoint.
- Controller decision if strict TDD refuses the minimal implementation-before-
  RED exception. Without that decision, a compiling behavioral RED remains
  honestly blocked.
- Real dependency-free orchestration and frozen K01-K09 behavioral RED.
- Exact dependency/feature/lockfile integration.
- K10a retained-entry and K10b EntryFactory adapter tests.
- Daemon broker-authorized caller wiring and loaded-secret refactors.
- Native macOS, Linux Secret Service, and Windows GNU A/B/C receipts.
- Independent integrated verifier result.
- Zeroization, cross-platform atomic overwrite, immediate OS-call interruption,
  and dependency-global log suppression remain explicitly unclaimed.
