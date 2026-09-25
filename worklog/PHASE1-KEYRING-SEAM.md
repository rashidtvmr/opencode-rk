# PHASE1-KEYRING-SEAM — discovery: no implementation-independent black-box caller seam

## Claim and boundary
- Task: `PHASE1-KEYRING-SEAM` (leased task; task card absent in `tasks/`, authority = synthesis payload + blocker `4814357` + current code).
- Session: `ses_f28c48535ffeMcwhMQUuVMAeqb` (claimed via `tools/completion_claims.py`).
- Branch: `plan/PHASE1-KEYRING-SEAM`, base `7262682e31c1f473912c9483804c9430eb4ee4c5` (verified pwd/branch/HEAD).
- Owned files: exactly `worklog/PHASE1-KEYRING-SEAM.md` + own ledger row.
- Method: text/static discovery only. No Cargo, dependency, product, test, keychain, credential, or secret access. No keyring probing, no env-secret reads, no user-DB touch.
- Status set by this lane: `blocked` (verdict below). No durability claim.

## Source evidence (exact current-code citations, base 7262682)
- Prior blocker `4814357` (`research(APP012): blackbox RED seam audit BLOCKED - no admitted persistence seam at 4dc8811`): full audit at `git show 4814357:worklog/APP012-KEYRING-BLACKBOX-RED-SEAM.md`; rejected Options 1–4 (no `oc2 setup` executable seam, in-memory onboarding only, no daemon save/presence flow, planning-only auth_store). Diff to current HEAD for all six probed files (`auth_store.rs`, `account_setup.rs`, `auth_profile.rs`, `lib.rs`, providers `Cargo.toml`, `onboarding.rs`) is empty — no drift since that audit.
- `crates/providers/src/auth_store.rs:1-7` — planning boundary: opens no keyring, no file/env/net I/O, retains no secret bytes.
- `crates/providers/src/auth_store.rs:62-90` — `StoreBackend::Keyring/File0600`, `resolve(bool)` is caller-supplied planning; File0600 fallback is the forbidden APP012 path.
- `crates/providers/src/auth_store.rs:92-124` — `StoreError` has only `PathNotAllowed/EmptyProvider/ProviderTooLong/TooLarge/KeyringUnavailable`; cannot represent runtime missing/corrupt/ambiguous outcomes.
- `crates/providers/src/auth_store.rs:347-431` — `plan_store`, `plan_store_with_secret`, `plan_refresh_persist`, `status_of`, `status_with_keyring` (caller boolean, probes no backend). No `save`/`load`/`delete`/`exists`, no backend trait, no reconstruction.
- `crates/providers/src/auth_store.rs:30-60` — `SealedSecret` is borrow-validation only; no owner/drain.
- `crates/providers/src/lib.rs:9` — only `pub mod auth_store`; no `keyring_persistence`/`keyring_adapter` module (verified by grep: zero hits for `keyring_persistence|KeyringService|BackendHandle|SecretInput|StoreLimits|CanonicalDataDir` in `crates/`).
- `crates/providers/Cargo.toml` — no `keyring` dependency; workspace `Cargo.lock`/`Cargo.toml` grep for `keyring` returns nothing.
- `crates/providers/src/auth_profile.rs:27-29` — `Keyring` is a provenance label only, not a backend.
- `crates/providers/src/account_setup.rs:1-8,332-454` — bounded in-memory setup store; `SecureStoreMarker` proves intent, persists nothing; caller owns persistence/transport.
- `crates/cli/src/onboarding.rs:348-398` — `AccountStore` trait + `MemoryAccountStore` documented test/offline fixture; production swap is future work, not present.
- `crates/cli/src/onboarding.rs:517-540` — `submit_credential` validates shape, persists nothing.
- Repo-wide: all `OnboardingSession|MemoryAccountStore|run_interactive` hits live inside `onboarding.rs` only; `crates/cli/src/main.rs:49` declares `mod onboarding;` with no setup/credential subcommand invoking it. No `oc2 setup` executable seam.
- `crates/cli/src/daemon_client.rs:765-791` — `creds_configured` ignores data dir, checks env keys only (forbidden fallback).

## Verdict: NONE — no implementation-independent black-box caller seam exists
Every candidate caller on this base fails the compiling-behavioral-RED bar (`docs/TDD.md:43-48`: RED must compile and fail for the missing behavior; import/type failure or never-executed tests are invalid):
1. `oc2` setup subprocess — no such subcommand; would need secret-via-argv/env (violates `docs/SECURITY.md:26-27`) + nonexistent daemon-lifetime owner.
2. Onboarding service + production store — only `MemoryAccountStore` fixture exists; `submit_credential` never persists; canary cannot survive drop-and-reconstruct; testing the fixture against itself is fabricated success.
3. Daemon save/presence flow — no `save`/`load`/`delete`/`exists` symbols; `creds_configured` is env-only. Nothing to import.
4. `auth_store`/`config` public methods — planning/env only; a compiling test would re-prove existing validation (already GREEN), not missing durability.

## Minimal real caller + RED ownership needed (no stubs)
Unblock requires controller/integration authority to admit ONE bootstrap checkpoint (scope, owner, freeze authority) — the `55174dc` correction route cited in `4814357` — then, strictly in order:
1. **RED owner (test-author lane, Cargo slot owner):** author `crates/providers/tests/keyring_persistence_red.rs` importing only the admitted seam (spec: `keyring_persistence::{BackendHandle, CanonicalDataDir, DeleteOutcome, KeyringService, LoadOutcome, Presence, SecretInput, StoreError, StoreLimits, ...}`), test-owned durable fake backend via injected handle, disposable canonicalized fixture dir. K01 only: `save("openai", canary)` via production orchestration A → drop A → reconstruct B on same durable state → `load == Present(canary)`, `exists == Present`, `delete == Deleted/AlreadyMissing`, `shutdown`. First valid failure = executed behavioral canary assertion. No `keyring` import in the test.
2. **Implementation lane (separate session):** minimal `crates/providers/src/keyring_persistence.rs` (+ `keyring_adapter.rs` for K10 `EntryOnly`/`EntryFactory` mapping) wired through the admitted production caller; dependency written **exactly** `keyring = "=3.6.3"` (or workspace equivalent with exact `=3.6.3`), never looser.
3. **Freeze authority:** record RED hash + command manifest before GREEN.
K06 corrupt-bytes mapping (`CorruptEncoding`) and K10 native mock mapping are separately owned, never restart evidence.

## Exact compiling behavioral RED plan (to run only after the seam is admitted)
```text
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120 cargo test -p opencode-rk-providers --test keyring_persistence_red -- --nocapture --test-threads=1
```
Expected RED: compiles, runs, fails on the canary `load == Present(canary)` assertion after reconstruct (missing durability). Freeze hash before any implementation.

## Lifecycle / denial / no-side-effect tests (frozen RED must include)
- Lifecycle: save → drop handle A → reconstruct B → load/exists → delete → delete-again (`AlreadyMissing`) → shutdown/drain; restart receipt across independent A/B.
- Denial: no-consent / invalid-id / oversize-secret cases assert **absence of side effects** (no file, no entry, empty disposable dir), not just error strings (`docs/SECURITY.md:54-56`).
- Redaction: Debug/Display/serialize projections contain no canary bytes.
- Resource limits: `MAX_SECRET_BYTES = 64*1024`, `MAX_PROVIDER_ID_BYTES = 128`, `MAX_PATH_BYTES = 1024` enforced; bounded store, no unbounded queue/retained output.

## Disposable fake issuer
Test-owned durable fake backend injected via `BackendHandle` (in-memory map on a disposable canonicalized temp dir); isolated fake grant issuer per `docs/SECURITY.md:64-66`. Never the user's real keychain, secrets, files, `.env`, or existing OpenCode database. No native keyring access in tests.

## Resource limits
- Discovery lane: zero Cargo/test/keychain use (honored).
- RED lane: single test target, `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`, `timeout 120`, disposable fixtures only; 8 GiB host budget respected, ≥2 GiB kept free.

## Platform matrix
- RED authors/runs on macOS (this host); installed acceptance additionally requires real Linux x64/arm64 (Landlock attachment) and Windows `x86_64-pc-windows-gnu` runners per synthesis gates `G-LINUX-RUNNER`/`G-WINDOWS-RUNNER`. Signing/notarization external, unsigned candidate only.
- Future `keyring = "=3.6.3"` must resolve on all three mandatory platforms via the integration authority's dependency-acceptance path.

## Authority gate (what unblocks this)
`G-KEYRING-SEAM` (owner: contract-authority, evidence `4814357`): admit an implementation-independent behavioral seam before RED; authorize exact pin `keyring = "=3.6.3"` if/when a dependency is accepted. Until then `KEYRING-DISCOVERY` stays `blocked-discovery`, `SETUP-IMPL` may use brokered credential input but claims no durable OS-secret persistence, and APP-012/Phase 1 stay open per the parent-close rule. No `completed` claimed here; verifier decides on the future RED lane.

## Validation (this lane, read-only)
- `git diff --check`: PASS.
- `grep keyring Cargo.toml Cargo.lock crates/providers/Cargo.toml`: zero hits (no dependency drift).
- `grep BackendHandle|KeyringService|... crates/`: zero hits (no seam drift).
- No Cargo/check/test/freeze run in this lane (not authorized; Cargo slot owned elsewhere). No product/test/dependency edit. No keychain/credential/secret access.
- Remaining unknown: exact text of `55174dc` correction (cited, not present on this branch); authority decision itself.

## Decisions
- Re-confirm `4814357` BLOCKED verdict on current base `7262682` (zero drift in all probed files) rather than inventing a seam.
- Specify minimal caller + RED ownership without authoring any test/product code (writing either here would be implementation-before-RED and file-ownership violation).
