# V2-SALVAGE-PRESERVATION worklog (round 3 — staged-only reconciliation)

## Package / gate
- Package: V2-SALVAGE-PRESERVATION (bounded preservation inventory repair, fail-closed)
- Gate: G0 pre-V2 preservation inventory correctness
- Base context SHA: `0088fbf2ea65251f344915c93bd8679c0c467320` (explicit; not stale acceptance)
- Branch: `v2/salvage-preservation`
- Pre-fix candidate reviewed: `171bbb7` (independently reviewed, not integrated)
- Status: PREVERIFIED independently on the candidate; NOT ACCEPTED on an integrated SHA.

## Changed paths
- `tools/convergence_v2_salvage.py` (schemaVersion 3)
- `docs/convergence-v2-salvage.json` (regenerated)
- `docs/CONVERGENCE_V2_SALVAGE.md` (regenerated)
- `worklog/V2-SALVAGE-PRESERVATION.md` (this file)
- `tests/bootstrap/test_convergence_v2_preservation.py` (independent frozen regression)

The implementation path is the preservation tool; an independent test owner
authored the regression suite. Canonical integration is performed by the integrator.

## RED evidence against pre-fix `171bbb7` (fail-open proven before code)
Frozen copy sha256 `dce663726053ed9b004dd2c566ad42e8dcd1b20e3ad11b04de3fa2a9556c697`.
```
RED classify_empty     -> ('SUPERSEDED_BY', 'INTEGRATED')   # empty => fail-open heritage
RED novelty_gitfail    -> ([], [], [])                       # failure cached as "no novel"
RED sensitive_rs       -> True                               # .rs source false-positive
RED sensitive_env      -> True
```

## Fixed behaviour (GREEN self-check)
`rtk /usr/bin/arch -arm64 /usr/bin/python3 tools/convergence_v2_salvage.py --self-check` ->
`SELF-CHECK OK: fail-closed=PENDING merge-only=PENDING unique-merge=PENDING staged-only=captured staged/unstaged/rename=separate safe-source=ok secret-deny=ok`

Fixtures executed (disposable temp repos, no Cargo):
1. git command failure/timeout -> `Analyzer.novelty` returns `None` (never cached empty);
   `decide(ok=False,...)` -> PENDING_REVIEW / NOT_ACCEPTED; `classify(False,[])` -> PENDING_REVIEW.
2. merge-only unique history (two branches both ancestors of base, then a second merge)
   -> `merge_only=True` -> PENDING_REVIEW / NOT_ACCEPTED until inspected.
3. staged add + unstaged modify + staged rename -> separate `staged.patch` and
   `unstaged.patch`, `meta.json` records observed HEAD + stagedPaths/unstagedPaths;
   staged content does not leak into unstaged and vice versa.
4. `tests/e2e/browser_credential_launch.rs` and `crates/x/src/credentials.rs`
   -> not sensitive (ordinary source); `.env`, `opencode.json`, `id_rsa`, `.pem`
   -> sensitive (denied).
5. A real disposable staged-only repository -> `stagedChanges=1`, `dirty=true`,
   staged content captured in `staged.patch` and path captured in `meta.json`.
6. Unique merge history whose non-merge patches match the base -> PENDING_REVIEW;
   patch equivalence does not discard an uninspected merge resolution.

## Round-3 independent RED and reconciliation

The integrator independently reproduced the staged-only defect against `81dadbb`:
`stagedChanges=1`, `trackedChanges=0`, `untrackedCount=0`, but `dirty=false`.
The predicate omitted the staged count. The previous explanation that rename
origin-token parsing caused the 18-to-16 discrepancy was incorrect and is
retracted. Regeneration against `0088fbf` finds exactly 18 dirty worktrees,
including these two staged-only worktrees omitted from the `-1600` snapshot:

- `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/authweb-prep`
- `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/stack-verify-retry`

Round 3 also bounds subprocess capture and joins the owned process on timeout or
overflow, checks tracked path sensitivity before materializing diff contents,
enforces the total patch-byte budget, and records unavailable stash/evidence
inventories explicitly.

Independent review then demonstrated three additional REDs: snapshot reuse
clobbered prior patches, unavailable status silently produced a clean result,
and an R100 rename from `.env` to `env.txt` hid the sensitive origin before an
unstaged diff was captured. The repair now refuses all existing snapshot roots,
records missing/unknown status as a gap, and inventories both rename sides using
`--no-renames` before reading content. Raw symlink targets are preserved in
`links.json` without dereferencing them. Regular-file reads use `O_NOFOLLOW`,
verify the opened file type, and enforce the actual byte budget.

Independent frozen regression:
`tests/bootstrap/test_convergence_v2_preservation.py`, SHA-256
`9295af1d780b8865aca44b5b59518b32c14b07904cb0626742dd14d8a898a08b`.
The same eight tests were RED against `81dadbb` (8 failures), then GREEN against
candidate source SHA-256
`6098baac892ece3fdc5c87b692acd28193f5fe52c598d122c9f1534b6d3822d5`.

## Key correctness changes
- `git_out`/`git_bytes` return `None` on timeout/nonzero; novelty/paths never cache
  failure as empty. Analysis failure -> explicit PENDING_REVIEW row, `analysisFailure=true`.
- SUPERSEDED_BY now means "all non-merge patches already in base by patch-id" and
  carries `acceptance=NOT_ACCEPTED` plus `supersededBy={ref,sha}` (exact base).
  Only exact ancestry yields `INTEGRATED_ANCESTOR` / `acceptance=INTEGRATED`.
- Dirty status parses `--porcelain=v1 -z` with nonzero-returncode check and rename
  source/destination handling (`parse_porcelain_z`).
- Preservation writes SEPARATE `staged.patch` / `unstaged.patch` / `untracked/` +
  `meta.json` (observed HEAD, byte sizes, path lists).
- Every skip (sensitive, over-budget, read/write error, ignored tree)
   recorded in `preservation.gaps`; never silently omitted.
- Sensitive matcher denies real stores (`.env`, `opencode.json`, `auth.json`,
  keychains, certs, tokens) but not ordinary `.rs/.py/.ts/...` source with a
  token in its filename.

## Structural coverage (schema v3, exact, reconciles)
- Frozen refs **659** = **331 unique tips + 328 aliases** (`refsAccounted=true`).
- By unique tip: INTEGRATED_ANCESTOR 41, SUPERSEDED_BY 4, PROCESS_ONLY 44,
  SALVAGE_PRODUCT 224, SALVAGE_TEST 17, PENDING_REVIEW 1.
- Validation: `analysisFailures=[]`, `mergeOnlyPending=[]`, `supersededNeedsBase=true`.
- All 659 frozen ref names retained as primary or alias (asserted by refsAccounted).
- Detached anchors **45/45**, all objectPresent=true (`detachedAllValid=true`).
- Worktrees **112/112** present and observed; **18 dirty**, including **2 staged-only**;
  statusError 0, statusTimeout 0.
- Stash entries 6 (`refs/stash=eda2b050…`, read-only, never popped); archive/ledger 6.

## Preservation snapshot (fresh, external, non-overwriting)
- Current dir: `/Users/mymac/Projects/opencode-rk-pre-v2-20260930-1130/v2-worktree-preservation-20260930-staged-complete`
- **18 dirty worktrees**, **3,379,672 payload bytes**, 18 observed-HEAD metadata
  records, separate staged/unstaged patches, and one raw dylib symlink record.
- Both staged-only patches were independently compared byte-for-byte with the
  live index diff. `authweb-prep`: 65,039 bytes, SHA-256
  `de8e822992b901d90cfde8c2d8e7c905071707dfc38837e148b767e9c82141c6`.
  `stack-verify-retry`: 45,564 bytes, SHA-256
  `eaf3824b2bce454c70183491177c8c12641511b8cd2619dfe7d4396a1a3a8613`.
- Historical round-2 snapshot: 16 dirty worktrees, 3,268,938 bytes, separate
  staged/unstaged/meta. This snapshot omitted the two staged-only worktrees above
  and is retained as evidence; it is not a complete preservation result.
- Previously-skipped large files now captured (budget raised to 4 MiB/file, 64 MiB/run):
  `docs/plans/phase1/audit/legacy-crosswalk.json` (337,312 B),
  `docs/plans/phase1/phase1-tasks.json` (419,154 B) -> under
  `db140e62150d/untracked/`; `off/.../dep-graph.part.bin` (524,520 B).
- The `-1400` and `-1600` snapshots remain preserved.

## Remaining gaps (G0 stays `pending` — honest)
1. `opencode.json` (root worktree `/Users/mymac/Projects/opencode-rk`) — DENIED as
   possible real API keys; left in place, reported as the sole current snapshot
   gap. Its name is present in the original frozen root-untracked inventory;
   no secret contents were read during this repair.
2. Full G0 content disposition remains pending; candidate rows are intake
   evidence, not product acceptance or permission for heritage merges.

## Verification commands / results
- `rtk /usr/bin/arch -arm64 /usr/bin/python3 tools/convergence_v2_salvage.py --self-check` -> `SELF-CHECK OK ...`
- `python3 tools/convergence_v2_salvage.py --base-sha fc2d201... --frozen-root <frozen> --preserve-dirty --snapshot-dir <new>` ->
  `{"base":"fc2d201...","frozenRefs":659,"uniqueTips":331,"validation":{...all true...},"worktrees":{...}}`
- reconciliation: `frozenRefs 659 = aliased 328 + unique 331`
- `find <snapshot> -name legacy-crosswalk.json -o -name phase1-tasks.json -o -name dep-graph.part.bin` -> 3 hits.
- `rtk /usr/bin/arch -arm64 /usr/bin/python3 tools/convergence_v2_salvage.py --base-sha 0088fbf2ea65251f344915c93bd8679c0c467320 --base-ref main-v2 --frozen-root /Users/mymac/Projects/opencode-rk-pre-v2-20260930-1130`
  -> refs 659 = 331 tips + 328 aliases; detached 45/45 valid; worktrees 112/112
  present; 18 dirty; 2 staged-only; zero analysis/status failures.
- The same inventory command with `--preserve-dirty --snapshot-dir <staged-complete>`
  -> 18 worktrees / 3,379,672 payload bytes / one sensitive gap / zero over-budget.
- `rtk env OC2_SALVAGE_TMP=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode /usr/bin/arch -arm64 /usr/bin/python3 -m unittest tests.bootstrap.test_convergence_v2_preservation`
  -> 8 tests GREEN (independent test owner).

## Status
PREVERIFIED on isolated branch `v2/salvage-preservation`. Acceptance requires the
integrator to integrate and re-run the gate on the exact integrated SHA. No history
merges performed. Parent should independently check coverage/diffs before integration.
