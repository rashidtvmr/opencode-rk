# APP012-KEYRING-BLACKBOX-RED-SEAM

## Claim and result

- Task: `APP012-KEYRING-BLACKBOX-RED-SEAM`
- Session: `ses_f2b38ca46ffemy32nQfKfaIRla`
- Branch: `plan/APP012-KEYRING-BLACKBOX-RED-SEAM`
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/plan-app012-keyring-blackbox-red-seam`
- Base: `4dc8811b65e6b340afe3b25efc3f55d2bb9d9147`
- Owned artifact: this worklog + own ledger row only
- Route: research/proposal. No Cargo/product/test/dependency edits. No
  Cargo slot use (owned elsewhere). No native keyring, credential, user-DB,
  or env-secret access.
- Result: **BLOCKED**. No policy-compliant existing executable/public caller
  seam admits a compiling behavioral RED for missing credential durability.

## Authority and prior decisions

| Evidence | Fact |
|---|---|
| `docs/TDD.md:43-48` | RED must compile and fail behaviorally; compile failure, missing import, never-executed test invalid |
| `docs/SECURITY.md:24-35,74-82` | No secret logging, no plaintext fallback, disposable fixtures, no user-DB mutation |
| `f92facb` (`red/APP012-KEYRING-PERSISTENCE`) | Honest RED BLOCKED: no admitted persistence seam on that base |
| `55174dc` (correction) | Exact candidate API + bootstrap checkpoint proposal; requested controller authority |
| `4dc8811` (verify, this worktree HEAD) | **B REJECT**: no implementation-before-RED exception exists in policy; no A authorization |
| `crates/providers/src/lib.rs:4-59` | No `keyring_persistence` / `keyring_adapter` module registered |
| `crates/providers/Cargo.toml` | No `keyring` dependency |
| `crates/server/tests/app012_tool_journey_red.rs` | Frozen unrelated APP012 RED, SHA-256 `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50` |

## Current public surface (verified on disk at 4dc8811)

### auth_store: planning only

- `crates/providers/src/auth_store.rs:1-7`: opens no keyring, performs no
  keyring/file/env I/O, retains no secret bytes.
- `crates/providers/src/auth_store.rs:62-90`: `StoreBackend::Keyring/File0600`,
  `resolve(keyring_available: bool)` is caller-supplied planning; File0600
  fallback is the forbidden APP012 path.
- `crates/providers/src/auth_store.rs:92-124`: `StoreError` has only
  `PathNotAllowed/EmptyProvider/ProviderTooLong/TooLarge/KeyringUnavailable`.
  Cannot represent runtime missing, ambiguous, corrupt outcomes.
- `crates/providers/src/auth_store.rs:347-431`: `plan_store`,
  `plan_store_with_secret`, `plan_refresh_persist`, `status_of`,
  `status_with_keyring` (caller boolean; probes no backend). No
  `save`/`load`/`delete`/`exists`, no backend trait, no reconstruction.

### Onboarding: in-memory only, zero production callers

- `crates/cli/src/onboarding.rs:348-398`: `AccountStore` trait +
  `MemoryAccountStore` (documented test/offline fixture; production swap is
  future work, not present).
- `crates/cli/src/onboarding.rs:424-586`: `OnboardingSession::begin`,
  `select_provider` (stages in-memory flag), `submit_credential` (`:517-540`,
  validates `sk-` shape, persists nothing), `select_model` (flips bool).
  Drop the store = total loss; no durable state survives reconstruction.
- `crates/cli/src/onboarding.rs:724-750`: `run_interactive` drives the same
  in-memory session.
- Repo-wide grep: all 25 `OnboardingSession|MemoryAccountStore|run_interactive`
  hits are inside `onboarding.rs` itself. No production caller exists.
- `crates/cli/src/main.rs`: only `mod onboarding;` declaration; no
  setup/onboard/credential subcommand invokes it. No `oc2 setup` executable
  seam exists today (`crates/cli/Cargo.toml` bins `oc2`/`opencode-rk`, no
  setup command wiring).

### daemon_client / daemon / config / responses: env or descriptor only

- `crates/cli/src/daemon_client.rs:775-791`: `creds_configured(_data_dir)`
  ignores the data dir; checks non-empty `OPENAI/ANTHROPIC/GOOGLE/GEMINI_API_KEY`
  env vars only. Env fallback explicitly forbidden for this RED.
- `crates/server/src/daemon.rs`: `PidLock`, `BackendDescriptor`,
  `DaemonPaths::for_data_dir`, `read/publish_backend_descriptor`. No
  credential save/presence/provider-request flow.
- `crates/providers/src/config.rs:84-150`: `from_env`/`get_api_key` env-sourced.
- `crates/providers/src/responses.rs:506-535`: `OpenAiResponsesClient::from_env`
  + retained `api_key: String`. Env-sourced; requires a real credential;
  forbidden and not a keyring path.

### Integration-test patterns

- `crates/providers/tests/prov_022_auth_store.rs`: planning + disposable
  `tempfile` dir; no secret persistence, no reconstruction.
- No `crates/providers/tests/keyring_persistence_red.rs` history exists.

## Four rejected existing-seam options (exact)

### Option 1: real `oc2` onboarding/setup subprocess vs disposable data dir, then reconstruct + provider readiness

- Exact symbols probed: bins `oc2`/`opencode-rk` (`crates/cli/Cargo.toml:9-17`);
  `mod onboarding` (`crates/cli/src/main.rs:49`); no Setup subcommand.
- Rejected: no such executable command exists. Would additionally require
  passing a secret via argv/env (secret-leakage violation), a daemon lifetime
  owner that does not exist, and provider readiness via env (forbidden
  fallback). Cannot compile today; subprocess invention is implementation.

### Option 2: public onboarding service + existing production account store, then reconstruct

- Exact symbols: `onboarding::OnboardingSession::begin`, `select_provider`,
  `submit_credential`, `select_model`; `AccountStore`; `MemoryAccountStore::new`.
- Rejected: `MemoryAccountStore` is the documented test/offline fixture, not
  production orchestration; `submit_credential` never persists; K01 canary
  cannot survive drop-and-reconstruct. A compiling test would assert existing
  in-memory validation (already GREEN behavior), not missing durability, or
  would exercise a test-owned fake against itself (fabricated success,
  forbidden by task and `f92facb:38-44`).

### Option 3: daemon API save/presence/provider-request flow

- Exact symbols probed: `daemon::PidLock::acquire/is_held`,
  `DaemonPaths::for_data_dir`, `read_backend_descriptor`,
  `publish_backend_descriptor`; `daemon_client::creds_configured`,
  `decide_lifecycle_authed`.
- Rejected: no `save`/`load`/`delete`/`exists`/scoped-load symbols exist on any
  of these paths. `creds_configured` is env-only. Nothing to import; a test
  would fail at import/type checking (invalid compile-fail RED).

### Option 4: existing auth_store/config public methods

- Exact symbols: `auth_store::plan_store/plan_store_with_secret/
  plan_refresh_persist/status_of/status_with_keyring/validate_dest/
  validate_secret/AllowlistedPath/StoreBackend/StoreError`;
  `config::ProviderConfig::from_env/get_api_key`.
- Rejected: planning/env only. No operation writes durable state; no
  reconstruction possible. A compiling test would re-prove already-existing
  validation, not the missing behavior. `config` env access is forbidden.

## Secret and process-lifetime constraints

- Secret: no existing seam accepts a bounded owned secret (`SecretInput`) or
  closure-scoped exposure (`with_exposed`). CLI `SecretString`
  (`onboarding.rs:158-184`) and provider `SealedSecret` (`auth_store.rs:30-60`)
  are borrow-validation only with no owner/drain. Any subprocess/env/file
  secret injection would violate `docs/SECURITY.md` no-logging/no-fallback and
  the task ban on real credentials and env fallback.
- Process lifetime: no daemon-owned `KeyringService` or shutdown/drain owner
  exists. Independent process A/B/C restart receipt is impossible without
  inventing orchestration. Test must not touch the native user keyring.

## K01 vs K06/K10 scope

- Black-box RED scope is K01 only: save via production orchestration A, drop A,
  reconstruct B against the same durable state, load exact canary; keyed by
  derived service/account, never retained entry or provider alone.
- K06 corrupt-bytes mapping (`BadEncoding(Vec<u8>)` to redacted
  `CorruptEncoding`) and K10 native `EntryOnly` mock / `EntryFactory` adapter
  mapping require the not-yet-existing `RawBackendError`/`KeyringBackend`
  surface plus `keyring = "=3.6.3"` target deps. Separately owned (K10a
  `crates/providers/tests/keyring_mock_api.rs`, K10b
  `crates/providers/src/keyring_adapter.rs`); never restart evidence; not
  provable from any black-box seam.

## K01 skeleton (spec only, not authored, Cargo slot owned elsewhere)

After a controller-admitted seam + real orchestration, and only then, the
independent RED owner may author `crates/providers/tests/
keyring_persistence_red.rs` importing only
`opencode_rk_providers::keyring_persistence::{BackendHandle, CanonicalDataDir,
DeleteOutcome, KeyringService, LoadOutcome, Presence, SecretInput, StoreError,
StoreLimits, ...}` with a test-owned durable fake backend through the injected
handle, disposable canonicalized fixture dir, `save("openai", canary)`, drop,
reconstruct, `load == Present(canary)`, `exists == Present`,
`delete == Deleted/AlreadyMissing`, `shutdown`. First valid failure: executed
behavioral canary assertion. No `keyring` import. Frozen command:

```text
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120 cargo test -p opencode-rk-providers --test keyring_persistence_red -- --nocapture --test-threads=1
```

Not authored or run in this lane by explicit route.

## Validation (read-only)

- `rtk shasum -a 256 crates/server/tests/app012_tool_journey_red.rs`:
  `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`
- `rtk git diff --check`: PASS
- No Cargo/check/test/freeze run in this lane (not authorized, slot owned
  elsewhere). No product/test/dependency edit. No native keyring or credential
  access.

## Verdict

**BLOCKED**. Exact blocker: no admitted compilable public
credential-persistence seam exists on `4dc8811`; honest compiling behavioral
RED for K01 durability is impossible without inventing imports (invalid
compile-fail RED) or testing a test-local fake against itself (fabricated
success). Unblock requires either a controller policy amendment authorizing
the `55174dc` bootstrap checkpoint (scope, owner, test-freeze authority), or a
policy-compliant pre-existing real seam admitted by integration authority.
`4dc8811` REJECT stands until then.
