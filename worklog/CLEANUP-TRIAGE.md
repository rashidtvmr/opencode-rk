# CLEANUP-TRIAGE (disposables only)

Scope: delete ONLY category (d) confirmed-unreferenced repo probe files. No lane src/test/worklog touched. No ralph.json/FEATURES.md/lib.rs edits.

## Before
- `git status --porcelain | wc -l` = 363
- modified (`^ M|^M`) = 204
- untracked (`??`) = 159
- `git diff --check` = clean (exit 0)

## Untracked classification (159 total)
- (a) owned test files `crates/*/tests/*` — 28 — KEEP
- (b) owned src impl `crates/*/src/*` — 12 — KEEP
- (c) worklogs `worklog/*` — 119 — KEEP
- (d) disposable probes `zz_probe*` — 0 in repo
- (e) `target/` artifacts — none listed in `git status --porcelain` (gitignored); left alone

Nonstandard untracked outside a/b/c: none (`grep -v` returned empty).

## Category (d) evidence
- `grep -rn 'zz_probe' crates/ tools/` → no hits (empty output).
- `git status --porcelain | grep -i 'zz_'` → NONE-zz-in-status.
- Root `ls zz_*` / `find . -maxdepth 2 -name 'zz_*'` (excl. target/.git) → no repo files.
- `/tmp` probes need no repo cleanup per task.

## After
- Deletions: none (zero confirmed-unreferenced repo files existed).
- `git status --porcelain | wc -l` = 363 (unchanged); untracked = 159 (unchanged).
- `timeout 120 rtk git diff --check` → exit 0, no output.

## Kept summary
- All 28 test + 12 src + 119 worklog untracked files kept as lane-owned.
- No action taken beyond this triage record.
