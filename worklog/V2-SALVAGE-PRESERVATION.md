# V2-SALVAGE-PRESERVATION worklog (round 2 — fail-closed repair)

## Package / gate
- Package: V2-SALVAGE-PRESERVATION (bounded preservation inventory repair, fail-closed)
- Gate: G0 pre-V2 preservation inventory correctness
- Base context SHA: `fc2d201f450b6020b1dce12268a550053d65235d` (explicit; not stale acceptance)
- Branch: `v2/salvage-preservation`
- Pre-fix candidate reviewed: `171bbb7` (independently reviewed, not integrated)
- Status: PREVERIFIED on isolated branch; NOT ACCEPTED on any integrated SHA.

## Changed paths (owned only, 4 files)
- `tools/convergence_v2_salvage.py` (schemaVersion 3)
- `docs/convergence-v2-salvage.json` (regenerated)
- `docs/CONVERGENCE_V2_SALVAGE.md` (regenerated)
- `worklog/V2-SALVAGE-PRESERVATION.md` (this file)

No source/product/tests/legacy-ledger edits. `main-v2` untouched.

## RED evidence against pre-fix `171bbb7` (fail-open proven before code)
Frozen copy sha256 `dce663726053ed9b004dd2c566ad42e8dcd1b20e3ad11b04de3fa2a9556c697`.
```
RED classify_empty     -> ('SUPERSEDED_BY', 'INTEGRATED')   # empty => fail-open heritage
RED novelty_gitfail    -> ([], [], [])                       # failure cached as "no novel"
RED sensitive_rs       -> True                               # .rs source false-positive
RED sensitive_env      -> True
```

## Fixed behaviour (GREEN self-check)
`python3 tools/convergence_v2_salvage.py --self-check` ->
`SELF-CHECK OK: fail-closed=PENDING merge-only=PENDING staged/unstaged/rename=separate safe-source=ok secret-deny=ok`

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
- Every skip (symlink, sensitive, over-budget, read/write error, ignored tree)
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
- Worktrees **112/112** observed; statusError 0, statusTimeout 0.
- Stash entries 6 (`refs/stash=eda2b050…`, read-only, never popped); archive/ledger 6.

## Preservation snapshot (fresh, external, non-overwriting)
- Dir: `/Users/mymac/Projects/opencode-rk-pre-v2-20260930-1130/v2-worktree-preservation-20260930-1600`
- 16 dirty worktrees, 3,268,938 bytes, separate staged/unstaged/meta.
- Previously-skipped large files now captured (budget raised to 4 MiB/file, 64 MiB/run):
  `docs/plans/phase1/audit/legacy-crosswalk.json` (337,312 B),
  `docs/plans/phase1/phase1-tasks.json` (419,154 B) -> under
  `db140e62150d/untracked/`; `off/.../dep-graph.part.bin` (524,520 B).
- The `-1400` copy was NOT overwritten.

## Remaining gaps (G0 stays `pending` — honest)
1. `opencode.json` (root worktree `/Users/mymac/Projects/opencode-rk`) — DENIED as
   possible real API keys; left in place, reported as a gap, not copied.
2. `crates/opentui-bridge/native/lib/aarch64-apple-darwin/libopentui.dylib`
   (worktree opencode-rk-web006-integrate) — symlink skipped, not dereferenced.
3. Dirty-count reconciliation: this strict v3 parser reports **16** dirty worktrees
   vs the earlier v2 run's **18**. v3 counts only tracked/untracked/rename entries;
   v2's naive `\x00` split also counted rename *origin* tokens, which can inflate.
   Flagged for parent reconciliation; v3 is authoritative until proven otherwise.
4. Not integrated / not ACCEPTED: all rows remain intake candidates.

## Verification commands / results
- `python3 tools/convergence_v2_salvage.py --self-check` -> `SELF-CHECK OK ...`
- `python3 tools/convergence_v2_salvage.py --base-sha fc2d201... --frozen-root <frozen> --preserve-dirty --snapshot-dir <new>` ->
  `{"base":"fc2d201...","frozenRefs":659,"uniqueTips":331,"validation":{...all true...},"worktrees":{...}}`
- reconciliation: `frozenRefs 659 = aliased 328 + unique 331`
- `find <snapshot> -name legacy-crosswalk.json -o -name phase1-tasks.json -o -name dep-graph.part.bin` -> 3 hits.

## Status
PREVERIFIED on isolated branch `v2/salvage-preservation`. Acceptance requires the
integrator to integrate and re-run the gate on the exact integrated SHA. No history
merges performed. Parent should independently check coverage/diffs before integration.