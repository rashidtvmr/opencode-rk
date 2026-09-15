# TOOL-019

Status: NOT STARTED. Kind: product. Runtime optional: True.
Mandatory for full declared release: yes.
Requirements: REQ-039 (proposed, new user requirement).
Dependencies: none.
Test obligations: TOOL-019-T01, TOOL-019-T02, TOOL-019-T03, TOOL-019-T04, TOOL-019-T05.

## User-observable outcome

Native inbuilt RTK core filters for `git status/diff/log`, `ls/read/grep/find/diff`
as a pure Rust library. Default-on with opt-out; passthrough by default for
unknown/empty input; `--raw`-equivalent `raw()` escape hatch returns
byte-identical output. Zero cost when disabled. Replaces external-wrapper hot
path per PLAN.md ADR-001 (no OS process per subagent).

## Source evidence

- AGENTS.md agent-operating-rules RTK section (prefix every command with `rtk`; passthrough always safe).
- docs/TDD.md sections 2-5: contract, compiling RED, freeze hash, GREEN rules.
- PLAN.md sections 5-6: slice template + mandatory RED/GREEN lifecycle.
- PLAN.md section 3 ADR-001: one native domain runtime, no process per subagent.
- tasks/TOOL-014.md: task-card model (bounded OutputStore, byte budget, eviction precedent).
- tasks/TOOL-003.md: ToolExecutor/ToolResult pattern (output, success, error).
- docs/proposals/RTK-SLICE-01-core-spec.md: RTK-CORE-T01..T05 contract, Filtered type, TRUNC_MARKER, determinism.
- docs/proposals/RTK-RES-01-rtk-research.md: issues I-01..I-10 root causes and design fixes baked in below.

## Observable contract

- `RtkConfig { enabled: bool, max_bytes: usize }`: `Default` is
  `enabled: true, max_bytes: 65536`. `disabled()` returns passthrough config.
- `FilterKind`: `GitStatus | GitDiff | GitLog | Ls | Read | Grep | Find | Diff`.
- `Filtered { stdout: Vec<u8>, stderr: Vec<u8>, code: i32, truncated: bool }`.
- `filter(kind: FilterKind, stdout: &[u8], stderr: &[u8], code: i32, cfg: &RtkConfig) -> Filtered`.
- `raw(stdout: &[u8], stderr: &[u8], code: i32) -> Filtered` (escape hatch, byte-identical).
- Enabled order: strip ANSI CSI sequences first, then kind-specific shrink, then
  byte-budget truncate with marker. Disabled: alias to `raw`.
- `TRUNC_MARKER = "\n... [rtk:truncated {dropped} bytes, rerun with raw]"`.
- Deterministic: same input bytes + kind + cfg => byte-identical output;
  no wall-clock, no hashmap iteration, no network, no I/O.

## Failure states (GitHub-issue fixes baked in)

- Empty stdout/stderr: passthrough, `truncated: false`, no marker (T03).
- Non-UTF8 bytes: operate on bytes; truncation splits at `char_boundary`.
- Over budget: truncate stdout only, append marker, set `truncated: true`;
  stderr always preserved verbatim with its own marker, never dropped (fixes I-04 prettier stderr loss).
- Unknown kind / parse failure: passthrough stdout unchanged, `truncated: false`.
- Filter never alters `code`; never swallows stderr; never emits color codes
  (fixes I-01 `rtk err` exit 0, I-03 golangci-lint 1-to-0 remap, I-06 pnpm code loss;
  shared exit-propagation choke point, per-tool code tables forbidden in filter layer).
- ANSI CSI stripped before filter, never leaked (fixes I-09 grep color leak).
- grep/find exit codes byte-preserved (0 match, 1 no-match, 2 error); invalid
  regex / permission-denied map to error record with stderr attached, never
  conflated with no-match (fixes I-07, I-08).
- Byte budget enforced on line length as well as total (fixes I-10 5000-char
  single-line blowup); filter idempotent, second application is passthrough
  (fixes chained-`rtk` empty output).
- Spawn class eliminated structurally: pure library, no `Command` spawn, no shell
  forwarding, so I-02 proxy misparse and I-05 missing-binary silence cannot recur;
  spawn errors in caller synthesize `code=127` record with stderr diagnostic.
- Heuristics (`smart`/`summary`) excluded from hot path: deterministic filters
  only by default; any heuristic behind explicit flag, output labeled.

## Resource bounds

- Pure library: no `Command` spawn, no thread, no I/O, no alloc beyond
  `max_bytes + marker.len()`. Time O(n), memory O(max_bytes). ADR-001 compliant.
- Disabled path: zero copy beyond passthrough (`raw` borrows or single copy),
  negligible cost; no regex engine startup.
- No unbounded queue, no unbounded retained output; owner and cancel path defined
  per TOOL-014 precedent (TOOL-014 byte-budget eviction).

## Test obligations (mirror RTK-CORE-T01..T05, frozen asserts)

- TOOL-019-T01 filter shrinks: fixture `git-status-long` (>50 lines, with ANSI):
  `out.stdout.len() < input.len()`, `!contains(out.stdout, "\x1b[")`,
  `out.stderr == input_stderr`, `filter(..) == filter(..)` (determinism).
- TOOL-019-T02 exit code preserved: codes `[0, 1, 128]` x each `FilterKind`:
  `assert_eq!(out.code, code)`, `assert_eq!(out.stderr, input_stderr)`.
- TOOL-019-T03 empty input passthrough: `stdout=b""`, `stderr=b""`:
  `assert_eq!(out.stdout, b"")`, `assert!(!out.truncated)`, no marker bytes.
- TOOL-019-T04 over-budget truncation marker: `max_bytes=1024`, input 100 KiB:
  `assert!(out.truncated)`, `ends_with(out.stdout, marker_suffix)`,
  `out.stdout.len() <= 1024 + MARKER_MAX`, `filter(..) == filter(..)`.
- TOOL-019-T05 raw hatch byte-identical: each `FilterKind`, ANSI + large input:
  `assert_eq!(raw.stdout, input_stdout)`, `assert_eq!(raw.stderr, input_stderr)`,
  `assert_eq!(raw.code, code)` even where T01 shrinks.

## TDD steps (docs/TDD.md 2-5, PLAN.md 6)

1. Inspect PLAN.md 5-6, docs/TDD.md 2-5, TOOL-014 card; cite commit + path:line.
2. Define contract/failures/bounds (above); open discovery proposal for gaps.
3. Author RED: 5 tests compile, fail on missing `rtk_core::filter/raw`.
4. Freeze test hash + command manifest; controller verifies on disk.
5. Implement minimum native Rust; GREEN; refactor; rerun full suite.
6. Regressions: stderr-preserved, ANSI-first, invalid-UTF8, disabled zero-cost.
7. Submit evidence + patch, never acceptance.

## Verification

- `cargo test -p rtk-core` passes TOOL-019-T01..T05 on frozen hash.
- `cargo check --workspace` clean; `cargo fmt --check` clean.
- Disabled config: benchmark/micro-assert shows no shrink path taken.

## Remaining gaps / unknowns

None (research RTK-RES-01 complete; slice spec RTK-SLICE-01 proposed owner
`crates/rtk_core/src/lib.rs`, new crate `rtk-core`; integrator wires
registration, worker additive only).
