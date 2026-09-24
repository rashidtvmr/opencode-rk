# APP012 keyring persistence contract correction

## Claim and boundary

- Task: `APP012-KEYRING-CONTRACT-CORRECTION`
- Task type: research
- Session: `ses_f2cc178b5ffe9zulW64uCTjFxa`
- Branch: `plan/APP012-KEYRING-CONTRACT-CORRECTION`
- Base: `135d26e`
- Owned path: this worklog and this task's row in `tasks/completion/claims.json`
- No product, test, manifest, lockfile, policy, platform store, or verifier edits.
- This document is a corrected proposal, not implementation, dependency acceptance,
  RED evidence, GREEN evidence, or APP-012 acceptance.

## Authority and source classification

Authoritative repository evidence:

- `PLAN.md:23-26`: workspace MSRV is Rust 1.85; `PLAN.md:56-61` requires
  explicit error, ordering, cancellation, cleanup, persistence, and protocol
  semantics; `PLAN.md:72-77` is ADR-002, not the keyring dependency decision.
- `worklog/APP-012.md:70-75`: controller decision authorizes exact
  `keyring` 3.6.3, forbids 4.2.0 for MSRV, prohibits live keychain mutation in
  local tests, and reserves native proof for disposable platform acceptance.
  This task artifact is evidence of the controller decision, not a substitute
  for controller state.
- `docs/TDD.md:11-15,32-41,77-83`: RED freeze, immutable tests, and independent
  verification boundaries.
- `docs/SECURITY.md:7-21,44-61`: capability broker, no plaintext fallback,
  no secret logging, scoped cancellation, and real platform isolation evidence.
- `crates/cli/src/daemon_client.rs:776-793`: current `creds_configured` is
  environment-only and returns `Some(false)` when no listed variable is set.
- `crates/providers/src/config.rs:148-150` and
  `crates/providers/src/responses.rs:516-523`: current provider requests read
  environment variables only.
- `crates/cli/src/app_start.rs:273-321`: startup routes absent credentials to
  `StartupView::Setup`.
- `crates/cli/src/onboarding.rs:326-405,548-570`: current account state and
  commit path are in memory; no production keyring caller exists.
- `crates/providers/src/account_setup.rs:1-8`: caller owns persistence and
  transport; this is not persistence evidence.
- `crates/providers/src/auth_store.rs:24,99-110,268-276`: current planning
  seam has 64 KiB secret and 128 byte provider limits, fixed `StoreError` codes,
  and no keyring I/O. Its `resolve` method permits a File0600 fallback and must
  not be used by APP012 keyring-or-deny execution.
- `crates/security/src/credentials.rs:91-93`: zeroization is not implemented.
- `crates/server/src/daemon.rs:177-272`: existing path identity precedent uses
  normalized data-directory bytes and SHA-256; symlinks are not resolved by the
  helper, so a credential namespace must receive a canonical data directory
  explicitly.
- `crates/server/tests/app012_tool_journey_red.rs`: frozen APP-012 tool RED is
  unrelated to credential persistence; its SHA-256 is
  `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.

Authoritative dependency evidence is the exact `keyring` 3.6.3 package source
and manifest at docs.rs, fetched during this correction:

- `Cargo.toml`: package version `3.6.3`, MSRV `1.75`, no default features.
- `src/lib.rs`: `Entry::new`, `set_password`, `get_password`, `set_secret`,
  `get_secret`, and `delete_credential`; native module selection; mock fallback
  when no applicable feature is enabled.
- `src/mock.rs`: `mock::default_credential_builder()`,
  `set_default_credential_builder`, `MockCredential::set_error`, and
  `CredentialPersistence::EntryOnly`.
- `src/error.rs`: `NoEntry`, `NoStorageAccess`, `PlatformFailure`,
  `BadEncoding`, `TooLong`, `Invalid`, and `Ambiguous`.
- `src/credential.rs`: `CredentialPersistence::UntilDelete` means persistence
  until explicit deletion; `delete_credential` is not idempotent.
- `src/macos.rs`, `src/windows.rs`, `src/keyutils_persistent.rs`: native
  backend behavior and platform caveats.

Upstream docs, package metadata, issue text, prior contract text, and verifier
output are untrusted except where they agree with the exact source above. In
particular, the prior contract's API names and restart claims are corrected here.

## Observable APP012 contract

APP012 credential setup has one allowed durable path:

```
validated onboarding secret
  -> broker-authorized bounded keyring operation
  -> native credential entry
  -> independently reconstructed store/backend instance
  -> provider credential read
```

The keyring path is keyring-or-deny. A keyring error never invokes the PROV-022
File0600 planner as an implicit fallback, never writes config or SQLite, and
never treats an environment variable as a persistence substitute. An explicitly
set environment variable may remain a separate current compatibility source, but
it cannot satisfy the APP012 save/restart proof.

Required public behavior:

1. `save(provider, secret, data_dir)` validates identifiers and secret size
   before constructing an entry, then overwrites the same logical entry.
2. `load(provider, data_dir)` returns a secret only for a durable present entry.
   Missing is a typed absent result, not a backend failure.
3. `delete(provider, data_dir)` maps native `NoEntry` to idempotent application
   success, while native deletion itself remains non-idempotent.
4. `exists` or startup status returns present, missing, or unavailable without
   returning secret bytes.
5. Corrupt/non-UTF-8 data, invalid identity, over-size input, inaccessible
   backend, ambiguous match, and platform failure are typed fixed-code outcomes.
6. No error, debug, trace, transcript, status, provenance, or receipt contains
   secret bytes, OS error text, service/account path material, or the raw bytes
   attached to `BadEncoding`.
7. A provider request may use a loaded secret only inside its bounded caller
   lifetime. No secret is persisted in config, SQLite, transcript, or env by
   this path.

The current env-only startup and in-memory onboarding are gaps, not behavior to
be described as implemented. Runtime wiring requires separate implementation and
integration ownership.

## Exact dependency and feature matrix

The serialized dependency proposal must use exact version `keyring = "=3.6.3"`
and `default-features = false`. There are no keyring default features. Bare
3.6.3 selects the mock backend on a target without an applicable native feature,
so a bare dependency is forbidden for product builds.

| Target policy | Cargo target dependency | Native store | Availability and restart boundary |
|---|---|---|---|
| macOS | `target_os = "macos"`, features `[` `apple-native` `]` | macOS Keychain | Login/user keychain; native receipt required |
| Linux | `target_os = "linux"`, features `[` `linux-native-sync-persistent`, `crypto-rust` `]` | keyutils cache plus synchronous Secret Service | Secret Service and session/unlocked collection required; only this persistent combo is eligible for restart proof |
| Windows | `target_os = "windows"`, features `[` `windows-native` `]` | Windows Generic Credential Manager | Native GNU build receipt required by product policy |
| Other targets | no product dependency/backend | none | Explicit unsupported-target build/configuration failure; never silently use mock as product persistence |

The integration proposal should express the first three as target-specific
dependency tables, each repeating the exact package version. Linux must not use
only `linux-native`: 3.6.3 documents that as keyutils storage with at most
`UntilReboot` lifetime. `linux-native-sync-persistent` combines keyutils with
Secret Service and requires `crypto-rust` or `crypto-openssl`; this contract
chooses `crypto-rust` to avoid an unapproved OpenSSL runtime dependency. The
proposal must record the generated lockfile package and feature resolution, but
this research lane does not edit it.

`keyring` 4.2.0 is forbidden: its upstream manifest declares Rust 1.88, above
the workspace MSRV 1.85. Docs.rs target metadata lists x86_64-pc-windows-msvc;
that metadata does not prove Windows GNU support. GNU support is a product
policy requiring an actual Windows GNU build and native receipt.

## Exact API and representation table

| Operation | Exact 3.6.3 API | Application result | Notes |
|---|---|---|---|
| Construct | `keyring::Entry::new(service, user)` | typed backend/identity error | `Entry::new` may validate platform length/empty constraints |
| Save text | `Entry::set_password(&str)` | `Present` or fixed error | Replaces existing value on supported native stores; final-state proof only |
| Load text | `Entry::get_password()` | `Present(String)`, `Missing`, `CorruptEncoding`, or fixed error | `NoEntry` means absent; `BadEncoding` raw bytes are immediately discarded and never formatted |
| Save bytes | `Entry::set_secret(&[u8])` | not used by baseline | Use only if a separately accepted binary contract exists |
| Load bytes | `Entry::get_secret()` | not used by baseline | Do not switch silently; arbitrary bytes need encoding contract and memory review |
| Delete | `Entry::delete_credential()` | success or application idempotent success | There is no `delete_password` in 3.6.3; native `NoEntry` is mapped to already absent |
| Test builder | `keyring::set_default_credential_builder(keyring::mock::default_credential_builder())` | mock selected before entries | No `set_default_mock!()` and no `MockEntryBuilder` API |
| Test error | downcast `Entry::get_credential()` to `keyring::mock::MockCredential`; call `set_error` | one-shot injected native error | Builder is process-global; serialize setup and restore policy before parallel tests |

Baseline representation is UTF-8 `SecretString` to `set_password` and
`get_password`. The caller rejects empty secrets and values over
`MAX_SECRET_BYTES = 64 * 1024` before backend invocation. `set_secret` and
`get_secret` are not interchangeable with password calls: if future providers
accept arbitrary bytes, add an explicit encoding/version contract, typed
corruption handling, and independent tests. Do not claim that a password API
can round-trip arbitrary bytes.

## Namespace and identity contract

The prior fixed `SERVICE = "opencode-rk"` plus provider-only user was not
workspace/data-directory safe. Use this deterministic identity instead:

```
service = "opencode-rk-v1"
namespace = lower_hex(SHA-256("opencode-rk-v1\0" || canonical_data_dir))[0..32]
provider_tag = lower_hex(SHA-256("opencode-rk-provider\0" || provider_id))[0..32]
user = "p:" || provider_tag || ":d:" || namespace
```

Rules:

- `canonical_data_dir` is obtained by the trusted startup/data-dir owner before
  the store call. The existing daemon `normalized_key` is not sufficient because
  it deliberately does not resolve symlinks.
- Hash input includes a fixed domain separator and version. The path itself is
  never placed in the keyring label, status, error, or logs.
- `provider_id` is the existing validated non-empty UTF-8 identifier, at most
  128 bytes, with no NUL. It is never placed raw in the service/account label.
  The fixed 32-hex-character provider tag plus 32-hex-character data-directory
  namespace keeps the derived user bounded across the supported platforms;
  implementation still checks the actual platform limits before `Entry::new`.
- Baseline account identity is one provider per data directory. A future
  multi-account ID must be added to the domain-separated user key and separately
  tested; it must not reuse provider-only labels.
- Native OS-user scoping remains part of the platform store. Data-directory
  digest prevents collisions between the PLAN.md per-user data directories.
  It does not provide authorization; the capability broker and OS store do that.

Namespace tests must prove equal canonical roots derive equal labels, distinct
roots derive distinct labels for the fixture set, provider separation, no raw
path leakage, NUL rejection, and platform-length rejection. A hash collision
claim is not made from a finite test.

## State, call, and error matrix

| Scenario | Calls | Observable result | Side-effect boundary |
|---|---|---|---|
| First save | validate -> bounded `Entry::new` -> `set_password` | `Present` | No native call on validation failure |
| Load present | validate -> new `Entry` -> `get_password` | secret to authorized caller only | Never log/serialize secret |
| Load missing | `get_password` returns `keyring::Error::NoEntry` | `Missing`, startup `Setup` | No fallback write |
| Overwrite | `set_password` same identity | final value is new value | Do not claim cross-platform atomicity |
| Delete present | `delete_credential` | success, then load is `Missing` | Native operation may be non-atomic; verify final state |
| Delete absent | native `NoEntry` | application success `AlreadyAbsent` | No error detail exposed |
| Locked/unavailable | `NoStorageAccess` or `PlatformFailure` | `BackendUnavailable` / `keyring_unavailable` | Keyring-or-deny; no File0600 fallback |
| Ambiguous | `Ambiguous` | `BackendAmbiguous` fixed code | No selected secret |
| Corrupt text | `BadEncoding` | `CorruptEncoding` fixed code | Raw attached bytes never escape or log |
| Invalid identity | `Invalid`/`TooLong` or app validation | `IdentifierInvalid` | Zero backend calls |
| Oversize/empty secret | app validation | `SecretInvalid` / `SecretTooLarge` | Zero backend calls |
| Unknown non-exhaustive error | wildcard mapping | `BackendUnavailable` | No `Display` or source-chain exposure |

The application error enum may use different internal Rust names, but its wire
and diagnostic codes must be fixed, non-secret, and tested. `StoreError` in the
current planning module lacks all needed runtime states; do not silently claim
it is the runtime implementation. Additive runtime error design requires an
integration proposal and implementation lane.

Overwrite and deletion are final-state guarantees only. Keyring 3.6.3 does not
give APP012 a cross-platform transaction or atomicity guarantee. A failed
overwrite may leave the previous value or an implementation-specific state;
the error is returned and a subsequent status read is required. A failed delete
does not claim the entry is absent.

## Restart proof and test backend contract

The 3.6.3 mock is explicitly `CredentialPersistence::EntryOnly`. Its builder
creates a fresh `MockCredential` for every `Entry::new`; values live in that
entry and do not persist across a newly reconstructed entry. Therefore:

- A retained mock `Entry` can test set/get/delete and one-shot error mapping.
- A store that calls `Entry::new` for each method cannot use this mock to prove
  restart persistence.
- `set_default_credential_builder` is process-global and must be initialized
  before entries, with a mutex/serial test boundary. It is not a durable store.
- The old T01/T05 wording that used mock save/retrieve after reconstructing the
  store is invalid and must not be frozen.

The independent RED must define an application-owned injectable backend seam.
The minimum deterministic fake is:

```
DurableFakeState = Arc<Mutex<HashMap<(service, user), String>>>
DurableFakeBackend::new(shared_state)
StoreInstance::from_backend(backend, canonical_data_dir)
```

`StoreInstance A` saves, is dropped, `StoreInstance B` is reconstructed with
the same shared durable fake state, loads, then is dropped; `StoreInstance C`
deletes; `StoreInstance D` loads and observes `Missing`. The fake must key by
the exact derived service/user pair, not provider alone, and must not be a
retained entry. This proves application lifecycle and namespace behavior, not
native OS durability.

The RED may additionally use the real 3.6.3 mock builder, serially, for retained
entry operation mapping and `MockCredential::set_error`. It must label those
cases as non-restart tests. No local RED may write a real Keychain, Secret
Service, keyutils, or Windows Credential Manager entry.

Native restart proof is a separate verifier/platform lane, not a mock test:

1. Build the exact target with the exact native feature row above.
2. Create a disposable OS account/credential namespace and isolated data dir.
3. Process A creates a fresh store object, saves a canary, exits normally.
4. Process B starts independently, reconstructs the store and `Entry`, loads the
   canary, verifies exact value, overwrites it, then exits.
5. Process C independently loads the rotated value, deletes it, and verifies
   `NoEntry`/missing.
6. Cleanup runs in a bounded finalizer and records success, unavailable backend,
   or failure. It never mutates a developer's ordinary keychain.

The receipt must include target triple, package/lock hash, feature row, OS/backend
availability, fresh process IDs or equivalent independent-process evidence,
operation outcomes, cleanup outcome, and redaction scan. `unavailable` is a
blocker for acceptance, not a pass.

## Blocking, cancellation, and resource contract

Native keyring APIs are synchronous. They must not run on Tokio worker threads.
The implementation shall:

- execute each operation in an owned `spawn_blocking` task;
- gate admission with a bounded semaphore, initial maximum two in-flight native
  operations per store service; no unbounded queue;
- bound request material before admission: provider ID 128 bytes, derived
  labels checked against platform limits, secret 64 KiB, and fixed diagnostic
  buffers with no retained raw backend errors;
- use a caller-visible operation deadline, initially 5 seconds, covering queue
  wait and result wait. This is a deadline, not a claim that Rust can interrupt
  an already-running OS call;
- retain ownership of every started `spawn_blocking` `JoinHandle` until it
  completes. On deadline, return `keyring_timeout` to the caller but keep the
  handle in a bounded draining set; never drop it into a detached task;
- expose an async `shutdown`/drain path owned by the daemon. Shutdown waits for
  all started native calls, releases semaphore permits, and reports a bounded
  shutdown timeout as an unresolved resource failure. No operation is silently
  replayed after timeout because save/delete side effects are ambiguous;
- ensure cancellation before backend start removes the queued request with no
  side effect. Cancellation after backend start returns cancellation to the
  caller only after ownership of the running call is retained for drain;
- serialize operations for the same derived entry, because 3.6.3 warns that
  same-entry concurrent accesses are not ordered reliably on Windows/Linux.

The native keyring crate does not promise a hard interrupt for blocking platform
calls. Any implementation claiming immediate cancellation, a hard 5-second OS
kill, zero retained bytes, or zero retained tasks is incorrect without a
platform-specific proof. Resource tests must observe in-flight count, task
completion/drain, process count, elapsed deadline behavior, and absence of a
detached task. The 8 GiB host budget still applies; no broad Cargo or platform
matrix runs are parallelized.

## Secret lifetime and redaction contract

No zeroization guarantee is made for `SecretString`, `String`, provider auth
types, or ordinary returned values. The repository explicitly records that
zeroization is not implemented. Native `keyring` Windows code uses zeroize for
some temporary copies internally, but that does not transfer a guarantee to the
application or other platforms. A future zeroization design requires an
accepted dependency/API decision and independent memory-lifetime evidence.

Required weaker guarantee:

- Secret input is borrowed or moved only into the bounded operation owner.
- No secret appears in `Debug`, `Display`, serde, logs, traces, telemetry,
  status, errors, provenance, test names, fixture paths, or process arguments.
- Loaded values are dropped at the end of the authorized caller scope; callers
  must not clone or retain them beyond the provider request. This is a lifetime
  rule, not memory erasure proof.
- Raw `keyring::Error` values are never serialized or formatted. In particular,
  `BadEncoding(Vec<u8>)` is mapped without exposing its attached bytes.
- Test canaries are synthetic and redaction-scanned; no real credential is
  allowed in fixtures.

## Platform acceptance matrix

| Target | Required native proof | Honest boundary |
|---|---|---|
| macOS | `apple-native`; fresh process A/B/C Keychain save/load/overwrite/delete and cleanup | Login keychain may prompt or be unavailable; receipt must say so |
| Linux | `linux-native-sync-persistent` plus `crypto-rust`; running Secret Service, unlocked collection, fresh process A/B/C | Headless/no session bus/unlocked collection maps unavailable; keyutils-only is not restart proof |
| Windows GNU | `windows-native`; actual `x86_64-pc-windows-gnu` build and fresh process A/B/C Generic Credential Manager proof | Docs.rs MSVC metadata is not GNU proof; unsigned CI can prove build/backend, not signing |
| CI mock | 3.6.3 mock builder only for retained-entry API/error tests | Never presented as durable or restart evidence |

The platform lane must report target triple, exact keyring version/features,
backend availability, and cleanup. No live developer keychain mutation is
authorized. Missing platform receipt keeps APP012 open.

## Ownership DAG and test boundaries

Ownership is deliberately split:

1. **Contract correction lane**: this worklog only; no product or test edits.
2. **Independent RED author**: new keyring persistence RED and durable fake
   fixture specification. Compiling failure must be run independently, then the
   exact test hash and command manifest are frozen. This lane cannot modify
   product code, Cargo files, or the existing frozen APP012 RED.
3. **Dependency integration lane**: serialized target dependency features,
   lockfile, and module registration. It cannot author or alter frozen tests.
4. **Implementation lane**: backend trait, native keyring adapter, typed mapping,
   namespace, bounded executor, and unit behavior. It cannot freeze or accept
   its own RED.
5. **Runtime wiring lane**: explicit caller ownership for onboarding commit,
   startup credential status, and provider request loading. Shared
   `daemon_client.rs`, `responses.rs`, and any central module registration need
   an integration proposal, not a hidden leaf edit.
6. **Independent platform verifier**: runs frozen RED, regressions, redaction
   scans, resource/cancellation cases, and disposable native A/B/C receipts on
   the exact integrated revision. It issues ACCEPT or REJECT.

The existing `app012_tool_journey_red.rs` remains byte-identical and unrelated
to this keyring RED. No lane may claim APP012 parent completion from keyring unit
GREEN, mock operation GREEN, or module compilation alone.

## Future RED command and fixture manifest

The RED author must provide a deterministic disposable fixture, no network, no
real credentials, no existing database mutation, bounded process cleanup, and
the following minimum scenarios:

| ID | Scenario | Required assertion |
|---|---|---|
| K01 | first save/load with durable fake | same value after new store instance |
| K02 | overwrite | new value visible; old value not returned |
| K03 | missing | typed missing; startup chooses Setup; no fallback write |
| K04 | delete present and absent | present becomes missing; absent is idempotent success |
| K05 | unavailable/locked | fixed `keyring_unavailable`; no File0600/config/SQLite side effect |
| K06 | corrupt encoding | fixed corruption code; raw bytes absent from all output |
| K07 | invalid/oversize inputs | backend call count remains zero |
| K08 | namespace | data-dir/provider separation and path redaction |
| K09 | cancellation/deadline | queue cancellation no side effect; running call drains; no detached task |
| K10 | retained 3.6.3 mock | exact API/error mapping only; explicitly not restart proof |

Expected RED: tests compile against the planned injectable interface and fail
for missing implementation behavior. A compile failure from an absent test
import or a missing fixture is invalid RED. This correction lane has no product
RED and must not invent a hash. The independent RED author freezes its own hash.

Suggested bounded commands for later lanes, not run here:

```sh
timeout 120 rtk cargo test -p opencode-rk-providers --test keyring_persistence_red -- --nocapture
rtk shasum -a 256 crates/providers/tests/keyring_persistence_red.rs
rtk git diff --check
```

Native platform commands must be target-runner-specific and disposable; a local
macOS command cannot stand in for Linux Secret Service or Windows GNU evidence.

## Verifier blocker-resolution table

| Verifier blocker at `135d26e` | Resolution in this correction | Owner/evidence boundary |
|---|---|---|
| Wrong `delete_password` API | Use `Entry::delete_credential()` | Dependency source; RED/API compile check |
| Invented `set_default_mock!` and `MockEntryBuilder` | Use `set_default_credential_builder(mock::default_credential_builder())`; downcast `MockCredential` for `set_error` | Dependency source; serial mock test |
| Mock restart fallacy | Use shared-state durable injectable fake for reconstructed instances; reserve A/B/C native process proof | RED author plus platform verifier |
| Bare dependency selected mock | Exact target feature table; no default features; lockfile receipt | Dependency integrator |
| Linux backend misnamed | Persistent Linux row is `linux-native-sync-persistent` plus `crypto-rust`; document Secret Service/keyutils combo | Dependency integrator plus Linux receipt |
| Windows GNU unsupported by metadata claim | Retain product GNU policy only; require actual GNU build/native receipt; do not cite docs.rs MSVC metadata as proof | Windows platform verifier |
| Fixed service/provider-only namespace collision | Domain-separated SHA-256 of canonical data dir in user label | Implementation lane; namespace tests |
| Missing blocking/cancellation bound | Owned `spawn_blocking`, semaphore 2, 5 s caller deadline, retained drain handles, same-entry serialization, shutdown drain | Implementation/resource verifier |
| Error/API state ambiguity | Typed missing, unavailable, ambiguous, corruption, invalid, timeout; fixed redacted mapping table | Implementation and independent verifier |
| Plaintext fallback leakage | APP012 keyring-or-deny; do not call `StoreBackend::resolve` for this path | Runtime wiring verifier; side-effect test |
| Unsupported zeroization claim | Remove guarantee; enforce redaction and scope-drop only; any zeroization is future separately accepted work | Security verifier |
| Password/binary ambiguity | Baseline UTF-8 `set_password/get_password`; binary API requires separate accepted contract | RED and implementation lanes |
| Combined RED/implementation ownership | Distinct RED, dependency, implementation, runtime wiring, and verifier lanes | Controller ledger |
| Current caller absent | Treat env-only/in-memory onboarding as open wiring gap; require explicit caller integration proposal | Runtime wiring lane; parent journey |

## Validation performed by this correction lane

```text
rtk git grep -n 'keyring\|credential\|api_key\|onboard' -- crates Cargo.lock Cargo.toml
  completed; no product keyring dependency/caller; current auth is env-only

rtk shasum -a 256 crates/server/tests/app012_tool_journey_red.rs
  945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50

rtk git diff --check
  required before commit; no product changes made
```

No Cargo build, native keyring call, real credential access, product RED, or
GREEN was run. Expected state is corrected research artifact only. Frozen
APP-012 hash remains unchanged. Independent RED authoring and fresh verification
remain required. `convergence_gate.py` and `validate_repository.py` are broader
controller gates and may remain blocked by unrelated ledger/backlog findings;
this lane does not alter them.

## Unresolved external blockers

- Controller must approve the additive runtime error/interface and target
  feature integration proposal.
- Independent RED author must freeze compiling failure against the durable fake;
  no keyring RED hash exists yet.
- Native macOS Keychain, Linux Secret Service, and Windows GNU A/B/C receipts do
  not exist in this research lane. Their absence blocks acceptance.
- The implementation must prove its owned executor/drain design on the actual
  runtime. This document does not claim cancellation or memory erasure proof.
- Runtime caller wiring from onboarding through startup and provider requests is
  absent in the current source and remains a parent repair boundary.
