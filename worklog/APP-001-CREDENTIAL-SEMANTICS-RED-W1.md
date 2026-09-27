# APP-001-CREDENTIAL-SEMANTICS-RED-W1

## Task
TEST-AUTHOR APP-001-CREDENTIAL-SEMANTICS-RED-W1: authorized test-only onboarding credential semantics. New test requires accepting arbitrary bounded non-whitespace credentials without a universal `sk-` prefix. No production changes.

## Claim and candidate
- Session: `ses_f1c36f94affe7JqD2IHy9c5jzK`
- Prior task claim was absent from this worktree's ledger; claimed directly via `tools/completion_claims.py`.
- Base: `ff61a62e6513addd53fe7d1f5f98165baea5dc2d`
- Branch: `red/APP-001-CREDENTIAL-SEMANTICS-W1`
- User-authorized worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/red-app001-credential-semantics-w1`

## Contract and source evidence
- `crates/cli/src/onboarding.rs:517-540`, `submit_credential`: current validation enforces `MIN_CREDENTIAL_LEN`, `MAX_SECRET_LEN`, `raw.starts_with("sk-")`, and no control/whitespace; success advances to `ModelSelect`. Failure returns a fixed `InvalidCredential` without copying the secret.
- `crates/cli/src/onboarding.rs:887-910`, `secret_never_logged`: redaction/format checks now use a short invalid whitespace/control fixture; exact fixture bytes must not appear in errors.
- User-authorized requirement: upstream OpenCode V2 semantics accept arbitrary bounded non-whitespace credentials; remove universal `sk-` requirement in the separate implementation lane.
- Existing repository test code was modified only at the authorized `secret_never_logged` fixture and new test block. No production code, manifest, or test runner changes.

## RED tests and freeze
- Modified `secret_never_logged` block SHA-256: `03d1623ed0500cbc2e5db830fbc74ab43efda132aee616047536940ef34d7ffe` (1,588 bytes). Stable extraction: bytes from `    #[test]\n    fn secret_never_logged() {` up to the next `    #[test]`.
- New `credential_semantics_accepts_non_sk_credential` block SHA-256: `9b1225c2c7a9ddf7281b8d2005234926bf76a8339399f4c4fa8ca3a41d2bdd6c` (279 bytes). Stable extraction: from `    // APP-001-CREDENTIAL-SEMANTICS:` up to the next `    #[test]`.
- Whole `onboarding.rs` candidate SHA-256: `2e561f9bb2b3f3120ef0b32d99a00ff4b07eb920103d51f9faeea98fc14274a7`.
- Standalone compile/run against real source: `rustc --edition 2021 --test crates/cli/src/onboarding.rs -o /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/onboarding-credential-tests && /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/onboarding-credential-tests --test-threads=1` -> 14 tests, 13 passed, 1 failed. `secret_never_logged` passed; new test failed only at `submit_credential(...).unwrap()` with `InvalidCredential { reason: "credential rejected", action: RetryCredentialEntry }`; remaining onboarding tests passed.
- The test asserts ModelSelect transition, model selection, Done/committed state, and exact credential absence from session Debug. No mocks.

## Cargo attempt and blocker
- Ran required focused command: `cargo test -p opencode-rk-cli --bin oc2 --no-default-features onboarding -- --test-threads=1` (CARGO_BUILD_JOBS=1, RUST_TEST_THREADS=1). It did not reach tests: `opencode-rk-opentui-bridge` custom build failed.
- Verified root cause: `crates/cli/Cargo.toml:42-44` dev-dependency enables `opencode-rk-opentui-bridge` `native`; `crates/opentui-bridge/build.rs:50-62` fails unless a native artifact exists under `native/lib/<TARGET>`. Host is `aarch64-apple-darwin`; required `libopentui.a` or `libopentui.dylib` absent from `crates/opentui-bridge/native/lib/aarch64-apple-darwin`. Existing unrelated untracked `x86_64-apple-darwin/` directory was left untouched.
- `rustfmt --check --edition 2021 crates/cli/src/onboarding.rs` reports existing formatting diffs at lines 136, 383, 943; these are outside changed blocks and were not rewritten. New/modified test blocks compile in `rustc --test` harness.

## Authorization
User authorized route `@9router-luna-free`, existing worktree/branch, test-only scope, focused cargo test then source-local standalone harness if native artifact blocks, separate block hashes, blocked claim, and commit/push.

## Status and remaining
Blocked test-author candidate: compiling RED verified by standalone test harness; Cargo target blocked by absent host native dependency. No production implementation, acceptance, or claim of GREEN. Parent APP-001 remains open pending implementation lane and integrated frozen-test verification.
