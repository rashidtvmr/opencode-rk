# APP-010-REVISION-RECEIPT-INTEGRATION

## Claim
- Task: `APP-010-REVISION-RECEIPT-INTEGRATION`
- Branch: `lane/PHASE1-product-spine-20260923`
- Prior owner: `ses_f2e29869bffeaVXDcsIsbjwjDl` (claim status `in-progress`; worklog showed audit begun, no landing).
- Reclaiming session: `ses_f2e1fb204ffeLVobC1YJZw1QxN`.
- Reclaim evidence recorded via `tools/completion_claims.py::reclaim`: prior owner exhausted steps after full GREEN verification and before landing; no landing commit/push; frozen hash and GREEN evidence recorded in `worklog/APP-010-REVISION-RECEIPT.md`. Reclaim returned row to `not-started`, then re-claimed by this session.
- Boundary: integrator only. No product or test edits. Changes limited to this worklog and the `tasks/completion/claims.json` ledger row.

## Pre-landing scope audit (this session)
- Branch/ref parity: `HEAD == origin/lane/PHASE1-product-spine-20260923 == 1f4a9e6ce4fa96ea2bf8b01bb034160a9d9c60a9`.
- Frozen test local SHA-256: `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317`.
- Frozen test canonical blob SHA-256 (`origin/...:crates/cli/tests/installed_default_entrypoint.rs`): `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317`.
- `git diff --exit-code origin/lane/PHASE1-product-spine-20260923 -- crates/cli/tests/installed_default_entrypoint.rs`: exit 0 (byte-identical, no test edit).
- Dirty scope exactly the permitted five files, no others:
  - `crates/cli/build.rs` (new)
  - `worklog/APP-010-FROZEN-INTEGRITY.md` (new)
  - `worklog/APP-010-REVISION-RECEIPT.md` (new)
  - `worklog/APP-010-REVISION-RECEIPT-INTEGRATION.md` (new)
  - `tasks/completion/claims.json` (modified: ledger rows)
- `git diff --check`: exit 0.
- `rustfmt --edition 2021 --check crates/cli/build.rs`: exit 0.

## Prior verifier evidence (trusted, cited; not re-run before landing)
From `worklog/APP-010-REVISION-RECEIPT.md` (frozen RED hash unchanged throughout):
- `rustfmt --edition 2021 --check crates/cli/build.rs`: GREEN.
- `env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo check -p opencode-rk-cli --features native`: GREEN, 0 errors, pre-existing warnings only.
- `env -u OC2_E2E_REVISION CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli --features native --test installed_default_entrypoint -- --test-threads=1`: GREEN 5/5 (compile-time receipt from HEAD `1f4a9e6`).
- `env OC2_BUILD_REVISION=1f4a9e6ce4fa96ea2bf8b01bb034160a9d9c60a9 ... --test installed_default_entrypoint`: GREEN 5/5 (explicit receipt path).
- Invalid explicit probe (`OC2_BUILD_REVISION=invalid-receipt-SENSITIVE-9`): exit 1, fixed diagnostic only, supplied value not echoed.
- No-repository probe (`PATH=/nonexistent`): exit 0, only `cargo:rerun-if-env-changed=OC2_BUILD_REVISION`, no fabricated receipt.
From `worklog/APP-010-FROZEN-INTEGRITY.md`:
- Canonical frozen test already byte-identical; `cmp` and `git diff --exit-code` both exit 0.

## Landing
- `git add crates/cli/build.rs worklog/APP-010-FROZEN-INTEGRITY.md worklog/APP-010-REVISION-RECEIPT.md worklog/APP-010-REVISION-RECEIPT-INTEGRATION.md tasks/completion/claims.json`
- `git diff --cached --stat`: 5 files changed, 356 insertions(+), 0 deletions.
- `git commit -m "APP-010: land compile-time revision receipt (build.rs) with frozen-integrity evidence"`: `c4325e4381741bf79cc39eb7a485d73ca7ef4f52`.
- `git push`: `1f4a9e6..c4325e4 lane/PHASE1-product-spine-20260923 -> lane/PHASE1-product-spine-20260923`.
- `git fetch origin`: HEAD `c4325e4381741bf79cc39eb7a485d73ca7ef4f52` == `origin/lane/PHASE1-product-spine-20260923` (HEAD==origin verified).

## Post-push integrated-revision verification (exactly one command, fresh bounded target)
- Fresh target: `CARGO_TARGET_DIR=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/app010-postpush-target.sdl1gy` (outside repo; worktree clean before and after).
- Command:
  `env -u OC2_E2E_REVISION CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 CARGO_TARGET_DIR=<fresh> cargo test -p opencode-rk-cli --features native --test installed_default_entrypoint -- --test-threads=1`
- Result: exit 0; `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.96s`.
  - `bare_no_command_enters_planned_native_route ... ok`
  - `missing_credentials_open_setup_without_offline_instruction_or_secret ... ok`
  - `native_renderer_emits_frame_and_restores_alternate_screen ... ok`
  - `rerun_same_executable_and_revision_is_deterministic ... ok`
  - `single_authenticated_daemon_reused_by_second_client ... ok`
- Log: `.../app010-postpush-target.sdl1gy/postpush.log` (build warnings only; unrelated pre-existing dead-code warnings across other crates).
- Frozen test SHA-256 after run: `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317` (unchanged).
- `git status --porcelain=v1` after run: empty (no stray files).

## Second evidence-only commit
- Changes only this worklog and the ledger row; no product/test source.
- Source/test diff between the two commits proven empty for the frozen test and `crates/cli/build.rs` (see landing verification). No second build required.

## Remaining unknowns
- APP-010 parent stays open for packaged archive binding; this integration lands the compile-time receipt + installed journey, not the packaged-archive story.
