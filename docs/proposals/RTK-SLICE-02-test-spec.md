# RTK-SLICE-02-test -- Failure-only filters for test/build/lint

Status: PROPOSED. Kind: product. Runtime optional: False.
Mandatory for full declared release: no (dev-tool quality slice, default-on opt-out).
Requirements: none (RTK inbuilt filter behavior).
Dependencies: none (pure output transform; needs RTK-SLICE-01 core filter plumbing if landed).
Test obligations: RTK-TEST-T01, RTK-TEST-T02, RTK-TEST-T03, RTK-TEST-T04, RTK-TEST-T05.

## User-observable outcome

Native inbuilt Rust failure-only filters for `pytest`, `cargo test`,
`tsc`, `lint`, `prettier --check`, `mypy`, `ruff check`, `cargo build`:
passing runs emit the verdict line only; failing runs emit the failing
test/rule plus trailing context (diff/snippet) plus the verdict line.
A filtered run never looks green when the command failed. Filters are
ON by default; `--no-filter` / `RTK_NO_FILTER=1` opts out to raw output.

## Source evidence

- PLAN.md sections 5-6: slice template + mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: RED must compile and fail for missing behavior; freeze hash; GREEN minimum.
- tasks/TOOL-014.md: task-card shape modeled here (Status/Kind/contract/boundary/tests).

## Observable contract

- `filter_test_output(tool: TestTool, stdout: &str, exit: i32) -> FilteredOutput`
  where `TestTool = Pytest | CargoTest | Tsc | Lint | Prettier | Mypy | Ruff | CargoBuild`.
- All-pass (exit 0): output is verdict line(s) only (e.g. `5 passed in 1.2s`,
  `0 errors`, `All checks passed`), passing per-test/dot noise removed.
- Failure (exit != 0): output contains every failing test/rule id + its
  trailing context block (assert diff / error snippet, up to N lines),
  plus the original verdict/summary line(s) verbatim, plus exit code.
- Verdict detection per tool: pytest (`passed|failed|error` summary),
  cargo test (`test result: ...`), tsc/mypy (`error TS|Found N errors`),
  ruff/eslint (`E.../W...` + `N problems`), prettier (`would reformat`),
  cargo build (`error[`, `warning: unused` kept only on failure).
- `FilteredOutput { text: String, verdict: String, exit: i32, truncated: bool }`.
- Opt-out: `no_filter=true` or env `RTK_NO_FILTER=1` returns input verbatim.
- Over-budget: truncate middle, keep first failing block + verdict, append
  marker `\n... [rtk: truncated, full log in <path>]`.

## Failure states

- Exit != 0 but no failure block matched: emit full tail (last 50 lines) +
  verdict if found, else emit everything; never emit empty output on failure.
- Exit == 0 but output unparseable: emit verdict line if found, else single
  line `ok (exit 0)`; never fail the filter itself.
- Filter panic/OOM: fall back to raw tail + verdict; filter never changes exit code.
- Issue-fix invariant: `exit != 0 => text` contains a failure marker
  (`FAILED|failed|error|would reformat|problems`) AND the verdict line.

## Resource bounds

- Streaming line-based; retained output bounded by `MAX_KEPT_BYTES = 64 KiB`
  per run (configurable, default 64 KiB); input scan is O(n) time, O(1) extra
  besides kept blocks; trailing context cap `CTX_LINES = 30` per failure block,
  max `MAX_BLOCKS = 20` blocks, rest counted and noted.

## Suggested module boundary

- Proposed owner: `crates/rtk/src/test_filter.rs` (new file, pure fn, no I/O).
- Entry: `pub fn filter_test_output(tool, stdout: &str, exit: i32, opts: &FilterOpts) -> FilteredOutput`.
- Tests: `crates/rtk/tests/test_filter.rs` (frozen T01..T05 below).

## Frozen tests (must RED before implement, frozen hash after)

- RTK-TEST-T01 all-pass verdict only: given pytest `5 passed` log with 200
  dot/per-test lines, assert output has `5 passed` and zero lines matching
  `PASSED|ok$`; assert `exit == 0`, `truncated == false`.
- RTK-TEST-T02 failure shows failing test + diff: given pytest failure with
  `FAILED test_x` + `assert 1 == 2` diff, assert output contains `test_x`
  AND `assert 1 == 2`, and does NOT contain a passing test id from input.
- RTK-TEST-T03 exit code preserved: for each tool fixture with exit 1/2/101,
  assert `FilteredOutput.exit == input exit`; assert filter returns Ok and
  never maps non-zero to zero.
- RTK-TEST-T04 verdict never filtered: for each tool failing fixture, assert
  output contains the exact verdict line (`test result: FAILED`,
  `Found 3 errors`, `would reformat`, `1 problem`); assert all-pass output
  contains verdict too; grep-based check on frozen fixtures.
- RTK-TEST-T05 over-budget truncates with marker: given 500 KiB failure log,
  assert `text.len() <= MAX_KEPT_BYTES + 1 KiB`, `truncated == true`, text
  ends with `[rtk: truncated`, and still contains verdict line.

## TDD steps

1. Add fixture logs + `tests/test_filter.rs` T01..T05; run, observe compiling RED.
2. Freeze test hash + command manifest (`cargo test -p rtk --test test_filter`).
3. Implement `test_filter.rs` minimum (verdict regexes + block capture + caps).
4. GREEN, refactor, rerun full `cargo test -p rtk`.
5. Negative: exit!=0 with unknown format still shows tail, never empty/green.

## Verification

- `cargo test -p rtk --test test_filter` (frozen T01..T05 GREEN).
- `cargo clippy -p rtk -- -D warnings && cargo fmt --check`.
- `git status --short` shows only this spec file (spec lane, no product code).
