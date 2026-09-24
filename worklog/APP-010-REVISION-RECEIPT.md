# APP-010-REVISION-RECEIPT

## Claim
- Task: `APP-010-REVISION-RECEIPT`
- Session: `ses_f2e4bc295ffeIavwxz3k0Ab2IR`
- Owned product file: `crates/cli/build.rs`
- Frozen test: `crates/cli/tests/installed_default_entrypoint.rs`, SHA-256 `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317`; read-only.
- Claim inspection found no existing `APP-010-REVISION-RECEIPT` row before this session; `completion_claims.claim` succeeded for this session. No prior owner row was present to reclaim.

## Source evidence
- `crates/cli/Cargo.toml:1-18`: package has CLI binaries; Cargo auto-discovers `crates/cli/build.rs`.
- `crates/cli/tests/installed_default_entrypoint.rs:356-360`: receipt lookup uses runtime `OC2_E2E_REVISION`, then compile-time `GIT_COMMIT`.
- `crates/cli/tests/installed_default_entrypoint.rs:530-535`: missing or empty receipt fails the deterministic journey.
- `tasks/completion/claims.json:458-466`: prior receipt lane blocked because no compile-time `GIT_COMMIT`; frozen test already integrated and immutable.
- `crates/cli/build.rs:23-48`: precedence, validation, env emission, rerun trigger.
- `crates/cli/build.rs:62-167`: repository/worktree metadata discovery and bounded watch paths.
- `crates/cli/build.rs:168-210`: bounded direct Git argv, environment clearing, output validation.

## Observable contract
- `OC2_BUILD_REVISION`, when present, takes precedence, accepts exactly 40 lowercase hexadecimal bytes, and invalid input fails with a fixed diagnostic without echoing the supplied value.
- Without explicit input, only direct `git rev-parse --verify HEAD` from a discovered repository root may provide the receipt. Output is bounded and validated identically.
- Valid output emits `cargo:rustc-env=GIT_COMMIT=<validated>` and always emits `cargo:rerun-if-env-changed=OC2_BUILD_REVISION`.
- Discoverable HEAD, symbolic ref, packed-ref, and worktree common metadata receive individual rerun triggers; `.git` is never recursively watched.
- Missing repository metadata or unavailable/invalid git output omits `GIT_COMMIT` and does not break compilation.
- The receipt identifies base revision only; dirty content is not represented.

## Failure states and bounds
- Invalid explicit value: deterministic build-script error; secret/value not printed.
- Absent explicit value plus unavailable git: no receipt env, successful build-script exit.
- Metadata files and git output are bounded at 4096 and 128 bytes respectively.
- Git receives no inherited environment beyond PATH plus non-interactive config controls; stderr is discarded.

## Decisions
- Standard library only. Direct `Command` argv, no shell.
- Worktree `.git` pointer and `commondir` are handled for stale-HEAD invalidation.
- No warning emitted for dirty state or missing git, avoiding fabricated claims and output leakage.

## Tests / verification
- `rustfmt --edition 2021 --check crates/cli/build.rs`: GREEN.
- `env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo check -p opencode-rk-cli --features native`: GREEN; 0 errors; pre-existing warnings only.
- `env -u OC2_E2E_REVISION CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli --features native --test installed_default_entrypoint -- --test-threads=1`: GREEN 5/5; compile-time receipt derived from exact HEAD `1f4a9e6ce4fa96ea2bf8b01bb034160a9d9c60a9`.
- `env OC2_BUILD_REVISION=1f4a9e6ce4fa96ea2bf8b01bb034160a9d9c60a9 CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli --features native --test installed_default_entrypoint -- --test-threads=1`: GREEN 5/5; explicit receipt path.
- Invalid direct build-script probe with `OC2_BUILD_REVISION=invalid-receipt-SENSITIVE-9`: exit 1; output only fixed `OC2_BUILD_REVISION must be exactly 40 lowercase hexadecimal characters`; supplied value absent.
- No-repository direct build-script probe with `PATH=/nonexistent`: exit 0; emitted only `cargo:rerun-if-env-changed=OC2_BUILD_REVISION`; no fabricated receipt.
- Frozen test SHA-256: `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317`.
- `git diff --check`: GREEN.

## Remaining unknowns
- Native CLI check and macOS installed journey may be limited by unrelated existing workspace/native-link failures; record exact output.
