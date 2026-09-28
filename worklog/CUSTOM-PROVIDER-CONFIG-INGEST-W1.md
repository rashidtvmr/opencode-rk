# Worklog: CUSTOM-PROVIDER-CONFIG-INGEST-W1

## Claim
- Task ID: CUSTOM-PROVIDER-CONFIG-INGEST-W1
- Status: blocked; implementation recovery candidate, parent E2E remains RED at provider turn adapter
- Session: ses_f1c2183f0ffedsFJx4mO9Y33o8
- Owned product file: crates/cli/src/main.rs
- Worktree: /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/impl-custom-provider-config-ingest-w1
- Branch: lane/CUSTOM-PROVIDER-CONFIG-INGEST-W1
- Frozen base: 84f834a9f4bdb1112ad65110b8da456363e2ba86
- Frozen E2E SHA-256: 6803ed3d094a6ce5127c1e3e0e41fe3c6f8c34d280897c8a471dae05b1b74964
- Recovery: prior worker stopped twice before code. User explicitly authorized recovery implementation and recovery verification. Claim reclaimed from stale in-progress row using completion_claims.reclaim evidence, then claimed under this session.

## Contract and current implementation
- Frozen baseline behavior at 84f834a: `crates/cli/src/main.rs::serve` loaded only explicit models file/cache or Catalog::default and built no project config; `crates/catalog/src/lib.rs::Catalog::from_models_dev_api_json` accepts map keyed by provider ID. See initial source evidence at this frozen commit and `worklog/CUSTOM-PROVIDER-CONFIG-E2E-W1.md`.
- Current implementation: bounded JSONC normalizer (64 KiB bytes, depth 64, 8192 nodes, 4096 items per object/array), redacted parse failures, global then project deep merge, arrays/scalars replace, provider projection, catalog built before daemon/listener binds. Config reads only config file data. No config env interpolation, credential file reads or config-content logging.
- Config lookup: global `{data}/opencode.json` then `.jsonc` fallback; project `<cwd>/opencode.json` then `.jsonc` fallback. `.json` wins if both exist. The only checked-in requirement evidence is task target text and `RAW_FEATURE.md:115` naming `opencode.json`; searched worklog and repository text, found no pinned V2 precedence source/test. Do not assert pinned precedence evidence exists. `.json`-first matches documented filename; JSONC remains fallback.
- Global/project application order is executable source-local fixture test: global loaded first, project merged second. `serve` at current `crates/cli/src/main.rs:987-999` loads config and constructs catalog before `SingletonDaemon::bind` and TCP listener bind (`:1001`, `:1033`).
- Frozen E2E source: `crates/cli/tests/custom_provider_config_e2e.rs:345-371` asserts `acme/model-key`; later journey tests turns, persistence, request shape, fallback env key.
- Limits and redacted errors in `main.rs::parse_config`, `validate_config_value`, and `load_config_directory`. File read is bounded to max+1. No environment substitutions performed. Existing unrelated runtime tool env remains unchanged.

## Verification attempts
- `rtk cargo fmt -- crates/cli/src/main.rs`: PASS but repo manifest formatting rewrote unrelated files; immediately restored all paths except owned main.rs.
- `rtk rustfmt --edition 2021 crates/cli/src/main.rs`: PASS but rustfmt recursively reformatted module children; all unowned paths restored. Final scoped rustfmt still needed.
- `rtk env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli --bin opencode-rk config_ingest_tests -- --test-threads=1`: failed pre-test at opentui native build script (dev-dependency activates native). `cargo test --no-default-features` had same dev-dependency failure.
- Allowed metadata query: `rtk cargo metadata --no-deps --format-version 1 ...` reports bin `opencode-rk` plus `oc2`.
- `rtk env CARGO_TARGET_DIR=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/red-custom-provider-config-e2e-w1/target CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli --no-default-features --bin opencode-rk -- --test-threads=1`: still failed at `opencode-rk-opentui-bridge` custom build, because target test dev-dependencies activate native. Exact failure reports `failed to run custom build command for opencode-rk-opentui-bridge`; native artifacts missing for host. No manifest/vendor changes allowed.
- `rtk env CARGO_TARGET_DIR=.../red-custom-provider-config-e2e-w1/target CARGO_BUILD_JOBS=1 cargo build -p opencode-rk-cli --no-default-features --bin opencode-rk`: returned filtered output with warning/codegen lines; exit code 0. Binary exists in both worktree and shared target, 43.5M at `target/debug/opencode-rk`. This provides real no-native binary for E2E but not source-local Cargo tests.
- Direct `rustc` compiling whole main.rs is not a valid target: cannot resolve Cargo dependencies/macros. An early diagnostic caught and fixed invalid byte-string vs `&str` in malformed comment self-test. Avoid repeating this invalid compile route.
- Frozen test SHA was verified before implementation via `rtk shasum -a 256 crates/cli/tests/custom_provider_config_e2e.rs`: exact expected SHA above.
- Last known git status before this worklog rewrite: only `M crates/cli/src/main.rs`, `?? worklog/CUSTOM-PROVIDER-CONFIG-INGEST-W1.md`; no test file edits. Last scoped `git diff --check` passed before final config path edits.

## Remaining verification/landing
- Need run standalone frozen E2E using exact compile command from `worklog/CUSTOM-PROVIDER-CONFIG-E2E-W1.md`, set `OPENCODE_RK_BIN` to fresh no-default-features `target/debug/opencode-rk`, enforce 60s runner timeout, confirm SHA, capture exact failure beyond catalog assertion.
- Need exercise source-local tests using narrow Cargo target only if possible without changing manifests. Otherwise state build/dev-dependency blocker; do not modify tests.
- Need scoped rustfmt without touching other files (use rustfmt `--config skip_children=true` and restore any accidental unowned paths); run git diff --check and ensure changed paths are exactly main.rs, this worklog, claims JSON.
- Update claim via `tools/completion_claims.py` to blocked with exact remaining parent turn-adapter gap and evidence. Commit only main.rs, claims, this worklog. Push `origin/lane/CUSTOM-PROVIDER-CONFIG-INGEST-W1`; no force push.

## Recovery verification (ses current recovery, 84f834a base)
- No prior claim found in ledger; claimed directly under this session.
- Frozen E2E SHA-256 re-verified: 6803ed3d094a6ce5127c1e3e0e41fe3c6f8c34d280897c8a471dae05b1b74964.
- No-default binary build PASS: `cargo build -p opencode-rk-cli --no-default-features --bin opencode-rk`, 43.5M at target/debug/opencode-rk.
- Integration test compile fails: `cargo test --no-default-features` triggers opentui-bridge native build (dev-deps), same prior worker blocker. Source-local config_ingest_tests require binary target build; not runnable without native artifacts.
- Standalone E2E replicated manually against pre-built binary:
  - `/api/models` catalog assertion PASS: `{"count":1,"models":[{"model_id":"model-key","provider_id":"acme",...}]}` includes `acme/model-key`.
  - Turn POST (line 405) FAILS at daemon API: HTTP 503, body `{"code":"service_unavailable","message":"provider credential is unavailable: ACME_API_KEY"}`.
  - Root cause: config specifies `env: ["ACME_PRIMARY_KEY","ACME_FALLBACK_KEY"]`; daemon resolves credential by lookup key `ACME_API_KEY` not present in configured env list. Unwired server/provider adapter in RuntimeWiring/turn handler, outside main.rs scope.
- Scoped rustfmt `--config skip_children=true` PASS (main.rs only). `git diff --check` clean.
- Changed paths: exactly `crates/cli/src/main.rs`, `tasks/completion/claims.json`, `worklog/CUSTOM-PROVIDER-CONFIG-INGEST-W1.md`. No test edits, no implementation edits.
