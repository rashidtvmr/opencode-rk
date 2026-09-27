# APP-001-CREDENTIAL-PERSISTENCE-RED3

## Claim and intake

- Task: `APP-001-CREDENTIAL-PERSISTENCE-RED3`, test-author vertical lane.
- Route: `xkiro/openai/gpt-6-luna`; explicitly present in the temporary approved allowlist.
- Owned product artifact: `crates/cli/tests/installed_setup_persistence_v2.rs` only; no product edits.
- Claim made with session `ses_f1e9e2dbcffe7Vq1tKm1jIoKXc` before file creation.
- `python3 tools/convergence_gate.py` was run; it is blocked by pre-existing off-plan/completion ledger findings (recorded, not altered).

## Source evidence (candidate 798cbff)

- `crates/cli/Cargo.toml:9-18`: installed package exposes `oc2` and compatibility `opencode-rk` binaries.
- `crates/cli/src/onboarding.rs:424-562`: setup transitions Welcome -> provider -> credential -> model and commits only at model selection; `cancel` removes staged uncommitted accounts at `:565-585`.
- `crates/cli/src/main.rs:218-265`: no-subcommand launch is routed through default launch/daemon planning, not a demonstrated setup persistence journey.
- `crates/cli/src/main.rs:412-438`: existing credential detection is environment-based for known provider keys.
- `crates/cli/src/daemon_client.rs:770-800`: daemon credential environment allowlist is distinct from provider secret storage.
- `crates/cli/tests/installed_default_entrypoint.rs:40-49`: prior installed helper falls back to PATH; this test intentionally requires explicit `OC2_E2E_BIN` and rejects stale/PATH execution.

## Observable contract / boundaries

The test uses `OC2_E2E_BIN` as the exact binary, disposable HOME and data roots, and a bounded PTY harness via macOS `/usr/bin/script` (no shell interpolation). It drives a first-run setup attempt, records bounded output, exits, restarts the same binary, and checks that setup state is consumed rather than silently forgotten. It also checks cancellation does not leave an account marker and that credential material is absent from captured stdout/stderr/argv/URL-like output. Secret values are constants in the test process only and are never written to this scratchpad.

Current CLI source does not expose a documented provider endpoint override for the interactive setup request; therefore the test fails at the first missing observable installed journey rather than fabricating a network probe. The loopback provider portion remains an explicit unresolved gap until a supported endpoint override/account-store surface exists.

Resource bounds: each PTY invocation has a 12-second deadline, output is retained up to 64 KiB, child is waited/reaped, and disposable roots are removed on teardown. No inherited provider environment is passed.

## RED evidence

Pending: compile and run the single new integration target against the freshly built `target/debug/oc2`, then freeze its SHA only after independent RED evidence. Expected failure is an assertion at the first missing setup persistence observable, not a compile failure or `todo!`.

## 2026-09-26 execution update

The compile command was attempted with one Cargo job and one test thread. It did not reach the test target: `crates/opentui-bridge/build.rs:58` aborts because the pinned native `libopentui` artifact is absent for `aarch64-apple-darwin`. This is an infrastructure/build prerequisite blocker, not valid RED evidence; no frozen SHA is claimed. The new test file remains unmodified after authoring.

## 2026-09-26 continuation — corrected PTY RED

- Corrected the earlier invalid test: `stdin` is piped into `/usr/bin/script`, which macOS routes to the caller shell rather than the invoked command. The harness now starts a PTY shell with echo disabled, `exec`s the exact binary from `OC2_E2E_BIN`, sends finite setup lines with flush/bounds, waits/reaps under a 12 s deadline, and retains at most 64 KiB output. No secret is stored in this scratchpad.
- Source evidence: tested binary commit `798cbff99f4e2a2ad21cc937df428eb6bd871cca`; `crates/cli/src/main.rs:358-371` says setup uses `MemoryAccountStore`, drops it at function return, and intentionally does not use data dir. `main.rs:402-438` consumes provider, credential, model and prints setup completion. `onboarding.rs:542-562` commits staged account to the given store only; `onboarding.rs:565-585` cancellation removes staged account. This establishes in-memory state only, not durable account persistence.
- Corrected target test compiles with `rustc --test --edition 2021 -o target/installed_setup_persistence_v2 crates/cli/tests/installed_setup_persistence_v2.rs`.
- Real RED command: `env OC2_E2E_BIN=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/impl-app001-installed-setup-w1/target/debug/oc2 ./target/installed_setup_persistence_v2 --nocapture`; result 3 passed, 2 failed. `first_setup_persists_provider_state_without_secret_output` fails at first missing durable account state (empty data root) after actual `setup complete`; restart test confirms provider-select setup renders again. No mocked success or fake output assertion.
- Secret handling checks captured PTY output and unprotected metadata-like files only. Credentials are excluded from tests' metadata scan when path names explicitly identify a credential/keychain/secret/token store; unknown actual protected-backend semantics remain for review. No argv or URL check is claimed because this test drives setup, not provider execution.
- Cancellation case supplies provider + credential but omits model/commit; no committed provider marker remains. Durable credential backend absent; real provider HTTP consumption is unverified and remains unresolved because this journey has no supported account-store-to-provider endpoint override.
- Frozen RED test SHA256: `ff78c51da1cb130ac7eb935fe498e0a8b8c2520b7a50cbc9a2291ffcb9bfb23d` (`crates/cli/tests/installed_setup_persistence_v2.rs`).

## Remaining gaps

- Real provider request with an exact stored fake credential cannot be asserted until the CLI supports a documented loopback endpoint override through the brokered account-store path.
- Independent verifier must rerun the frozen target and determine whether the failure is the intended missing product behavior.
