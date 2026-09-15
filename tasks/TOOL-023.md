# TOOL-023

Status: NOT STARTED. Kind: product. Runtime optional: True.
Mandatory for full declared release: yes.
Requirements: REQ-039 (proposed, RTK analysis/infra/package failure-only filters + global pass/chain/proxy/opt-out contract).
Dependencies: none.
Test obligations: TOOL-023-T01, TOOL-023-T02, TOOL-023-T03, TOOL-023-T04, TOOL-023-T05.

## User-observable outcome

Native failure-only/shrink filters for analysis (`err log json summary deps env`),
infra (`gh pr view / run list / issue list / docker ps / kubectl get / logs`),
packages (`pip list / pnpm install / npm run <script>`); unknown commands pass
through byte-identical. `&&`-chained segments filter independently. `rtk proxy <cmd>`
runs unfiltered but counts bytes. Global + per-command opt-out kills filtering
with zero overhead.

## Source evidence

- PLAN.md sections 5-6: slice independence rules, mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: lifecycle order, compiling RED, frozen hash, GREEN minimum.
- tasks/TOOL-014.md: task-card model (Status/Kind/contract/test obligations).
- tasks/BASE-006.md: opt-out precedent (`Features::default()` all-off, gate before
  subsystem init, `Feature::ALL` exhaustive default-off test).
- docs/proposals/RTK-SLICE-03-pass-spec.md: pass contract, bounds,
  RTK-PASS-T01..T05 definitions mirrored below as TOOL-023-T01..T05.
- AGENTS.md lines 87-90: `rtk` prefix rule, per-segment `&&` chaining, bare raw form.
- Classification: new user requirement (REQ-039 proposed), deliberate
  resource-bounded deviation (output shrink is presentational; exit code, stderr
  and accounted bytes preserved by caller).

## Observable contract

- `filter_pass(kind: PassKind, stdout: &[u8], stderr: &[u8], code: i32, cfg: &RtkConfig) -> Filtered`
  where `PassKind = Err | Log | Json | Summary | Deps | Env | Gh | Docker | Kubectl | Pip | Pnpm | Npm | Unknown`.
- Known kinds: failures-only (keep verdict/error lines + trailing context, drop noise).
- Unknown/empty: byte-identical stdout+stderr, `truncated: false`, code kept.
- Chain: split `&&` (respecting quotes) at dispatch; each segment filters by its
  own kind; one segment's failure never rewrites siblings; exit = last nonzero.
- `proxy`: output byte-identical AND `accounted_bytes += stdout.len() + stderr.len()`;
  no shrink, no truncate, no marker even over budget.
- Opt-out: global `disabled()` / env `RTK_NO_FILTER=1` forces `raw` for every
  kind (composes: global OR per-command `no_filter` wins; no AND loophole).
  Disabled = alias to `raw`; zero regex/parse work.
- Suggested module boundary: `crates/rtk/src/pass_filter.rs` owning
  `PassKind`, `RtkConfig`, `Filtered`, `filter_pass`; `lib.rs` only re-exports.
  Integrator wires registration; worker never edits shared schemas.

## Failure states

- Unknown command: passthrough unchanged, code+stderr kept, never empty output.
- Empty stdout+stderr: passthrough, `truncated: false`, no marker.
- Non-UTF8: byte ops, split trunc at `char_boundary`; stderr never dropped.
- Over budget: truncate stdout middle, keep first error block + verdict, append
  `TRUNC_MARKER`; proxy path never truncates, only accounts.
- Filter never alters `code`; never swallows stderr; never emits ANSI.

## Resource bounds

- Pure fn: no spawn/thread/I-O; time O(n), retained `<= max_bytes + marker`;
  `max_bytes` default 65536 (mirrors TOOL-014 OutputStore budget).
- Disabled/proxy: O(1) extra beyond single copy for accounting; no regex engine
  startup; micro-assert proves shrink path not taken.
- No I/O, no clock, no network, no global state.

## Test obligations (frozen)

- TOOL-023-T01 (unknown passthrough identical, mirrors RTK-PASS-T01):
  `kind=Unknown`, ANSI + 100 KiB in: `assert_eq!(out.stdout, input)`,
  `assert_eq!(out.stderr, input_stderr)`, `assert_eq!(out.code, code)`,
  `assert!(!out.truncated)`.
- TOOL-023-T02 (chain segments independent, mirrors RTK-PASS-T02):
  `a-ok && b-fail && c-unknown`: seg A shrunk (`len < in`), seg B keeps fail
  id + verdict, seg C identical; `assert_eq!(exit, b_code)`; siblings untouched
  by B's failure.
- TOOL-023-T03 (proxy accounts without filtering, mirrors RTK-PASS-T03):
  over-budget fail log via proxy: `assert_eq!(out.stdout, input)`,
  `assert!(!out.truncated)`, no marker,
  `assert_eq!(accounted, in_stdout.len() + in_stderr.len())`.
- TOOL-023-T04 (opt-out zero overhead, mirrors RTK-PASS-T04): global `disabled()`
  + per-cmd `no_filter` x each known kind: `assert_eq!(out.stdout, input)`;
  shrink-fn call-count `== 0`; global OR per-cmd each suffices
  (matrix: global on/off x per-cmd on/off).
- TOOL-023-T05 (budget marker, mirrors RTK-PASS-T05): known kind, 500 KiB fail
  log, `max_bytes=65536`: `assert!(out.truncated)`,
  `out.stdout.len() <= 65536 + MARKER_MAX`, `ends_with(marker)`, output still
  contains verdict line.

## TDD steps

1. Inspect: PLAN.md 5-6, TDD.md 2-5, TOOL-014.md, BASE-006.md,
   RTK-SLICE-03-pass-spec.md (done, see evidence).
2. Contract: defined above.
3. Author tests TOOL-023-T01..T05; establish compiling RED (fail: no `filter_pass`).
4. Freeze test hash + command manifest.
5. Implement minimum `filter_pass` natively in Rust.
6. GREEN, refactor, rerun; negatives (unknown-with-nonzero-exit keeps all bytes;
   proxy over-budget raw; opt-out composes OR; disabled path call-count zero).
7. Evidence + patch; verifier decides acceptance.

## Verification

```bash
git status --short -- tasks/TOOL-023.md
cargo test -p rtk --test pass_filter
cargo clippy -p rtk -- -D warnings && cargo fmt --check
```
