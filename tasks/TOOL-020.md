# TOOL-020

Status: NOT STARTED. Kind: product. Runtime optional: True.
Mandatory for full declared release: yes.
Requirements: REQ-039 (proposed, RTK failure-only test/build/lint filters).
Dependencies: none.
Test obligations: TOOL-020-T01, TOOL-020-T02, TOOL-020-T03, TOOL-020-T04, TOOL-020-T05.

## User-observable outcome

Native inbuilt Rust failure-only filters for `pytest`, `cargo test`,
`tsc`, `lint`: passing runs emit the verdict line only; failing runs
emit every failing test/rule plus trailing context (diff/snippet) plus
the verdict line. A filtered failing run never looks green. Filters are
ON by default with opt-out; zero cost when off (passthrough, input
returned verbatim).

## Source evidence

- PLAN.md sections 5-6: slice independence rules, mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: lifecycle order, compiling RED, frozen hash, GREEN minimum.
- tasks/TOOL-014.md: task-card model (Status/Kind/contract/test obligations).
- tasks/TOOL-021.md: sibling opt-out default-on card shape (Status NOT STARTED,
  Runtime optional True, REQ-039 proposed, deps none).
- docs/proposals/RTK-SLICE-02-test-spec.md fully: filter contract, failure
  states, bounds, RTK-TEST-T01..T05 definitions mirrored below as TOOL-020-T01..T05.
- Classification: new user requirement (REQ-039 proposed), deliberate
  resource-bounded deviation (filtering is presentational; raw log retained
  by caller, exit code never altered).

## Observable contract

- `filter_test_output(tool: TestTool, stdout: &str, exit: i32, opts: &FilterOpts) -> FilteredOutput`
  where `TestTool = Pytest | CargoTest | Tsc | Lint`.
- All-pass (exit 0): output is verdict line(s) only (e.g. `5 passed in 1.2s`,
  `test result: ok`, `0 errors`, `All checks passed`); passing per-test/dot
  noise removed.
- Failure (exit != 0): output contains every failing test/rule id + its
  trailing context block (assert diff / error snippet, up to `CTX_LINES`
  lines each), plus the original verdict/summary line(s) verbatim, plus exit code.
- Verdict detection per tool: pytest (`passed|failed|error` summary),
  cargo test (`test result: ...`), tsc (`error TS|Found N errors`),
  lint (`E.../W...` + `N problems`).
- `FilteredOutput { text: String, verdict: String, exit: i32, truncated: bool }`.
- Opt-out default-on: `no_filter=true` or env `RTK_NO_FILTER=1` returns input
  verbatim (single branch, no parse, no allocation beyond return).
- Over-budget: truncate middle, keep first failing block + verdict, append
  marker `\n... [rtk: truncated, full log in <path>]`.
- Suggested module boundary: `crates/rtk/src/test_filter.rs` owning
  `TestTool`, `FilterOpts`, `FilteredOutput`, `filter_test_output`;
  `lib.rs` only re-exports (integrator wires per PLAN.md section 5).

## Failure states

- Failed run never looks green: `exit != 0 => text` contains a failure marker
  (`FAILED|failed|error|problems`) AND the verdict line (issue-fix invariant).
- Exit != 0 but no failure block matched: emit full tail (last 50 lines) +
  verdict if found, else emit everything; never emit empty output on failure.
- Exit == 0 but output unparseable: emit verdict line if found, else single
  line `ok (exit 0)`; never fail the filter itself.
- Filter panic/OOM: fall back to raw tail + verdict; filter never changes exit code.
- `FilteredOutput.exit` always equals input exit; non-zero is never mapped to zero.

## Resource bounds

- Zero cost when off: single opt-out branch, no parse, no heap beyond return.
- Streaming line-based; retained output bounded by `MAX_KEPT_BYTES = 64 KiB`
  per run (configurable, default 64 KiB); input scan O(n) time, O(1) extra
  besides kept blocks; trailing context cap `CTX_LINES = 30` per failure block,
  max `MAX_BLOCKS = 20` blocks, rest counted and noted.
- No I/O, no clock, no network, no global state; pure function of inputs.

## Test obligations (frozen, mirror RTK-TEST-T01..T05)

- TOOL-020-T01 (all-pass verdict only, mirrors RTK-TEST-T01): given pytest
  `5 passed` log with 200 dot/per-test lines, assert output has `5 passed`
  and zero lines matching `PASSED|ok$`; assert `exit == 0`, `truncated == false`.
- TOOL-020-T02 (failure shows failing test + diff, mirrors RTK-TEST-T02):
  given pytest failure with `FAILED test_x` + `assert 1 == 2` diff, assert
  output contains `test_x` AND `assert 1 == 2`, and does NOT contain a passing
  test id from input.
- TOOL-020-T03 (exit code preserved, mirrors RTK-TEST-T03): for each tool
  fixture with exit 1/2/101, assert `FilteredOutput.exit == input exit`;
  assert filter returns Ok and never maps non-zero to zero.
- TOOL-020-T04 (verdict never filtered, mirrors RTK-TEST-T04): for each tool
  failing fixture, assert output contains the exact verdict line
  (`test result: FAILED`, `Found 3 errors`, `1 problem`); assert all-pass
  output contains verdict too; grep-based check on frozen fixtures.
- TOOL-020-T05 (over-budget truncates with marker, mirrors RTK-TEST-T05):
  given 500 KiB failure log, assert `text.len() <= MAX_KEPT_BYTES + 1 KiB`,
  `truncated == true`, text ends with `[rtk: truncated`, and still contains
  verdict line.

## TDD steps

1. Inspect: PLAN.md 5-6, TDD.md 2-5, TOOL-014.md, RTK-SLICE-02-test-spec.md
   (done, see evidence).
2. Contract: defined above.
3. Author tests TOOL-020-T01..T05; establish compiling RED (fail: no filter).
4. Freeze test hash + command manifest.
5. Implement minimum `filter_test_output` natively in Rust (verdict match +
   block capture + caps).
6. GREEN, refactor, rerun; negative tests (exit != 0 unknown format shows
   tail, never empty/green).
7. Evidence + patch; verifier decides acceptance.

## Verification

```bash
git status --short -- tasks/TOOL-020.md
cargo test -p <rtk-crate> --test test_filter
cargo check -p <rtk-crate>
```

(`<rtk-crate>` resolves at implement time; no `crates/rtk` exists yet;
suggested boundary is `crates/rtk/src/test_filter.rs` per RTK-SLICE-02.)
