# APP012 keyring contract correction verification

## Task and verdict

- Task: `APP012-KEYRING-CONTRACT-CORRECTION-VERIFY`
- Type: verification
- Role: credential-persistence contract verifier
- Session: `ses_f2c0ac049ffeGGVeohLhuPAU43`
- Branch: `plan/APP012-KEYRING-CONTRACT-CORRECTION`
- Candidate: `41f37f0a312dfaeff041b67e754aaf98ba3cf107`
- Owned path: this worklog and this task's ledger row only
- Route: `9router-xk-gpt56-luna`

**ACCEPT** for authorizing a separate compiling behavioral RED. This is not
implementation, dependency acceptance, native platform proof, runtime wiring,
integration, or APP-012 acceptance. APP-012 remains open.

## Authority and current-product boundary

The corrected contract is the candidate artifact at
`worklog/APP012-KEYRING-CONTRACT-CORRECTION.md`. The current tree still has no
keyring dependency or product keyring caller:

- `Cargo.toml:23` sets workspace MSRV to Rust `1.85`.
- `crates/providers/Cargo.toml:9-17` has no keyring dependency.
- `crates/providers/src/lib.rs:4-59` has no keyring persistence module.
- `crates/cli/src/daemon_client.rs:775-791` is env-only
  (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GOOGLE_API_KEY`, `GEMINI_API_KEY`).
- `crates/providers/src/config.rs:147-150` reads env only;
  `crates/providers/src/responses.rs:516-522` uses that env lookup.
- `crates/cli/src/onboarding.rs:155-215` provides bounded, redacted
  in-memory `SecretString`; it does not persist to a keyring.
- `crates/security/src/credentials.rs:34-93` is in-memory and explicitly
  records that zeroization is not implemented.

The frozen unrelated APP-012 tool RED remains byte-identical. No keyring RED,
dependency, source, runtime, or native receipt exists in this candidate.

## Blocker-by-blocker verification

### Exact keyring 3.6.3 API, errors, features

Verified the local exact package source at
`$HOME/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/keyring-3.6.3`:

- `Cargo.toml:12-16,41-89`: version `3.6.3`, MSRV `1.75`, no default feature,
  features `apple-native`, `windows-native`, `linux-native`,
  `linux-native-sync-persistent`, `sync-secret-service`, `crypto-rust`.
- `src/lib.rs:311-312,350-472`: `Entry::new`, `set_password`, `get_password`,
  `set_secret`, `get_secret`, `delete_credential`, `get_credential`, and
  target-dependent native/mock selection.
- `src/mock.rs:10-35,185-235`: supported mock setup is
  `set_default_credential_builder(mock::default_credential_builder())`;
  error injection is `MockCredential::set_error`; no
  `set_default_mock!` or `MockEntryBuilder`.
- `src/error.rs:24-56,61-100`: `NoEntry`, `NoStorageAccess`,
  `PlatformFailure`, `BadEncoding(Vec<u8>)`, `TooLong`, `Invalid`, and
  `Ambiguous`; the enum is non-exhaustive.
- `src/credential.rs:99-104,137-173`: deletion API is
  `delete_credential`; native persistence is described by
  `CredentialPersistence::UntilDelete`.
- `src/mock.rs:185-230`: mock persistence is `EntryOnly`, not durable.

Correction worklog lines 117-164 and 414-423 use these exact spellings. The
forbidden `4.2.0` is absent from the candidate contract and remains forbidden
because its declared MSRV exceeds the workspace MSRV.

Feature rows are exact and target-scoped: macOS `apple-native`; Linux
`linux-native-sync-persistent` plus `crypto-rust`; Windows `windows-native`;
unsupported targets fail explicitly rather than silently selecting mock. The
package metadata's docs.rs Windows MSVC target is not treated as GNU proof.

### Password/binary representation

Baseline is intentionally UTF-8 `set_password`/`get_password`; arbitrary bytes
require a separate accepted `set_secret`/`get_secret` contract. This matches
`src/lib.rs:165-172` and `src/error.rs:39-42,98-100`, including
`BadEncoding(Vec<u8>)`. The correction requires raw bad-encoding bytes to be
discarded before application diagnostics. Empty and over-`64 KiB` inputs are
application validation failures before backend invocation. No binary round-trip
claim is made.

### Namespace, collision, privacy

The corrected derivation is domain/version-separated SHA-256 over the trusted
canonical data directory and provider ID, with bounded lower-hex tags. Raw
paths and provider IDs are not labels. Equal canonical roots and distinct
fixture roots are testable; finite fixtures do not make a collision-proof
claim. Canonicalization is deliberately owned by trusted startup/data-dir
code, not the existing lexical-only daemon `normalized_key`. This resolves the
prior fixed service/provider-only collision.

### Durable fake and restart boundary

The corrected contract explicitly rejects `EntryOnly` as restart evidence
(`lines 231-265`). The proposed application-owned fake is shared
`Arc<Mutex<HashMap<(service, user), String>>>`, keyed by the exact derived
identity. Required sequence: store A saves, store B reconstructs and loads,
store C deletes, store D observes missing. This proves application lifecycle
and namespace behavior without misrepresenting the keyring mock as durable.
The real 3.6.3 mock is retained only for same-entry operation/error mapping,
with process-global setup serialized.

### Native A/B/C receipts

No native receipt is present or claimed. The correction explicitly separates
platform verification and requires disposable independent processes A/B/C:

- macOS Keychain with `apple-native`;
- Linux Secret Service-backed persistent combo with
  `linux-native-sync-persistent` plus `crypto-rust`;
- Windows GNU Credential Manager with `windows-native` and an actual
  `x86_64-pc-windows-gnu` build.

Receipts must contain target triple, exact package/features and lock hash,
backend availability, independent-process evidence, outcomes, cleanup, and
redaction scan. Missing/unavailable native receipt keeps APP-012 open. No live
developer keychain mutation is authorized. This is honest and does not block
authoring the separate deterministic RED.

### Blocking, ownership, concurrency, timeout

The correction states owned `spawn_blocking`, maximum two in-flight native
operations per store service, caller deadline of five seconds covering queue
and result wait, retained `JoinHandle`s in a bounded drain set, shutdown/drain,
pre-start cancellation, post-start cancellation only after ownership retention,
no replay of ambiguous save/delete side effects, and same-entry serialization
(`lines 283-316`). It explicitly says timeout does not interrupt an already
running OS call. This fixes the prior detached-task/immediate-cancellation
overclaim.

The RED author must freeze the concrete pending-admission cap and drain
observation interface before implementation. The contract's non-negotiable
"no unbounded queue" requirement is accepted as a RED/implementation
constraint; a semaphore alone must not be used as proof of a bounded waiter
queue.

### Sizes, cleanup, lifetime

The correction bounds provider ID at 128 bytes, secret input at 64 KiB,
derived labels/platform limits, diagnostic buffers, in-flight operations, and
draining ownership. Native A/B/C cleanup is bounded and receipt-backed. No
claim is made that timeout cancels OS work, that deletion is cross-platform
atomic, or that every started task is immediately gone.

### Redaction and memory honesty

The correction removes the prior false zeroization claim. It records that
`SecretString`, ordinary `String`, provider auth types, and returned values do
not receive an application zeroization guarantee. The enforceable baseline is
scope-limited secret ownership, no secret in Debug/Display/serde/logs/traces/
status/provenance/process arguments, raw keyring errors never formatted, and
drop at the authorized caller boundary. This agrees with
`crates/security/src/credentials.rs:93` and the existing redacted
`SecretString` at `crates/cli/src/onboarding.rs:155-199`.

### Missing, locked, unavailable, overwrite, delete, corrupt, encoding

The corrected matrix (`lines 202-229`) gives explicit typed behavior:

- `NoEntry`: typed missing, no fallback write, startup Setup.
- `NoStorageAccess`/`PlatformFailure`: fixed unavailable code, keyring-or-deny.
- overwrite: same identity, final-state proof only, no atomicity claim.
- present delete: success then missing; absent `NoEntry` maps to idempotent app
  success while native deletion remains non-idempotent.
- `BadEncoding`: fixed corruption code; attached bytes never escape.
- `Invalid`/`TooLong`: fixed invalid-identity/platform-limit mapping.
- empty/over-size input: rejection before backend call.
- `Ambiguous` and unknown non-exhaustive errors: fixed safe mapping, no source
  or OS detail.

APP012 explicitly does not call `StoreBackend::resolve` for plaintext
fallback. No env/config/SQLite fallback is allowed to satisfy save/restart.
Existing env-only compatibility remains visibly separate and cannot satisfy
the persistence proof.

### Ownership and executable RED boundary

The correction separates: independent RED author; serialized dependency/
lockfile/feature integrator; implementation adapter and bounded executor;
runtime wiring for onboarding/startup/provider calls; and independent verifier.
It preserves the existing APP-012 RED as unrelated and immutable.

The future manifest is concrete without fabricating a hash: K01 durable fake
save/load; K02 overwrite; K03 missing/Setup/no fallback; K04 present/absent
delete; K05 unavailable/no plaintext side effect; K06 corruption/redaction;
K07 invalid/oversize zero backend calls; K08 namespace/privacy; K09
cancellation/deadline/drain/no detached task; K10 retained 3.6.3 mock API/error
mapping only. The proposed test path and command are explicit:
`crates/providers/tests/keyring_persistence_red.rs` and
`timeout 120 rtk cargo test -p opencode-rk-providers --test
keyring_persistence_red -- --nocapture`. Hash is intentionally absent until
the independent RED compiles and fails. No test-source guessing or fabricated
RED result is used.

## Commands and results

Required bounded validation:

```text
rtk git grep -n 'keyring\|credential\|api_key\|onboard' -- crates Cargo.lock Cargo.toml
  PASS; current tree remains dependency-free and env-only for keyring behavior.

rtk shasum -a 256 crates/server/tests/app012_tool_journey_red.rs
  945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50

rtk git diff --check
  PASS

rtk python3 tools/convergence_gate.py
  FAIL; pre-existing controller/backlog convergence findings, including 92
  ledger findings. No product or policy changes made.

rtk python3 tools/validate_repository.py
  FAIL; pre-existing backlog exhaustion failure, 51 errors. Protection policy
  fixture self-test passed. No product or policy changes made.
```

No product test was run, as required for this verification lane. No Cargo
build, native keyring call, real credential access, runtime wiring, or RED
authoring was performed. Resource-heavy validation was not started. The
attempted `rtk free -h` equivalent was unavailable on this macOS shell; no
resource claim is made.

## Unresolved acceptance gaps

These do not block separate RED authorization, but block later APP-012
acceptance:

1. No dependency integration or lockfile feature receipt.
2. No keyring persistence implementation or runtime caller wiring.
3. No native macOS/Linux/Windows A/B/C receipt.
4. No independent RED source, compile-failing result, frozen keyring RED hash,
   or verifier result.
5. RED author must specify a finite pending-admission cap and observable drain
   counters before freeze; semaphore capacity alone is not queue capacity.

The frozen existing APP-012 hash remains unchanged. APP-012 remains open.
