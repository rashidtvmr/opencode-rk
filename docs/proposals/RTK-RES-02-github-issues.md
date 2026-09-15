# RTK-RES-02: GitHub issue research on output-filter failure modes

Status: proposal research only. No product code.
Scope: output-filter/condenser defects relevant to inbuilt RTK (exit codes, stderr, pipes, ANSI, truncation, messaging).
Sources: fetched Bodies of rtk-ai/rtk issues below (full text via webfetch, Sep 2026); local probes on rtk 0.43.0. Rule per SECURITY.md section 6: never mask a failure to keep the loop moving; preserve reproduction, stop, report.

## Issue list (symptom + root cause + design fix)

### G-01 pytest filter drops duration line (#4071)
Symptom: `rtk pytest` prints `Pytest: 2 passed`, raw run prints `2 passed in 3.04s`. Agent cannot tell 0.2s from 10min; suite regressing 12s to 10min reads identical. Failing path also duration-less.
Cause: pytest filter builds count summary, discards `in Ns` group. Other filters keep it (gradle `BUILD SUCCESSFUL in 8s`, cargo `Finished ... in 45.23s`, vitest/ctest/dotnet duration groups). Oversight, not policy.
Fix: summary line keeps duration: `Pytest: 2 passed in 3.04s`. Parse `=+ X passed[, Y failed][, Z errors] in Ns +=` with optional duration; emit `in Ns` whenever captured.

### G-02 plain `rtk read` dies on non-UTF-8 file (#4065)
Symptom: `rtk read /tmp/nonutf8.log` exits 1, prints only `invalid utf-8 sequence ... from index 38`, no content. Same file with `--head-lines 3` is byte-exact. Hook rewrites `cat f` to plain read, so one Latin-1 byte yields empty result that reads as empty file.
Cause: `run()` takes byte path only when `byte_line_window()` returns Some (needs head/tail lines). Plain read falls to `String::from_utf8` and errors.
Fix: default path is lossy-tolerant: `from_utf8_lossy` + truncation marker, or route all reads through existing byte-exact path. Never exit 1 with zero bytes on undecodable input.

### G-03 spawn-failure error names nothing (#4056)
Symptom: `rtk pip list` on broken install prints `Failed to run pip list: Failed to execute command: No such file or directory (os error 2)`. Missing binary, broken shebang, permission problem all identical.
Cause: `src/core/stream.rs:739 capture_raw` has program name one line above, uses it for exit-code label only, not error path. Affects all 21 `exec_capture` callers. Streaming path already fixed (#2634); capture path missed.
Fix: spawn errors include resolved program path: `failed to spawn \`/usr/local/bin/pip\` (is it installed, or does its shebang point at a missing interpreter?): ...`. Synthesize record code=127 on stderr.

### G-04 `rtk ls` never compacts, ignores all flags (#4051)
Symptom: `rtk ls`, `rtk ls -1`, `rtk ls -la` byte-identical, plain `ls -la` normalized only (BSD `@` strip, ISO dates, recomputed total). Bare `ls` becomes 368B vs native 25B (14x). Regression: `rtk gain` shows ls savings 58-62% (Jun-Aug) collapsing to 0% (Sep 14) on same 0.42.4 binary.
Cause: ls handler forces `-la` shape regardless of flags; no column strip/compaction; regression trigger unidentified (config/HOME/locale/RTK_NO_TOML all ruled out).
Fix: honor flags (bare `ls` stays one-per-line names); compact `-la` (drop `.`/`..`, owner/group/timestamp columns or collapse); savings must never go negative vs native, else passthrough.

### G-05 `rtk rewrite` fails silently on `gh pr view --json` + pipe tail (#4067)
Symptom: `rtk rewrite 'gh pr view 123 --json reviews'` exits 1, empty stdout, no message. Hook passes raw command through, silently losing ~100K tokens/30d (largest missed-savings source per `rtk discover`). Secondary: `git log -1 | grep -n foo` rewrites first command only, pipe tail left raw.
Cause: rewrite grammar lacks `--json <fields>` variant for gh pr view; pipe splitter handles head only.
Fix: silent-no-rewrite is forbidden: on no-match emit `rtk <original>` passthrough with exit 0 plus marker, or error naming unmatched token. Grammar: accept `--json <csv>` on gh pr view; rewrite every pipe segment, not just head.

### G-06 broken-but-present `pip` blocks `uv` fallback (#4054)
Symptom: stale Homebrew pip (shebang `python3.7` deleted) makes `rtk pip list` fail, never tries `uv`; removing pip makes it work via uv.
Cause: `pip_cmd.rs:27` decides on `!tool_exists("pip") && tool_exists("uv")`; `tool_exists` checks PATH+executable bit only, not runnability. Do NOT always prefer uv (reverted per #3389, wrong env).
Fix: fallback on spawn ENOENT too: if `pip` spawn fails `No such file or directory`, retry via `uv pip` and label output with backend used. Existence check stays PATH-based; runnability decided at spawn.

### G-07 strict JSON schema breaks on upstream shape change (#4063)
Symptom: `rtk cc-economics` fails: `Invalid JSON structure for monthly data: missing field month at line 39 column 5` after ccusage 20.0.20 renamed/dropped `month`.
Cause: serde struct requires `month`; any upstream rename is a hard error killing whole report.
Fix: tolerant parse: `#[serde(default)]` + `Option` for volatile fields, `deny_unknown_fields` off; missing field degrades that row/column with marker, never aborts report. Pin/test against last-known schema in CI.

### G-08 meta commands burn the daily hook reminder (#4036)
Symptom: `rtk config` (also trust, hook-audit, discover, session, cc-economics) triggers `maybe_warn()`, touching `.hook_warn_last`; next `rtk ls` stays silent 24h. `init`/`verify`/`gain` already exempt via hand-maintained `matches!`.
Cause: reminder eligibility is a second hand-list beside `is_operational_command`; every new meta command reopens the bug (PRs #3582, #3614 each add one more).
Fix: single source of truth: derive reminder eligibility from `is_operational_command` classification (meta = never warn, never touch marker). No second list.

### L-01 ANSI CSI leaks through `rtk grep` (local probe, 0.43.0)
Symptom: `printf '\033[31mred\033[0m\n' | rtk grep red | cat -v` retains `^[[31mred^[[0m`.
Cause: no CSI strip before filter; color bytes consume the budget the filter claims to save.
Fix: strip ANSI CSI first, then shrink. Exact command + output above; reproduce with `cat -v`.

### L-02 `rtk pipe --filter err` unknown-filter exits 1 empty (local probe, 0.43.0)
Symptom: `printf 'line1\nFAIL boom\nline3\n' | rtk pipe --filter err` prints `Unknown filter 'err'. Available: cargo-test, pytest, go-test, go-build, tsc, vitest, grep, rg, find, fd, git-log, git-diff, git-status, log, mypy, ruff-check, ruff-format, prettier`, exit 1.
Cause: pipe filter registry lacks generic err/test/summary filters; failure is correct (exit 1) but discoverability poor.
Fix: inbuilt RTK documents exact filter-kind enum up front; unknown kind lists available kinds (already does) and never silently passes raw as filtered.

## Cross-cutting rules for inbuilt RTK
1. Exit codes: filter never remaps (G-05 silent 1 must become loud; L-02 keeps 1 with message). Note: local `rtk err ls /nonexistent` on 0.43.0 already preserves exit 2, RES-01 I-01 class appears fixed here, keep regression test.
2. Both streams captured; stderr verbatim with own budget marker, never dropped.
3. Byte budget at char boundary + `TRUNC_MARKER`; filters idempotent so chained segments passthrough.
4. Errors name the program + resolved path + hint (G-03/G-06); spawn ENOENT suggests fallback actually tried.
5. Tolerant external schemas; strictness never aborts a whole report (G-07).
6. Meta/operational split single-sourced (G-08); duration/count lines preserved (G-01); flags honored, savings never negative (G-04); undecodable bytes degrade, never blank (G-02).

## Sources / honesty
- Fetched full bodies: https://github.com/rtk-ai/rtk/issues/4071, .../4065, .../4056, .../4051, .../4067, .../4054, .../4063, .../4036, .../4050 (last supports G-03/G-06 messaging only). Listing page: https://github.com/rtk-ai/rtk/issues.
- websearch backend returned zero results for all queries; no non-rtk comparator issues citable. Local probes use rtk 0.43.0 (`rtk --version` verified). No URLs or numbers invented.
