# APP-001-FILE0600-SETUP-BASE-W1

## Claim
- Session: `ses_f1a1c1349ffepkhimlj6Jn4qNj`
- Branch: `integration/APP-001-FILE0600-SETUP-BASE-W1`
- Candidate base: `96b4a33ddee7d39fa5de508c8bea701326b23324`
- Integrated HEAD: `d59c0d1` (full hash recorded in handoff)
- Scope: integration only; no new product code; no `main.rs` or test edits.

## Source evidence
- Read first: `AGENTS.md`, `.agents/WORKER.md`, `PLAN.md`, `docs/TDD.md`, `docs/SECURITY.md`, `docs/CONVERGENCE.md`.
- `tools/completion_claims.py:94-115` claims before worklog creation; ledger updated only via this module.
- Candidate `96b4a33` parent `84f834a`; credentials `8653719`, backend `d137a09`, provider lock `ff61a62` were non-ancestors. Cherry-picked only reviewed commits and required frozen RED commit `60f0a6c` (not intervening/unrelated history).
- Backend exports `write_atomic_file` / `load_credential` in `crates/providers/src/auth_store.rs`; provider Cargo requires `rustix` `fs,process`, CLI path dependency and lock record present.
- `crates/cli/src/main.rs` from config candidate is byte-identical: SHA-256 `5c7aebea67b46f439d3b4a4d1c644617cef6005a9c39a2abf9ffefc907f06b8c`; config-ingest symbols include `load_config_directory`, `project_custom_providers`.

## Integration
- Cherrypicks: `6f3333f` rustix fs prewire, `d137a09` File0600 backend, `ff61a62` providers lock, `8653719` credential semantics, `60f0a6c` frozen File0600 test, `5117d76` CLI providers dependency.
- Claims conflicts were union-merged via `completion_claims.save_ledger`; zero duplicate-different rows. Each claim reapplied through `completion_claims.claim` or retained current task ownership. Backend/provider/test claims and worklogs retained.
- No edits to product files beyond cherry-picking reviewed source commits; no edits to `main.rs` or tests.
- Frozen test `crates/cli/tests/installed_setup_file0600.rs`: SHA-256 `c99b15bca871f1258f4ae3c886a57cc108a695476083da057caeae7cec0d3e40`, 288 logical source lines (`wc -l` reports 288).

## Verification
- `rustc --edition 2021 --test crates/cli/tests/installed_setup_file0600.rs -o /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/installed_setup_file0600`: compiled.
- `OC2_E2E_BIN=target/debug/oc2 RUST_TEST_THREADS=1 /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/installed_setup_file0600 --test-threads=1`: 2/5 pass; expected setup caller gap remains: t01/t02 missing `<data_dir>/credentials`, t04 accepts pre-created symlink. t03/t05 pass. These are next setup lane work, not hidden/edited.
- 14-test onboarding harness: took frozen onboarding source from RED `09a590e`, applied only the exact reviewed `8653719` source hunk in `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/onboarding_frozen_14.rs`; `rustc --edition 2021 --test ... && ... --test-threads=1` -> 14 passed. Repo source/tests untouched by harness generation.
- Current integrated `onboarding.rs` local 13-test tests fail `secret_never_logged`, because its baseline invalid-key fixture is accepted by the newly authorized no-universal-prefix semantics; the frozen 14-test RED commit includes the corrected whitespace/control fixture. No test edits made to resolve this.
- `env CARGO_BUILD_JOBS=1 cargo check -p opencode-rk-cli --bin oc2 --no-default-features --locked`: PASS, 469 pre-existing warnings.
- `env CARGO_BUILD_JOBS=1 cargo build -p opencode-rk-cli --bin oc2 --no-default-features --locked`: PASS, 469 pre-existing warnings.
- `git diff --check`: PASS. Frozen test hash verified from final HEAD.
- `python3 tools/convergence_gate.py` on source candidate: BLOCKED by 60 pre-existing ledger findings (off-plan claims, no-acceptance notes); no gate/policy files changed.

## Remaining boundary / stop
- Candidate is a next-lane base, not application acceptance. Frozen installed setup RED is exact: 3 expected caller-boundary failures remain. Parent setup stays open.
- `vm_stat` after focused verification showed 22,992 free 16KiB pages (~359 MiB); no further resource-heavy checks run.
- Next lane must start from this branch HEAD and own approved setup caller integration; do not change frozen tests or source base `main.rs` here.
