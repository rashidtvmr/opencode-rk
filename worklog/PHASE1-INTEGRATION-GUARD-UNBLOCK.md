# PHASE1-INTEGRATION-GUARD-UNBLOCK

## Verdict

**BLOCKED.** No automated repair is authorized from this lane. The exact decision
owner is the integration authority/controller owner for `ralph.json`,
`FEATURES.md`, `sources/backlog-exhaustion.json`, ownership-gap manifests,
`tasks/completion/claims.json` acceptance state, `PLAN.md`, and verifier/config
authority. This lane may propose reconciliation; it may not change those paths.

Focused candidate evidence does not override the repository guard, convergence
gate, independent verifier, or integrated-parent boundary.

## Baseline and guard evidence

- Base and current `origin/main`: `8a91a7b49a5a1c948218ad8f176d44e015530dcb`.
- Command: `python3 tools/validate_repository.py`.
- Result on `origin/main`: ruleset import OK; readback fixture self-test OK;
  protection OK; `validate_backlog_exhaustion: 51 error(s)`; canonical guard
  stops at `validate_repository: FAIL backlog exhaustion exit=1`.
- Command: `python3 tools/convergence_gate.py`.
- Result on `origin/main`: `CONVERGENCE BLOCKED`, `total=58`; two completed notes
  admit `no acceptance`, and 56 completed claims are off-plan.
- These are read-only observations. No guard, controller, evidence, policy,
  test, source, or Cargo path was changed.

`validate_repository.py:21-28` defines the canonical chain. It runs ruleset
checks, protection, backlog exhaustion, DISC-003 manifest validation, and plan
validation. `validate_repository.py:106-112` fails closed on the first failing
check. `validate_backlog_exhaustion.py:2500-2550` rejects accepted/unknown rows,
category drift, live-ledger drift, and stale source-grounded accounting.

## Complete origin/main guard-error taxonomy

The 51 backlog errors are pre-existing on `origin/main`. They comprise:

| Family | Count | Representative exact errors | Required owner/action |
|---|---:|---|---|
| Backlog accounting/classification | 3 | `backlog exhaustion classifies accepted/unknown stories`; `controller-accepted stories must never appear in the exhaustion ledger`; `backlog exhaustion summary drifted from live Ralph/classification accounting` | Controller/integration authority must reconcile live Ralph classification against the checked-in exhaustion ledger. |
| Routing ownership | 2 | `ROUTE-009: routing ownership-gap is stale after Ralph task semantics changed`; same for `ROUTE-010` | Integration authority reviews `sources/routing-ownership-gap.json` against live task semantics; no lane-side manifest edit. |
| Operations ownership | 14 | For `OPS-001`, `OPS-002`, `OPS-003`, `OPS-004`, `OPS-005`, `OPS-006`, `OPS-008`: `ownership-gap is stale after Ralph task semantics changed` plus `task/worklog appeared; ... needs deliberate review` | Controller/integration authority decides whether task/worklog evidence changes ownership; then reconciles operations manifest and ledger. |
| Release assurance | 6 | For `REL-001`, `REL-002`, `REL-003`: `release assurance gap is stale after Ralph semantics changed` plus `task/worklog appeared; ... needs deliberate review` | Release/integration authority decides task ownership and acceptance evidence; then updates manifest/ledger through protected review. |
| REQ-017 extensibility | 4 | For `EXT-001`, `EXT-002`: `REQ-017 ownership gap is stale after Ralph semantics changed` plus `task/worklog appeared; ... needs deliberate review` | Extensibility/integration authority reconciles requirement ownership and evidence. |
| Sharing ownership | 10 | For `SHARE-001` through `SHARE-005`: `sharing ownership-gap is stale after Ralph semantics changed` plus `task/worklog appeared; ... needs deliberate review` | Sharing/integration authority performs deliberate manifest and task-binding review. |
| Remaining extensibility | 6 | `EXT-004`, `EXT-006`, `EXT-009`, `EXT-010`, `EXT-011`, `EXT-012`: `remaining extensibility gap is stale after Ralph semantics changed` | Extensibility owner updates source-grounded decomposition only after review. |
| Integrations ownership | 6 | `INT-001`, `INT-003`, `INT-005`, `INT-006`, `INT-007`, `INT-009`: `integrations ownership-gap is stale after Ralph semantics changed` | Integration owner reconciles task ownership and residual partitions. |
| **Total** | **51** | Exact full output captured by the command above | **No automatic repair.** |

The counts are computed from the exact 51 bullet lines in the origin guard
output: 3 + 2 + 14 + 6 + 4 + 10 + 6 + 6 = 51. `validate_backlog_exhaustion.py`
contains the family checks at routing `:700-778`, operations `:1018-1047`,
release `:1238-1264`, REQ-017 `:1390-1417`, sharing `:1640-1666`, remaining
extensibility `:1880-1908`, and integrations `:2035-2062`.

Independent convergence findings are not folded into the 51-count. The
convergence gate reports 58 structural/ledger findings: 56 completed off-plan
claims plus `AUD-017` and `AUD-020` notes admitting `no acceptance`. `docs/CONVERGENCE.md:26-28`
keeps such parents open; `docs/CONVERGENCE.md:99-105` states that ledger
`completed` is not acceptance authority.

## Candidate attribution: pre-existing versus lane-introduced

Guard runs were repeated on each candidate worktree. All produced the same
`validate_backlog_exhaustion: 51 error(s)` and `validate_repository: FAIL
backlog exhaustion exit=1`:

| Candidate | Revision | `origin/main` ancestor distance | Guard delta | Classification |
|---|---|---:|---|---|
| Base | `8a91a7b49a5a1c948218ad8f176d44e015530dcb` | 0 | 51 | Pre-existing baseline |
| Batch implementation | `55acf2942d950fea0b4c90c336ec10713f0fa81d` | 184 | 0 | No lane-introduced guard family |
| Batch verifier | `30de11af23a4db7ef701d5a753933a18897954d1` | 185 | 0 | No lane-introduced guard family |
| Cancellation test candidate | `56f8e2b3d9bd1f50481d7c8b0ee9fc0a13b662d7` | 187 | 0 | No lane-introduced guard family |
| Cancellation verifier | `0fb07076d4608f3c7b08eeeb2342cf11b85177d0` | 188 | 0 | No lane-introduced guard family |

Ancestry was checked with `git merge-base --is-ancestor origin/main <revision>`
and `git rev-list --count origin/main..<revision>`. Thus candidate focused
tests do not prove guard cleanliness, and the candidates did not create the
origin guard failures.

### Batch implementation and verifier

`55acf29` changes only `crates/tools/src/registry_dispatch.rs`, its worklog,
and its ledger row relative to its immediate RED parent. The implementation
at `registry_dispatch.rs:278-284` extracts only `Ready::Spawn`; immediate
results remain in the request-order rebuild at `:328-342`. This preserves
single-dispatch parity, denial behavior, no-store-write behavior, and bounded
semaphore use. Frozen RED SHA-256:
`aaad6ab33406a3d5cecf8ca8d5ce0ae6ca16ba7aa22ce264d1abf5240efdc0af`.

The independent verifier at `30de11a` reports:

- `cargo test -p opencode-rk-tools --test registry_batch_immediate_red
  -- --test-threads=1`: 6 passed, 0 failed.
- `cargo test -p opencode-rk-tools --test phase1_shell_broker
  -- --test-threads=1`: 3 passed, 0 failed.
- `cargo check -p opencode-rk-tools`: 0 errors, 6 pre-existing warnings.
- Broad library: 107 passed, 4 pre-existing failures. The verifier classifies
  stale shell expectations, missing macOS `/bin/false`, and stale `disc103_t05`;
  it does not claim those failures are fixed.
- Verifier decision: `ACCEPT. Ready for integration proposal. Not merged, not
  main-pushed, parent acceptance not claimed.`

Safe interpretation: batch is a candidate implementation plus independent
verification, not integrated acceptance. It may be proposed for landing only
after the sequence below and exact integrated-revision reruns.

### Cancellation source/test and verifier

`56f8e2b` adds `crates/tools/tests/phase1_shell_cancellation.rs` and evidence;
it does not add a new implementation beyond the accepted process seam. The
candidate suite ran 9/9 Unix scenarios. Its own evidence explicitly says
`NOT RED. NOT FROZEN`: the accepted seam already supplied the deterministic
behavior before the cancellation suite was authored. Under `docs/TDD.md:43-47`,
that is not a valid compiling RED artifact. It must not be represented as frozen
RED acceptance.

The independent verifier at `0fb0707` reports:

- Unix cancellation suite: 9 passed, 0 failed.
- Broker suite: 3 passed, 0 failed.
- CWD bytes suite: 1 passed, 0 failed.
- `cargo check -p opencode-rk-tools`: 0 errors, 3 pre-existing warnings.
- No surviving fixture processes; Windows is explicitly unsupported.
- Verdict: `ACCEPT for Unix cancellation behavior only`, explicitly excluding
  Windows support, server/registry wiring, parent/release acceptance, and any
  frozen-RED claim.

Safe interpretation: retain as Unix behavior evidence only. Do not claim a
frozen cancellation RED/GREEN lifecycle. A future contract change requiring
new behavior needs an independently authored compiling RED before implementation.

## Authority-safe repair proposal

No repair is authorized in this lane. The following is a proposal for the
integration authority, not an instruction to mutate protected evidence:

1. Freeze the exact target revision and export read-only controller status for
   every error family. Preserve the 51-error baseline and diff against it.
2. Reconcile controller state, `ralph.json`, `FEATURES.md`, and
   `sources/backlog-exhaustion.json` as one reviewed change. Regenerate only
   from live source-grounded semantics. Do not mark work accepted merely to
   suppress errors.
3. Review each ownership-gap manifest against task/worklog appearance. Either
   document why the evidence does not change ownership, or assign a real
   source-grounded child/owner. A manifest update requires the protected-path
   process in `docs/REPOSITORY_PROTECTION.md:10-42` and CODEOWNER review.
4. Reconcile off-plan completed claims through the controller/orchestrator. Do
   not silently delete rows or rewrite notes. `convergence_gate.py` must be
   rerun after controller reconciliation; parents with `no acceptance` remain
   open.
5. Re-run `python3 tools/validate_repository.py` and
   `python3 tools/convergence_gate.py` on the exact candidate revision. A green
   static guard is necessary, not sufficient for release or parent completion.

The reconciliation manifest, `PLAN.md`, and controller state require
integration authority. The manifest is checked by `validate_repository.py:26`;
`PLAN.md` defines acceptance ownership at `PLAN.md:113-120` and release
completion at `PLAN.md:241-251`; `docs/REPOSITORY_PROTECTION.md:150-167`
reserves platform/policy conclusions for an administrator. This lane cannot
repair any of those contracts.

## Safe landing sequence for candidates

1. **Authority gate:** controller/integration owner resolves or explicitly
   records the 51 guard findings. No candidate lands on `main` while the
   canonical guard fails unless the integration authority documents an approved
   blocked state outside this lane.
2. **Batch candidate:** independently verify frozen RED SHA and source diff;
   run the focused batch and broker tests on `55acf29`; run the broad affected
   suite and preserve unrelated failures. Merge `55acf29` only through the
   serialized integration lane, then rerun both frozen targets on the exact
   merged revision.
3. **Cancellation evidence:** do not merge `56f8e2b` as frozen RED acceptance.
   If retaining the Unix test as non-frozen evidence is authorized, land it
   with the verifier evidence only after confirming platform scope and no
   accidental Windows claim. Otherwise keep it as an unmerged candidate.
4. **Verifier:** land verifier records only with their corresponding candidate
   revision and exact hashes. Verifier records never substitute for controller
   acceptance.
5. **Parent/release boundary:** rerun convergence, installed end-to-end
   journey, security/resource checks, and independent verifier on exact
   `origin/main`. Passing focused Cargo tests does not close the parent.

Required focused commands after integration:

```text
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test registry_batch_immediate_red -- --test-threads=1
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test phase1_shell_broker -- --test-threads=1
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test phase1_shell_cancellation -- --test-threads=1
python3 tools/validate_repository.py
python3 tools/convergence_gate.py
```

The cancellation command is evidence-only unless a valid frozen RED exists.

## Rollback and stop conditions

Rollback is revision-scoped and non-destructive: stop promotion, retain remote
candidate branches, and reset the integration branch to its last verified
`origin/main` through the integration owner. Do not force-push, delete remote
lane branches, rewrite frozen tests, weaken assertions, or edit guard output to
obtain GREEN. If a post-merge focused test, guard, verifier, or exact-revision
hash check fails, revert the candidate merge through the reviewed integration
path, preserve logs and hashes, and reopen the candidate. If controller or
platform authority is unavailable, remain `BLOCKED`.

## Evidence boundary

This artifact is a research-only integration proposal. It does not mark batch,
cancellation, a parent, or release accepted. It records no credentials, no
database mutations, and no host-destructive action.
