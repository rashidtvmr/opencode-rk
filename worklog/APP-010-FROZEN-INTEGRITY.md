# APP-010-FROZEN-INTEGRITY

## Claim
- Task: `APP-010-FROZEN-INTEGRITY`
- Session: `ses_f2e4d3794ffepNEsPynSulLele`
- Owned file: `crates/cli/tests/installed_default_entrypoint.rs`
- Boundary: restore only this frozen test from exact `origin/lane/PHASE1-product-spine-20260923` blob; no product files or `build.rs`; no Cargo.
- Prior-worker boundary violation: reported restoring/editing the frozen test and left uncompiled `build.rs`; `build.rs` remains outside this lane.

## Source evidence
- Canonical ref: `origin/lane/PHASE1-product-spine-20260923`
- Canonical ref commit: `1f4a9e6ce4fa96ea2bf8b01bb034160a9d9c60a9`
- Canonical path: `crates/cli/tests/installed_default_entrypoint.rs`
- Expected SHA-256: `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317`
- Local SHA-256: `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317`
- Canonical blob SHA-256: `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317`

## Observed scenario / decisions
- `origin` fetched before comparison.
- Local file already matched canonical blob byte-for-byte; restoration was not needed.
- No test edit made. No `build.rs` or product file touched.
- No commit/push: shared worktree is dirty; user explicitly prohibited landing in this lane.

## Tests / integrity checks
- `git show origin/lane/PHASE1-product-spine-20260923:crates/cli/tests/installed_default_entrypoint.rs | cmp - crates/cli/tests/installed_default_entrypoint.rs`: exit 0.
- `git diff --exit-code origin/lane/PHASE1-product-spine-20260923 -- crates/cli/tests/installed_default_entrypoint.rs`: exit 0.
- `git status --short -- crates/cli/tests/installed_default_entrypoint.rs`: no output.
- Cargo intentionally not run: known uncompilable `build.rs` outside scope.

## Remaining unknowns
- None for this integrity lane. The known `build.rs` issue remains outside scope.
