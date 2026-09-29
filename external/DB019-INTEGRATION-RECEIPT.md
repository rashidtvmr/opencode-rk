# DB-019 integration receipt

## Candidate identity

- Base/published staging: `origin/wave/1-staging` = `5312b60be2b4a5bb635dfb67946aee4630076712`
- Verified product commit: `3fbbc6646db5c35e1dc5171b57840e27f86f10cd`
- Candidate worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/DB019-integration`
- Product scope: `crates/storage/src/lib.rs`; source SHA-256 `aa76b730b3292adf9abebdfb6b72954015ef7f9f8388b79572645935752cdb42`
- Independent validation: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/DB019-INDEPENDENT-VALIDATION-v9.md`, SHA-256 `e3163956ea4070ab08becca01a7695e94e73cfb31bc04e968ff7869779356ee`

## Verification

Executed sequentially under Python process-group supervision, `CARGO_BUILD_JOBS=1`,
`RUST_TEST_THREADS=1`, `--locked --offline`, one heavy slot, 300-second bound:

1. `cargo test --locked --offline -p opencode-rk-storage --test typed_history -- --test-threads=1`: exit 0, 7 passed, 0 failed. Log: `external/db019-v9-typed_history.log`.
2. `cargo test --locked --offline -p opencode-rk-storage --lib -- --test-threads=1`: exit 0, 122 passed, 0 failed. Log: `external/db019-v9-lib.log`.

Max RSS for the independent receipt was 72,515,584 bytes. Frozen hashes remain
DB-021 `7291dd0d97b65d5c131086e67bc8372a98c26c136a6dd9a6566f3f25067e2aef` and
DB-022 `b767bf2fb5647ca3065b6f3c922a5197c372763f45a7abbf6c15db31139ffcdf`;
TUI-012 frozen source is unchanged. `lane_gate.py` has no DB-019 mapping and was
intentionally not invoked with unrelated lanes.

## Claims and authority

Staging claims were read from `5312b60be2b4a5bb635dfb67946aee4630076712` and had
206 rows. Their rows and blocked fences were preserved; the candidate adds only the
integration session's explicit in-progress DB-019 ownership row through
`tools/completion_claims.py`. The prior repair owner was `ses_f1460e6d4ffeRJJGfqYV56e0AA`;
this candidate does not silently overwrite that historical ownership or claim
completion. DB-019 remains in-progress pending independent component gate and caller
handoff. DB-020 is not leased and is dependency-blocked until its caller contract
and verification are satisfied.

The missing partial `external/DB019-INTEGRATION-PREP.md` was not fabricated; its
known source artifacts and v9 receipt were preserved/referenced. No main merge,
acceptance, controller, security, dependency, verifier, or frozen-test file was
changed.

## Gate plan / unresolved boundary

- Component candidate: migration and storage regression evidence is sufficient for
  independent integrator review, subject to exact-tree receipt verification.
- DB-019 completion: not asserted here; the focused suites do not exercise every
  newly added typed writer/history path and no independent caller/runtime proof is
  present in this lane.
- DB-020 handoff: candidate may be consumed only after the DB-020 owner confirms
  the exact public bridge adaptation (`Arc<str>` identity and structured history)
  and writes its own RED/GREEN evidence. No invented DB-022 HTTP prerequisite is
  added.
- Parent/convergence: blocked independently by the existing convergence findings;
  this candidate does not call parent complete.
