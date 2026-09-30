# V2-SALVAGE-PRESERVATION worklog

## Package / gate
- Package: V2-SALVAGE-PRESERVATION (bounded preservation inventory repair)
- Gate: G0 pre-V2 preservation inventory correctness
- Base SHA: `fc2d201f450b6020b1dce12268a550053d65235d` (main-v2)
- Branch: `v2/salvage-preservation` (worktree `/Users/mymac/Projects/opencode-rk-v2-salvage-preservation`)
- Status: PREVERIFIED on the isolated branch; NOT yet ACCEPTED on an integrated SHA.

## Changed paths (owned only)
- `tools/convergence_v2_salvage.py` (rewritten; schemaVersion 2)
- `docs/convergence-v2-salvage.json` (regenerated; 2.8 MB -> ~1.2 MB)
- `docs/CONVERGENCE_V2_SALVAGE.md` (regenerated report)
- `worklog/V2-SALVAGE-PRESERVATION.md` (this file)

No source/product/tests/legacy-ledger edits. No `main-v2` mutation.

## Frozen-input preservation history
Prior outputs were copied read-only into the external snapshot before regeneration:
- `v2-worktree-preservation-20260930-1400/prior-convergence-v2-salvage.json`
  sha256 `966afcf4e85f7d10fe2d6e654f3b901260b394f9ad384dff20905f0b43612c3c`
- `v2-worktree-preservation-20260930-1400/prior-CONVERGENCE_V2_SALVAGE.md`
  sha256 `ddc8e89c52673b16dad3660e81871a1fb91aafaed09c7b3e8c267bde2d8d97ef`
- `v2-worktree-preservation-20260930-1130/v2-worktree-preservation-20260930-1400/prior-convergence_v2_salvage.py`
  sha256 `7c9b887a8e4b4a307f94735dfd9487565d2e1f4bbf62011020fea52761ab9a3c`

## Defects found and fixed (proven by fixture before code)
Scoped self-check fixture (`--self-check`, disposable temp repo, no Cargo):
`SELF-CHECK OK: dup=('SUPERSEDED_BY', 0, 1) product=('SALVAGE_PRODUCT', 1, 0) test=('SALVAGE_TEST', 1, 0) process=('PROCESS_ONLY', 1, 0) aliases_dedup=1`

1. Stale base: previous JSON used `baseSha=2798d151...` which is not on `main-v2`.
   Fixed: `--base-sha` is an explicit required context argument (`fc2d201...`).
2. Enumerated live refs only (heads+origin), so `main-v2`, `v2/*`, archive, ledger,
   stash were invisible. Fixed: frozen `refs-before.txt` is the authority; 659 refs
   (342 heads / 317 remotes / 0 others).
3. Wholesale three-dot diff misclassified inherited/cherry-picked commits as salvage.
   Fixed: `git rev-list --no-merges` + `git cherry` patch-id equivalence; only `+`
   patches are novel; `-` patches become `SUPERSEDED_BY` (0 novel).
4. No dedup of identical tips. Fixed: tips grouped by SHA, aliases recorded
   (`aliasedRefs=328`).
5. No worktree/detached/stash/reflog/archive coverage. Fixed: 112 descriptors,
   45 detached anchors, 6 stash entries (read-only, never popped), 6 evidence refs.
6. No dirty metadata. Fixed: bounded `git status --porcelain -z` (25 s timeout) for
   every frozen worktree; 18 dirty, 0 timeout, 0 missing.
7. Unreviewed substantive code auto-discarded. Fixed: `candidateGroups` intake list;
   `acceptance=NOT_ACCEPTED` for all novel content; nothing auto-deleted.
8. No intake-vs-final distinction. Fixed: `acceptance` field plus `distinction` block.

## Generated coverage (exact)
- frozen refs: 659 = 331 unique tips + 328 aliases (reconciles exactly)
- by unique tip: INTEGRATED_ANCESTOR 41, SUPERSEDED_BY 4, PROCESS_ONLY 44,
  SALVAGE_PRODUCT 224, SALVAGE_TEST 17, PENDING_REVIEW 1
- worktrees: 112 total, 112 present, 0 missing, 18 dirty, 0 status-timeout
- detached anchors: 45
- stash entries: 6 (`refs/stash` = `eda2b050...`)
- archive/ledger evidence refs: 6 (5 archive + `refs/ledger/canonical`)

Note: `SALVAGE_PRODUCT` fell from a stale 441 (inflated by inherited commits) to a
patch-id-correct 224. This is a correctness improvement, not a loss of coverage.

## Dirty preservation snapshot
- Dir: `/Users/mymac/Projects/opencode-rk-pre-v2-20260930-1130/v2-worktree-preservation-20260930-1400`
- Preserved 18 dirty worktrees, 2,091,197 bytes, per-worktree `tracked.patch` +
  `untracked/` copies. No owner file mutated; no reset/clean/prune.
- Budget skips (left in place, recorded): `docs/plans/phase1/audit/legacy-crosswalk.json`,
  `docs/plans/phase1/phase1-tasks.json` (opencode-rk-phase1-plan);
  `off/local_application-.../dep-graph.part.bin` (opencode-rk-phase1-restored).
- Sensitive matcher (conservative) skipped without content:
  `tests/e2e/browser_credential_launch.rs` (opencode authweb-prep worktree).
  This is a likely FALSE POSITIVE — a `.rs` test source, not a secret. It is a
  preservation gap, not a leak; the file remains in place, untouched.

## G0 status
`pending`. Root worktree dirty state was previously captured by the integrator's
`root-untracked.tar.gz`; this pass adds per-worktree dirty capture. Because the
matcher skipped one likely-non-secret source file (and 2 files exceed budget), G0 is
NOT claimed satisfied. No fake backed-up claim is made.

## Remaining observed failures / risks
- 2 oversized files not snapshotted (still in place).
- 1 conservative sensitive false positive not snapshotted (still in place).
- This is isolated-branch PREVERIFIED only; acceptance requires the integrator to
  integrate and re-run on the exact integrated SHA.

## Verification commands / results
- `python3 tools/convergence_v2_salvage.py --self-check` -> `SELF-CHECK OK ...`
- `python3 tools/convergence_v2_salvage.py --base-sha fc2d201... --frozen-root <frozen> ...`
  -> `{"base":"fc2d201...","counts":{...},"frozenRefs":659,"uniqueTips":331,"worktrees":{...}}`
- reconciliation: `frozenRefs 659 = aliased+unique 659`
- `du -sh <snapshot>` -> 5.3M (includes prior-copy history + 18 preserved worktrees)