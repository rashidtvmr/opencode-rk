# APP012-KEYRING-PERSISTENCE-RED

## Claim and result

- Task/session: `APP012-KEYRING-PERSISTENCE-RED` / `ses_f2b9ea3b9ffeze4hG5032Y6Tk8`.
- Branch/base: `red/APP012-KEYRING-PERSISTENCE` at `1cb703996b6b1180bce7cb511acf439187755bd8`.
- Owned paths: this worklog; this task's row in `tasks/completion/claims.json`. The leased test path remains unauthored.
- Result: **BLOCKED before RED authoring**. No honest compiling behavioral RED can exist on this base.
- Prohibited actions observed: no test, fake, dependency, module, API, or product-source edit; no Cargo/check/test/freeze; no native keyring access.

## Exact source evidence

| Evidence | Current behavior / boundary |
|---|---|
| `worklog/APP012-KEYRING-CONTRACT-CORRECTION.md:76-115`, `145-164`, `202-229` | Accepted durable lifecycle, UTF-8 password representation, typed missing/unavailable/ambiguous/corrupt/invalid/oversize outcomes, keyring-or-deny, final-state overwrite/delete. |
| `worklog/APP012-KEYRING-CONTRACT-CORRECTION.md:166-200` | Accepted versioned, domain-separated canonical-data-directory/provider namespace; no raw path/provider labels. |
| `worklog/APP012-KEYRING-CONTRACT-CORRECTION.md:231-265` | keyring 3.6.3 mock is `EntryOnly`; accepted durable fake requires shared state keyed by derived `(service,user)` across independently reconstructed stores. |
| `worklog/APP012-KEYRING-CONTRACT-CORRECTION.md:318-338` | Secret lifetime/redaction ceiling; no zeroization claim; no secret in debug, display, serde, logs, status, or errors. |
| `worklog/APP012-KEYRING-CONTRACT-CORRECTION.md:379-401` | K01-K10 manifest; compile-fail, missing import, or missing fixture is invalid RED. |
| `worklog/APP012-KEYRING-CONTRACT-CORRECTION-VERIFY.md:14-16` | Separate behavioral RED authorized; not implementation or APP-012 acceptance. |
| `worklog/APP012-KEYRING-CONTRACT-CORRECTION-VERIFY.md:93-103` | Shared-state durable fake accepted; entry-only mock explicitly not restart evidence. |
| `worklog/APP012-KEYRING-CONTRACT-CORRECTION-VERIFY.md:177-194` | RED must compile and fail behaviorally; exact K01-K10 boundary; no test hash yet. |
| `crates/providers/src/auth_store.rs:1-7` | Planning boundary opens no keyring, performs no keyring/file/env I/O, retains no secret bytes. |
| `crates/providers/src/auth_store.rs:92-124` | `StoreError` has only path/provider/size/unavailable codes; cannot represent runtime missing, ambiguous, or corruption outcomes. |
| `crates/providers/src/auth_store.rs:268-276` | Provider validation exists. |
| `crates/providers/src/auth_store.rs:347-382` | Store planning and sealed-secret size validation exist; no runtime operation. |
| `crates/providers/src/auth_store.rs:394-430` | Status takes a caller-supplied availability boolean; probes no backend. `resolve` may select `File0600`; APP012 execution must not use that fallback. |
| `crates/providers/src/auth_profile.rs:15-49`, `128-146`, `221-245` | Keyring provenance is secret-free metadata only. |
| `crates/providers/src/lib.rs:4-59` | No credential-persistence module is registered. |
| `crates/providers/Cargo.toml:9-20` | No keyring dependency. |
| `docs/TDD.md:43-48`, `58-63` | RED must compile and fail for missing behavior; compile failures, missing imports, and unfrozen tests are invalid. |
| `docs/SECURITY.md:24-35`, `74-82` | No plaintext secret fallback or secret logging; preserve fail-closed behavior. |

## Why no honest compiling RED exists

A compiling integration test must import an existing public production seam. This base exposes no persistence backend trait, backend error model, store constructor, durable state container, identity derivation function, or `save`/`load`/`delete`/`exists` methods. Accepted worklogs describe required future behavior but contain no admitted, compilable Rust signatures.

Options rejected:

1. Import invented future symbols: test fails at import/type checking; forbidden compile-fail RED.
2. Put a fake store entirely in the test: exercises test code, not production; fabricated success.
3. Reuse `auth_store` planning APIs: cannot save/load/delete or reconstruct persisted state; would test already-existing validation, not missing behavior.
4. Use keyring 3.6.3's entry-only mock as restart proof: false durability claim explicitly rejected.
5. Require a native OS keyring: prohibited in this RED and non-deterministic.

Therefore `crates/providers/tests/keyring_persistence_red.rs` was not authored. No RED hash exists. No test claims acceptance.

## K01-K10 scenario matrix

| ID | Required behavioral RED after an admitted seam exists |
|---|---|
| K01 | Save via store A; drop A; reconstruct B against the same injected durable backend state; load exact canary. State keyed by derived service/account, never by retained entry or provider alone. |
| K02 | Save old value; reconstruct; overwrite; reconstruct; assert new value exact and old value absent. Assert final state only, never cross-platform atomicity. |
| K03 | Load a never-saved reconstructed store; assert typed missing, distinct from unavailable/corrupt. Assert no fallback config, SQLite, env, or `File0600` write. |
| K04 | Delete present credential; reconstruct; assert missing. Delete missing credential; assert idempotent application success. |
| K05 | Inject locked/unavailable backend error. Assert fixed unavailable code. Assert no plaintext/config/SQLite/env fallback and no raw platform error detail. |
| K06 | Inject corrupt/non-UTF-8 backend bytes. Assert fixed corruption code. Assert raw bytes absent from debug, display, serde, status, receipt, and error source chain. |
| K07 | Reject empty/oversize secret plus invalid/oversize/NUL provider before backend invocation. Assert backend call count zero and prior durable state unchanged. |
| K08 | Equal canonical roots derive equal identity; distinct roots/providers derive distinct fixture identities. Assert bounded labels; no raw root/provider/NUL leakage. |
| K09 | Assert secret canary absent from store/backend debug, display, serde, status, receipt, and fixed errors. Assert no OS detail/source-chain leakage. |
| K10 | After dependency integration, retained keyring 3.6.3 mock may prove same-entry operation/error mapping only. Label it non-durable; never use it as restart evidence. |

## Required admitted public seam

A serialized integration/contract lane must first land a compiling public seam; this RED lane must not invent or implement it. Minimum required capabilities:

1. A public injected backend contract whose state is externally owned/shared and survives dropped store instances. It must support entry creation/write/read/delete plus observable call/error injection without OS keyring access.
2. Public backend-raw and application-facing typed results sufficient for: missing, unavailable/locked, ambiguous, corrupt encoding, invalid identity, oversize secret, unknown backend failure. Debug/display/serde must expose only fixed redacted codes.
3. A public deterministic identity value/derivation for the accepted service, canonical data-directory namespace, provider tag, and bounded platform-label validation. Raw path/provider must not appear in labels.
4. A public store constructor accepting injected backend plus canonical data directory; independently constructing equivalent stores must not rely on a retained `Entry`.
5. Public `save`, `load`, `delete`, and `exists` operations. Save/load must use the accepted UTF-8 representation and 64 KiB pre-backend bound. Delete must map missing to application success.
6. Secret-safe status/receipt/debug/serde projections and an observation seam sufficient to prove zero backend calls, no fallback side effects, and no secret/error-detail leakage.
7. Module registration sufficient for an external integration test to compile. This seam alone is not proof of implementation correctness, native persistence, runtime wiring, or APP-012 acceptance.

K01-K10 must then be authored against those exact symbols, compiled, and observed failing at a behavioral assertion. Only then may the controller freeze the test hash.

## Pending command after unblock

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120 cargo test -p opencode-rk-providers --test keyring_persistence_red -- --nocapture --test-threads=1
```

Not run on this base. Expected after seam admission: compile success, K01 execution, behavioral assertion failure. Import/type/signature failure remains invalid.

## Commands and evidence

- `rtk git rev-parse HEAD`: `1cb703996b6b1180bce7cb511acf439187755bd8`.
- Read-only contract/source/API/history inspection: completed.
- `rtk git log --all -- crates/providers/src/keyring_persistence.rs crates/providers/tests/keyring_persistence_red.rs`: no implementation or test history.
- Repowise overview attempted: repository index absent; direct source used.
- Cargo/check/test/freeze: **not run**, per explicit resource rule and blocker.
- Native keyring/credential access: **none**.
- Final commit, push, and remote verification: recorded in handoff; not yet known while this worklog content was staged for commit.

## Status

`blocked`. Exact blocker: admitted compilable public credential-persistence seam absent. Honest RED test, behavioral failure, and freeze impossible under the owned-path restriction.
