# APP012-KEYRING-PERSISTENCE-CONTRACT-VERIFY

## Claim

- Task: `APP012-KEYRING-PERSISTENCE-CONTRACT-VERIFY`
- Session: `ses_f2ccbfb6effeTXk1flxdKqQP55`
- Branch: `plan/APP012-KEYRING`
- Owned file: this worklog only, plus this task's ledger row
- Task type: verification
- No product, test, manifest, lockfile, policy, or prior contract edits

## Verdict

**BLOCKED: contract tip `c56703a` is not ready as written.** The source call-graph
gap and security intent are real. Dependency version/MSRV is confirmed. Several
keyring API, mock, feature-selection, platform, identity, lifecycle, and TDD
ownership claims are false or unproven. Correct these before RED authoring or
dependency integration proposals.

## Source and dependency evidence

| Claim/surface | Evidence | Result |
|---|---|---|
| Workspace MSRV | `Cargo.toml:23`: `rust-version = "1.85"` | PASS |
| Keyring dependency absent | `crates/providers/Cargo.toml` has no keyring entry; `Cargo.lock` has no `keyring` package | PASS |
| Existing provider credential check | `crates/cli/src/daemon_client.rs:776-793`: `creds_configured` checks only non-empty `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GOOGLE_API_KEY`, `GEMINI_API_KEY`, then returns `Some(false)` | PASS, persistence gap confirmed |
| Existing provider request credential source | `crates/providers/src/config.rs:148-150`: `get_api_key` reads `env::var`; `crates/providers/src/responses.rs:516-523`: `OpenAiResponsesClient::from_env` calls it | PASS, no keyring fallback |
| Startup routing | `crates/cli/src/app_start.rs:273-321`: `Some(false)` and `None` route TTY launch to `StartupView::Setup` | PASS |
| Onboarding persistence | `crates/cli/src/onboarding.rs:326-405`: `AccountStore` and `MemoryAccountStore` are in-memory; `select_model` only calls `commit_account` at `crates/cli/src/onboarding.rs:548-570`; no secret-store call | PASS, proposed call graph is not current behavior |
| Secure setup boundary | `crates/providers/src/account_setup.rs:1-8`: caller owns persistence/transport; no production keyring caller | PASS, parent journey remains open |
| Planning seam | `crates/providers/src/auth_store.rs:1-8`: planning-only, no keyring/file/env I/O; `StoreError::KeyringUnavailable` at `:99-110`; 64 KiB constant at `:24` | PASS |
| Provider provenance | `crates/providers/src/auth_profile.rs:17-43,223-241`: `AuthProvenance::Keyring`, secret-free `describe` and debug projection | PASS for metadata only |
| Secret redaction | `crates/cli/src/onboarding.rs:147-215,895-900`: `SecretString` Debug/Display and redaction helper; `crates/cli/src/native_status.rs:402-406`: keyring context is `Visibility::Secret` | PASS for existing surfaces, not proof of future keyring path |
| Frozen APP-012 RED | `crates/server/tests/app012_tool_journey_red.rs` SHA-256 `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`; test sets env at `:324-327`, not keyring | PASS, unrelated to keyring persistence |

The contract's `PLAN.md:72-77` citation for the keyring decision is incorrect:
those lines are inside ADR-002. The dependency decision appears in
`worklog/APP-012.md:70-75`, which is task evidence, not PLAN authority. Use the
actual controller decision path in the integration proposal.

## Dependency audit

Official package metadata/source for `keyring 3.6.3`:

- `https://docs.rs/crate/keyring/3.6.3/source/Cargo.toml`: `edition = "2021"`,
  `rust-version = "1.75"`, `version = "3.6.3"`; compatible with workspace 1.85.
- The same manifest has no `[features] default` set. Native stores are optional:
  `apple-native`, `windows-native`, `linux-native`, and
  `linux-native-sync-persistent`/`sync-secret-service` combinations.
- `https://docs.rs/crate/keyring/3.6.3/source/src/lib.rs`: when no applicable
  native feature is enabled, platform selection falls back to `mock`; macOS,
  Linux, and Windows each have explicit feature-gated native modules.
- `https://docs.rs/crate/keyring/3.6.3/source/Cargo.toml` documents package
  targets in docs.rs metadata as Linux, macOS, iOS, and
  `x86_64-pc-windows-msvc`. It does not prove the product's claimed GNU-only
  Windows acceptance boundary.
- `https://github.com/open-source-cooperative/keyring-rs/blob/main/Cargo.toml`
  identifies keyring `4.2.0` as `rust-version = "1.88.0"`; it is forbidden by
  the workspace MSRV. Do not add it.

### Dependency corrections

| Contract statement | Result | Required correction |
|---|---|---|
| `keyring = "=3.6.3"` is sufficient for native persistence | FAIL | Add target-appropriate feature selection in the serialized integration proposal. Bare 3.6.3 uses mock on platforms without enabled native features. |
| macOS uses Keychain, Linux uses Secret Service, Windows uses Credential Manager | FAIL as a bare-dependency claim | State exact features and target builds. 3.6.3 `linux-native` is keyutils; Secret Service needs `sync-secret-service` plus crypto, or the persistent combo feature. |
| `3.6.3` MSRV 1.75 | PASS | Keep exact pin and record package-source evidence. |
| `4.2.0` requires Rust 1.88 | PASS | Keep forbidden. |
| Windows `x86_64-pc-windows-gnu` is proven by keyring metadata | FAIL | Product may retain GNU-only policy, but it needs a real target build/acceptance receipt. Upstream package docs metadata only shows MSVC. |

## Keyring API and mock audit

The contract uses API spellings not present in 3.6.3:

- `Entry::new(service, user)` is real.
- `set_password` and `get_password` are real.
- Delete is `Entry::delete_credential()`, not `delete_password()`.
- Missing reads and deletes return `keyring::Error::NoEntry`.
- Access failures are `NoStorageAccess` or `PlatformFailure`; error values can
  carry platform detail and must be mapped to fixed redacted application codes.
- `https://docs.rs/crate/keyring/3.6.3/source/src/mock.rs` is authoritative for
  the v3 mock. There is no `keyring::set_default_mock!()` or
  `MockEntryBuilder` API. The supported setup is:
  `keyring::set_default_credential_builder(keyring::mock::default_credential_builder())`.
  Error injection uses `mock::MockCredential::set_error` after downcasting the
  entry credential.

Most important test limitation: v3.6.3 mock persistence is **EntryOnly**. Its
`MockCredentialBuilder` creates a fresh credential per `Entry::new`; data is
held in that entry and does not persist across a newly created entry. Therefore
the proposed T01 "save then retrieve" and T05 delete/retrieve tests cannot
prove a store whose each method reconstructs `Entry::new`. They can only pass
with a retained entry or an additional injected fake keyed by service/account.
The mock cannot prove restart persistence. Real platform acceptance must prove
save, process restart, new `Entry`, retrieve, and delete on a disposable
platform account without mutating local test keychains.

The mock builder is process-global. RED tests must serialize builder setup and
avoid cross-test contamination. Do not run real OS keyring writes in unit/CI
RED.

## State and scenario matrix

| Scenario | Current source | Contract requirement | Verification |
|---|---|---|---|
| First save | No caller exists; onboarding only flips an in-memory bool | `set_password` then durable present | GAP; future RED must use an injectable operation seam, then platform acceptance |
| Retrieve after restart | No keyring read; env-only startup | New process/new entry retrieves same secret | GAP; mock cannot prove it |
| Missing entry | No current keyring path; keyring v3 returns `Error::NoEntry` | `None`/`Some(false)` and Setup | Desired mapping, API correction required |
| Overwrite | No current keyring path; v3 `set_password` updates entry | Same service/user rotates value | Desired, final-state test only; atomicity unproven |
| Delete present | No current keyring path; v3 method is `delete_credential` | Remove and report missing | Desired mapping |
| Delete absent | v3 returns `Error::NoEntry` | Idempotent success | Desired mapping, must be explicit |
| Locked/unavailable backend | No current caller; v3 exposes `NoStorageAccess`/`PlatformFailure` | `StoreError::KeyringUnavailable`, fixed code only | Desired mapping; never serialize OS error |
| Oversize secret | `auth_store.rs:24` and validation enforce 64 KiB at planning seam | Reject before backend write | PASS as seam; future executable test required |
| Invalid provider | `auth_store.rs:268-276` rejects empty, NUL, and >128 bytes | No backend write | PASS as seam; keyring platform identifier constraints still need validation |
| Headless CI | No dependency currently; bare 3.6.3 would select mock | Native backend unavailable maps Setup; tests never touch native store | Contract currently false unless features are integrated |

`StoreBackend::resolve` in `auth_store.rs` can resolve Keyring to File0600 when
availability is false. That is appropriate for the separate PROV-022 fallback
contract, but APP-012's keyring path explicitly says keyring-or-deny. The future
caller must not invoke this resolution to create a plaintext fallback.

## Identity and persistence corrections

- `SERVICE = "opencode-rk"` and account key `provider_id` are proposed values,
  not source-derived facts. They need an integration decision and collision test.
- A fixed service plus provider-only account key collides across multiple data
  directories for one OS user, while `PLAN.md:69-75` defines the singleton as
  per OS user **and data directory**. Either derive a stable data-directory
  namespace, or explicitly document one shared credential namespace and its
  security implications. Do not call the fixed value workspace-anchored.
- Existing `AccountSetupStore` replacement is in-memory metadata semantics, not
  evidence that keyring entries overwrite or delete. Keep those contracts
  separate.
- Keyring operations are synchronous platform calls. The implementation plan
  must state blocking-pool ownership, timeout/deadline behavior, cancellation
  limits, and no detached task. The current contract only says lazy/bounded and
  lacks an executable resource bound.
- `SecretString` is a `String` with `Clone`; ordinary Rust `String` drop does
  not zero memory. `crates/security/src/credentials.rs:91-93` explicitly notes
  zeroization is not implemented. Remove the contract claim that dropped
  `SecretString` bytes are zeroed, or add and independently accept a zeroization
  design. This is a security gap, not a documentation detail.
- Keyring `set_password` is UTF-8 string storage. If future code accepts
  arbitrary secret bytes, use the v3 `set_secret`/`get_secret` API and define
  encoding behavior. Otherwise retain `SecretString` and reject invalid input
  before the backend.
- Keyring deletion atomicity across all OS stores is not established by the
  crate contract. Claim idempotent final state, not cross-platform atomicity,
  until disposable platform evidence exists.

## Redaction/security matrix

| Surface | Evidence/result |
|---|---|
| Secret Debug/Display | PASS: `onboarding.rs:189-203` emits `[REDACTED]` |
| Log/transcript substitution | PASS for caller-known values: `onboarding.rs:206-215,895-900`; future keyring callers must never format retrieved values |
| Store errors | PASS at planning seam: `auth_store.rs:99-110` fixed codes; map all keyring errors without OS detail |
| Provider config | PASS for current config shape: stores env variable name, not env value; `get_api_key` still returns a raw in-memory `String` |
| Auth status/provenance | PASS: `auth_profile.rs` and `native_status.rs` contain no token material and hide Secret visibility |
| Existing DB | PASS by absence of keyring dependency/caller; future implementation must not persist secret bytes in SQLite/config/transcript |
| Zeroization | FAIL: no zeroization guarantee; see `credentials.rs:91-93` |
| Plaintext fallback | PASS as APP-012 requirement only if caller denies on keyring failure; PROV-022 File0600 fallback remains separate |

## RED, implementation, and integration ownership

The contract's proposed ownership says the "implementer lane" owns
`crates/providers/tests/keyring_persistence_red.rs` and then implements the
module. That combines RED authorship and implementation and violates the
independent TDD boundary in `docs/TDD.md:11-15,32-41,77-83` and `PLAN.md:113-119`.

Required split:

1. Independent RED author: new `crates/providers/tests/keyring_persistence_red.rs`,
   with a compiling failure and frozen hash. It must not modify product code,
   Cargo files, or the existing APP-012 RED.
2. Dependency/integration lane: serialized changes to
   `crates/providers/Cargo.toml`, `Cargo.lock`, target feature selection, and
   `crates/providers/src/lib.rs`. It must not author or alter frozen tests.
3. Implementation lane: new persistence module and caller wiring only after RED
   freeze. Shared `crates/cli/src/daemon_client.rs` and
   `crates/providers/src/responses.rs` need explicit integration ownership.
4. Independent verifier: runs the frozen RED, existing regressions, redaction
   checks, disposable platform acceptance, and exact integrated revision.

The existing frozen file remains byte-identical. Its hash was rechecked. No
keyring RED exists in this worktree, so there is no keyring RED hash to freeze
or claim green.

## Required corrections before ACCEPT

- Correct dependency feature selection and platform target evidence.
- Replace v3-inaccurate mock/method names.
- Redesign mock tests so they do not claim restart persistence from EntryOnly
  mock storage.
- Resolve service/account namespace against data-directory isolation.
- Add blocking/cancellation/resource bounds and explicit keyring error mapping.
- Remove false zeroization statement; decide zeroization policy.
- Split independent RED author, dependency integration, implementation, and
  verifier lanes.
- Keep keyring failure keyring-or-deny in APP-012; do not silently invoke the
  PROV-022 File0600 fallback.

## Validation

- `rtk git grep -n 'keyring\|credential\|api_key\|onboard' -- crates Cargo.lock Cargo.toml`: completed; no keyring dependency/module, env-only/current auth surfaces found.
- `rtk shasum -a 256 crates/server/tests/app012_tool_journey_red.rs`: `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.
- `rtk git diff --check`: clean before this worklog/ledger change; rerun before commit.
- Product RED/GREEN: `N/A — no product test: verification lane forbids RED, implementation, and dependency edits.`
- Contract validator: source/dependency/platform/scenario/ownership matrix above; result BLOCKED, not ACCEPT.
- `rtk python3 tools/convergence_gate.py`: `CONVERGENCE BLOCKED`; 91 ledger findings, including completed off-plan `APP012-KEYRING-PERSISTENCE-CONTRACT` and this parent lane's repository state.
- `rtk python3 tools/validate_repository.py`: `FAIL backlog exhaustion exit=1`; 51 backlog errors. No policy/source changes made.

## Remaining unknowns

- Exact target feature matrix and dependency integration owner.
- Product decision for one credential namespace versus data-dir-scoped identity.
- Actual macOS Keychain, Linux Secret Service, and Windows GNU disposable
  acceptance receipts.
- Zeroization policy and whether keyring API should use password or binary
  secret methods.
