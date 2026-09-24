# INSTALLED-DEFAULT-CONTRACT-INTEGRATION

## Claim

- Task: controller-authorized frozen-contract integration.
- Session: `ses_f2e5bebe3ffeHFPPeWXtRu4mVl`
- Owned test: `crates/cli/tests/installed_default_entrypoint.rs`
- Source commit: `26da87430cb38f34779d92211eecb3e73e3cbf8d`
- Required test SHA-256: `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317`

## Contract

Cherry-pick the source commit onto the product-spine branch. Preserve product-spine claims and incoming task evidence when resolving only `tasks/completion/claims.json`. Do not edit product code or assertions.

Expected focused behavior: without `OC2_E2E_REVISION`, four of five frozen journeys pass and the revision-receipt assertion fails; with `OC2_E2E_REVISION` set to the full current commit hash, five of five pass. The environment value is a test receipt, not release proof. Packaging must inject a truthful receipt.

## Evidence

- Source test hash verified from `26da874`: required SHA-256.
- Source task scratchpad and claims entry included by incoming commit.

## Verification

- Cherry-picked `26da87430cb38f34779d92211eecb3e73e3cbf8d` as `15381e3e4d99c622201ecc89aa0b3391c5854203`.
- Resolved only the claims ledger conflict. Preserved `INSTALLED-DEFAULT-CONTRACT` incoming evidence and product-spine claims; added this integration claim.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 rtk cargo test -p opencode-rk-cli --test installed_default_entrypoint --features native -- --test-threads=1`: expected RED, 4/5 passed, 1 failed at missing revision receipt.
- `OC2_E2E_REVISION=$(git rev-parse HEAD) CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 rtk cargo test -p opencode-rk-cli --test installed_default_entrypoint --features native -- --test-threads=1`: GREEN, 5/5.
- `rtk git diff --check`: clean.
- Test SHA-256: `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317`.

The receipt value is test evidence tied to the source revision, not release proof. Packaging must inject a truthful `OC2_E2E_REVISION` or compile-time `GIT_COMMIT`.

## Remaining

Receipt blocker: installed packaging must provide a truthful `OC2_E2E_REVISION`; no product change authorized in this integration lane. Parent remains open.
