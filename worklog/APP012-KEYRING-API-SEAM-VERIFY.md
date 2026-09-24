# APP012-KEYRING-API-SEAM-VERIFY

## Verdict

**REVISE.** Candidate contract `6dcfe198e096721169efb2d96f0d763b35c8daff` is not exact enough to authorize implementation, dependency integration, or behavioral RED.

**No implementation authorization.** No product, test, dependency, Cargo, native-keyring, or runtime-wiring change is authorized by this verification. Verification task itself is complete.

Task: `APP012-KEYRING-API-SEAM-VERIFY`  
Session: `ses_f2b72911cffed5MyTRY0HbGHlI`  
Branch: `verify/APP012-KEYRING-API-SEAM`  
Owned path: this worklog and this task's row in `tasks/completion/claims.json` only.

## Evidence boundary

- Candidate contract: `worklog/APP012-KEYRING-API-SEAM-CONTRACT.md` at `6dcfe19`.
- Binding TDD order: `docs/TDD.md:21-48`, especially `:32-38` and `:45-48`.
- Security/resource rules: `docs/SECURITY.md:19-30`, `:49-59`, `:74-82`.
- Existing product remains unwired:
  - `crates/providers/src/lib.rs:4-59` has no keyring-persistence module.
  - `crates/providers/Cargo.toml:9-20` has no `keyring` dependency.
  - `crates/cli/src/daemon_client.rs:775-793` checks environment variables only.
  - `crates/providers/src/config.rs:147-150` and `responses.rs:516-523` read environment credentials only.
  - `crates/cli/src/onboarding.rs:348-405`, `:513-570` retain in-memory account state.
  - `crates/providers/src/auth_store.rs:1-8`, `:72-79`, `:394-430` is planning-only and permits a separate `File0600` resolution; APP-012 must not use that fallback.
- Exact `keyring` 3.6.3 source inspected at `$HOME/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/keyring-3.6.3`:
  - `src/lib.rs:322-512`: `Entry::new`, `set_password`, `get_password`, `delete_credential`, `get_credential`.
  - `src/mock.rs:16-35`, `:185-235`: builder, `MockCredential::set_error`, `EntryOnly` persistence.
  - `src/error.rs:15-56`, `:61-100`: non-exhaustive errors and payload-bearing `BadEncoding(Vec<u8>)`.
  - `src/credential.rs:99-104`, `:137-173`: native deletion is non-idempotent; mock persistence is `EntryOnly`.
  - `src/keyutils_persistent.rs:67-129`: Linux backend logs raw `keyring::Error` text at debug level.
- Tokio source inspected at `$HOME/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tokio-1.53.1/src/task/blocking.rs:82-120`, `:220-225`: `spawn_blocking` closures are `'static`; started blocking tasks cannot be aborted and runtime shutdown may wait indefinitely.
- Frozen unrelated APP-012 RED remains unchanged. SHA-256:
  `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.

## Nine blocker/correction groups

### 1. RED lifecycle order — REVISE

**Blocker.** The contract orders full API seam implementation first (`:567-588`), then compile/API verification, then RED authoring (`:580-584`). `docs/TDD.md:32-48` requires tests to compile and fail before behavior implementation. A compile-failing test is explicitly invalid.

**Correction.** Resolve bootstrap order in an integration/authority decision. Either authorize a narrowly scoped, real compile seam that is not claimed as behavior implementation, or explicitly change the lifecycle decision before RED. Do not call full validation/executor/orchestration implementation “compile verification.” Freeze a behavioral RED only after the exact public API can compile.

### 2. K10 adapter testability — REVISE

**Blocker.** `KeyringBackend` constructs a fresh `keyring::Entry` inside every operation (contract `:458-475`). The 3.6.3 mock stores data only in that entry; `MockCredential::set_error` requires downcasting a retained entry. The proposed K10 table (`:519-521`, `:560`) cannot drive an error through `KeyringBackend` as written. A fresh mock entry is also not restart evidence.

**Correction.** Split K10 into:
- direct retained-entry 3.6.3 mock API/error test, explicitly non-durable; and
- adapter mapping test using an approved `EntryFactory`/entry seam, or a separately owned adapter unit test with access to the private entry constructor.

The approved seam must preserve the production boundary: no default mock backend, no file/environment fallback, no native store access in ordinary RED.

### 3. K06 corrupt-encoding mapping — REVISE

**Blocker.** `RawBackendError` is payload-free, so the shared fake cannot inject `keyring::Error::BadEncoding(Vec<u8>)`. It can test only application handling of a redacted `CorruptEncoding`; it cannot prove the 3.6.3 adapter mapping or raw-byte disposal.

**Correction.** Split K06 into an app-level redaction test and a direct adapter mapping test. Construct a controlled `BadEncoding` in the adapter seam, map without formatting/retaining, and assert raw bytes, OS detail, and source text are absent from every application-visible surface. Keep the raw test value synthetic and disposable.

### 4. Secret ownership versus `spawn_blocking` — REVISE

**Blocker.** The proposed `save(&self, provider_id: &str, secret: &str)` (`:358-362`) cannot directly satisfy Tokio's `'static` `spawn_blocking` closure requirement. A timeout/cancellation-safe implementation must move an owned secret value or make an explicitly bounded owned copy. The claim that `save` “never clones” and retains the borrow only for the call (`:256-260`) is therefore not implementable as stated.

**Correction.** Choose one explicit contract:
- accept an owned non-clone secret type and move it into the admitted task; or
- accept a borrowed input, create one documented bounded owned copy at admission, retain it only in the owner-held task/drain state, then drop it.

State the retained byte bound, cancellation behavior, and no-zeroization ceiling. Do not promise memory erasure.

### 5. Executor/lifecycle/resource semantics — REVISE

**Blocker.** `StoreLimits` has public fields (`:317-328`), allowing invalid literals. `StoreError` has no invalid-limit variant. Shutdown deadline, shutdown idempotence, behavior after shutdown, join errors, task panics, and atomic admission ordering are unspecified. `in_flight` and `draining` may overlap, but their relationship is not defined. The service also lacks a precise permit/drain state machine.

**Correction.** Make limits private and validated. Add a typed invalid-limit result or remove caller-configurable limits. Define:
- fixed `MAX_IN_FLIGHT = 2`;
- bounded pending count, checked before waiting;
- exact meaning and overlap/inclusion of `in_flight`, `pending`, and `draining`;
- timeout behavior before admission versus after native start;
- owner-held `JoinHandle` transfer and no detached task;
- fixed shutdown grace bound;
- idempotent/concurrent shutdown behavior;
- post-shutdown operation result;
- fixed redacted handling of `JoinError`, panic, and cancellation.

Tests must observe actual task completion/drain, not only counters.

### 6. Service ownership and canonical namespace — REVISE

**Blocker.** `KeyringStore::from_backend` hides service ownership (`:353-356`), while `KeyringService::shutdown(&self)` has no daemon lifecycle contract. `derive_identity` accepts a `&Path` and relies on an informal “trusted startup owner” precondition (`:278-300`). The path has no stated byte bound. Hash-derived labels avoid raw path/provider text but do not prevent dictionary guessing.

**Correction.** Define an explicit service owner and view lifetime. Add a validated canonical-data-directory capability/newtype or an equivalent construction rule that rejects empty, relative, non-UTF-8, over-limit, and ambiguous paths before identity derivation. State a maximum path byte budget. Retain only derived labels. Describe privacy as “no raw path/provider labels,” not resistance to enumeration of known paths/providers.

### 7. Production orchestration/caller seam — REVISE

**Blocker.** The contract names future wiring (`:562-565`, `:589-593`) but supplies no production orchestration contract. Current startup, onboarding, and provider request paths remain environment/in-memory only. `OpenAiResponsesClient` stores `api_key: String` (`responses.rs:506-550`), so the proposed loaded-secret scope is not currently enforced.

**Correction.** Specify a real integration boundary before authorization:
- daemon owns one `KeyringService`;
- onboarding save occurs only after credential validation and before account commit;
- startup presence calls `exists` with keyring-or-deny behavior;
- provider requests load through an authorized bounded scope;
- environment compatibility remains a separately labeled source and never satisfies APP-012 persistence;
- broker authorization, target-specific `KeyringBackend` selection, and unsupported-target failure are explicit;
- no `StoreBackend::resolve`, File0600, config, SQLite, or implicit environment fallback.

A fake-only seam cannot close this gap.

### 8. Diagnostic/logging boundary — REVISE

**Blocker.** The contract promises redacted application diagnostics (`:163-161`, `:442-446`), but Linux `keyring` 3.6.3 itself emits raw error text through `log::debug!` (`keyutils_persistent.rs:67-129`). The absolute phrase “no OS detail in logs” is too broad unless dependency logging is controlled and tested.

**Correction.** Separate application-owned guarantees from dependency behavior. State that application errors, receipts, status, and telemetry never format or retain `keyring::Error`; specify logger filtering/target configuration and a redaction test if the stronger no-OS-log-text claim is required. Do not claim the dependency cannot log raw details.

### 9. Loaded-secret and zeroization ceiling — REVISE

**Blocker.** `LoadedSecret::with_exposed` exposes `&str` to a callback that can clone or retain the value. The contract correctly disclaims zeroization, but its wording can be read as type-enforced non-retention. The repository explicitly records no zeroization at `crates/security/src/credentials.rs:91-93`.

**Correction.** State the enforceable rule: callers must use the value only in the authorized provider-request scope, must not clone/retain it, and must not log/serialize it. Treat `LoadedSecret` as a redaction/lifetime discipline, not memory erasure. If stronger guarantees are required, open a separate dependency/API decision and memory-lifetime proof.

## Corrected future RED import split

### K01-K09: dependency-free application seam

The K01-K09 RED may import only the admitted public seam, after its compile bootstrap is resolved:

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

Use an application-owned shared durable fake keyed by exact derived `(service, account)` labels. K01-K09 may prove orchestration, outcomes, validation, namespace separation, redaction, admission, cancellation, and drain semantics. They do not prove OS persistence or production caller wiring.

K01-K09 assertions remain:
- K01 save/drop/reconstruct/load with shared backend state;
- K02 overwrite final state;
- K03 typed missing and no fallback write;
- K04 present/absent delete outcomes;
- K05 fixed unavailable code and no fallback;
- K06 app-level corrupt redaction (adapter portion belongs to K10);
- K07 invalid/oversize input with zero backend calls;
- K08 canonical namespace separation and raw-label absence;
- K09 bounded deadline, cancellation, drain, and shutdown observation.

### K10: separate dependency/native-adapter RED

K10 must be a separate test ownership/dependency boundary, after the exact `keyring = "=3.6.3"` target-feature integration. Its imports may include:

```rust
use keyring::{
    mock,
    set_default_credential_builder,
    Entry,
    Error,
};
use keyring::mock::MockCredential;
```

Retain the `Entry` returned by the mock builder, downcast with `get_credential()`, inject one-shot `MockCredential::set_error`, and serialize process-global builder setup. This proves exact 3.6.3 API/error mapping only; it is not restart proof. Adapter mapping requires the approved entry-factory/adapter seam described in blocker 2. Native A/B/C receipts remain separate and required for persistence acceptance.

## Required authorization boundary

Until all nine groups are corrected and independently re-verified:

- **Do not implement** `keyring_persistence` production behavior.
- **Do not add** `keyring`, target features, or lockfile entries.
- **Do not author or freeze** a behavioral RED against invented or incomplete symbols.
- **Do not wire** onboarding, startup, provider requests, or native backend selection.
- **Do not claim** native persistence, restart durability, zeroization, atomic overwrite/delete, or broad log suppression.

## Validation and landing evidence

Allowed read-only checks for this verification lane:

```text
rtk shasum -a 256 crates/server/tests/app012_tool_journey_red.rs
945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50

rtk git diff --check
PASS

rtk git grep -n 'keyring\|credential\|api_key\|onboard' -- crates Cargo.lock Cargo.toml
Completed; current product remains dependency-free and environment/in-memory only for this path.

No Cargo command, product test, native keyring call, credential access, test edit, dependency edit, or runtime wiring performed.
```

Verification status is complete with contract verdict **REVISE**. Landing is limited to this worklog and the owned completion-ledger row.
