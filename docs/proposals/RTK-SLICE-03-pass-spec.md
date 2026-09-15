# RTK-SLICE-03-pass -- Analysis/infra/package filters + global pass/chain/proxy/opt-out contract

Status: PROPOSED. Kind: product. Runtime optional: False.
Mandatory for full declared release: no (lean-tooling proposal).
Requirements: none (new user requirement; REQ-032 precedent: no hidden cost when off).
Dependencies: none (pure output transform; composes with SLICE-01 core plumbing if landed).
Test obligations: RTK-PASS-T01, RTK-PASS-T02, RTK-PASS-T03, RTK-PASS-T04, RTK-PASS-T05.

## User-observable outcome

Native failure-only/shrink filters for analysis (`err log json summary deps env`),
infra (`gh pr view/run list/issue list docker ps/kubectl get/logs`), packages
(`pip list pnpm install npm run <script>`); unknown commands pass through
byte-identical. `&&`-chained segments filter independently. `rtk proxy <cmd>`
runs unfiltered but counts bytes. Global + per-command opt-out kills filtering
with zero overhead.

## Source evidence

- Commit `a453067`, PLAN.md sections 5-6: slice template + RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: RED compiles+fails, freeze hash, GREEN minimum.
- tasks/TOOL-014.md: task-card model (bounded store, byte budget, T01..T05 shape).
- AGENTS.md lines 87-90: `rtk` prefix rule, per-segment chaining, bare raw form.
- tasks/BASE-006.md opt-out precedent: `Features::default()` all-off, gate
  before subsystem init, `Feature::ALL` exhaustive default-off test.

## Observable contract

- `filter_pass(kind: PassKind, stdout: &[u8], stderr: &[u8], code: i32, cfg: &RtkConfig) -> Filtered`
  where `PassKind = Err | Log | Json | Summary | Deps | Env | Gh | Docker | Kubectl | Pip | Pnpm | Npm | Unknown`.
- Known kinds: failures-only (keep verdict/error lines + trailing ctx, drop noise);
  unknown/empty: byte-identical stdout+stderr, `truncated: false`, code kept.
- Chain: split `&&` (respecting quotes) at dispatch; each segment filters by its
  own kind; one segment's failure never rewrites siblings; exit = last nonzero.
- `proxy`: output byte-identical AND `accounted_bytes += stdout.len()+stderr.len()`;
  no shrink, no truncate, no marker even over budget.
- Opt-out: global `disabled()` / env `RTK_NO_FILTER=1` forces `raw` for every
  kind (composes: global OR per-command `no_filter` wins; no AND loophole).
  Disabled = alias to `raw`; zero regex/parse work.

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

## Suggested module boundary

- Proposed owner: `crates/rtk/src/pass_filter.rs` (new file, pure fn, no I/O).
- Entry: `pub fn filter_pass(kind, stdout, stderr, code, cfg) -> Filtered`.
- Tests: `crates/rtk/tests/pass_filter.rs` (frozen T01..T05 below).
- Integrator wires registration; worker never edits shared `lib.rs`/schemas.

## Frozen test obligations (exact asserts)

- RTK-PASS-T01 unknown passthrough identical: `kind=Unknown`, ANSI+100 KiB in:
  `assert_eq!(out.stdout, input)`, `assert_eq!(out.stderr, input_stderr)`,
  `assert_eq!(out.code, code)`, `assert!(!out.truncated)`.
- RTK-PASS-T02 chain segments independent: `a-ok && b-fail && c-unknown`:
  seg A shrunk (`len < in`), seg B keeps fail id+verdict, seg C identical;
  `assert_eq!(exit, b_code)`; siblings untouched by B's failure.
- RTK-PASS-T03 proxy accounts without filtering: over-budget fail log via proxy:
  `assert_eq!(out.stdout, input)`, `assert!(!out.truncated)`, no marker,
  `assert_eq!(accounted, in_stdout.len()+in_stderr.len())`.
- RTK-PASS-T04 opt-out zero overhead: global `disabled()` + per-cmd `no_filter`
  x each known kind: `assert_eq!(out.stdout, input)`; shrink-fn call-count `== 0`;
  global OR per-cmd each suffices (matrix: global on/off x per-cmd on/off).
- RTK-PASS-T05 budget marker: known kind, 500 KiB fail log, `max_bytes=65536`:
  `assert!(out.truncated)`, `out.stdout.len() <= 65536 + MARKER_MAX`,
  `ends_with(marker)`, output still contains verdict line.

## TDD steps

1. Cite commit + path:line evidence; define contract/failures/bounds (above).
2. Author RED: 5 tests compile, fail on missing `filter_pass`/proxy/accounting.
3. Freeze test hash + manifest (`cargo test -p rtk --test pass_filter`).
4. Implement minimum native Rust; GREEN; refactor; rerun full `cargo test -p rtk`.
5. Negatives: unknown-with-nonzero-exit keeps all bytes; proxy over-budget raw;
  opt-out composes OR; disabled path call-count zero.
6. Submit evidence + patch, never acceptance.

## Verification

- `cargo test -p rtk --test pass_filter` passes RTK-PASS-T01..T05 frozen hash.
- `cargo clippy -p rtk -- -D warnings && cargo fmt --check` clean.
- `git status --short` shows only this spec file (spec lane, no product code).
