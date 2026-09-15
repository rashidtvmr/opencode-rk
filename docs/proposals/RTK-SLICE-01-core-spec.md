# RTK-SLICE-01 - Native inbuilt RTK core filters

Status: PROPOSED. Kind: product. Runtime optional: False.
Mandatory for full declared release: no (lean-tooling proposal).
Requirements: none (new user requirement, GitHub-issue fixes baked in).
Dependencies: none.
Test obligations: RTK-CORE-T01, RTK-CORE-T02, RTK-CORE-T03, RTK-CORE-T04, RTK-CORE-T05.

## User-observable outcome

Native inbuilt output filters for `git status/diff/log`, `ls/read/grep/find/diff`
as a Rust library. Default-on with opt-out; zero cost when off. Passthrough by
default for unknown/empty input; `--raw`-equivalent escape hatch returns
byte-identical output. Fixes baked in: exit-code preservation, stderr never
swallowed, ANSI color-code stripping before filter, byte-budget truncation
markers, deterministic output.

## Source evidence

- PLAN.md section 3 ADR-001: one native domain runtime, no process per subagent.
- PLAN.md sections 5-6: slice template + mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: contract, RED, freeze, GREEN rules.
- tasks/TOOL-014.md: task-card model (bounded OutputStore, byte budget).
- tasks/TOOL-003.md: ToolExecutor/ToolResult pattern (output, success, error).
- Current RTK: external wrapper process; this slice replaces hot path with
  inbuilt library per ADR-001.

## Observable contract

- `RtkConfig { enabled: bool, max_bytes: usize }`: `Default` is
  `enabled: true, max_bytes: 65536`. `disabled()` returns passthrough config.
- `FilterKind`: `GitStatus | GitDiff | GitLog | Ls | Read | Grep | Find | Diff`.
- `Filtered { stdout: Vec<u8>, stderr: Vec<u8>, code: i32, truncated: bool }`.
- `filter(kind: FilterKind, stdout: &[u8], stderr: &[u8], code: i32, cfg: &RtkConfig) -> Filtered`.
- `raw(stdout: &[u8], stderr: &[u8], code: i32) -> Filtered` (escape hatch).
- Enabled: strip ANSI CSI sequences first, then kind-specific shrink, then
  byte-budget truncate with marker. Disabled: alias to `raw`.
- `TRUNC_MARKER = "\n... [rtk:truncated {dropped} bytes, rerun with raw]"`.
- Deterministic: same input bytes + kind + cfg => byte-identical output;
  no wall-clock, no hashmap iteration, no network, no I/O.

## Failure states

- Empty stdout/stderr: passthrough, `truncated: false`, no marker.
- Non-UTF8 bytes: operate on bytes; truncation splits at `char_boundary`.
- Over budget: truncate stdout only, append marker, set `truncated: true`;
  stderr always preserved verbatim (budget applies with its own marker, never drop).
- Unknown kind/parse failure: passthrough stdout unchanged, `truncated: false`.
- Filter never alters `code`; never swallows stderr; never emits color codes.

## Resource bounds

- Pure library: no `Command` spawn, no thread, no I/O, no alloc beyond
  `max_bytes + marker.len()`. Time O(n), memory O(max_bytes).
- Disabled path: zero copy beyond passthrough (`raw` borrows or single copy),
  negligible cost; no regex engine startup.

## Suggested module boundary

- Proposed owner: `crates/rtk_core/src/lib.rs` (new crate `rtk-core`).
- Integrator wires `pub use` / registration fragment; worker does not edit
  shared `lib.rs`, `Cargo.toml`, schemas. Additive only.

## Frozen test obligations (exact asserts)

- RTK-CORE-T01 filter shrinks: fixture `git-status-long` (>50 lines, with ANSI):
  `out.stdout.len() < input.len()`, `!contains(out.stdout, "\x1b[")`,
  `out.stderr == input_stderr`, `filter(..) == filter(..)` (determinism).
- RTK-CORE-T02 exit code preserved: codes `[0, 1, 128]` x each `FilterKind`:
  `assert_eq!(out.code, code)`, `assert_eq!(out.stderr, input_stderr)`.
- RTK-CORE-T03 empty input passthrough: `stdout=b""`, `stderr=b""`:
  `assert_eq!(out.stdout, b"")`, `assert!(!out.truncated)`, no marker bytes.
- RTK-CORE-T04 over-budget truncation marker: `max_bytes=1024`, input 100 KiB:
  `assert!(out.truncated)`, `ends_with(out.stdout, marker_suffix)`,
  `out.stdout.len() <= 1024 + MARKER_MAX`, `filter(..) == filter(..)`.
- RTK-CORE-T05 raw hatch byte-identical: each `FilterKind`, ANSI + large input:
  `assert_eq!(raw.stdout, input_stdout)`, `assert_eq!(raw.stderr, input_stderr)`,
  `assert_eq!(raw.code, code)` even where T01 shrinks.

## TDD steps

1. Inspect PLAN.md 5-6, docs/TDD.md 2-5, TOOL-014 card; cite commit + path:line.
2. Define contract/failures/bounds (above); open discovery proposal for gaps.
3. Author RED: 5 tests compile, fail on missing `rtk_core::filter/raw`.
4. Freeze test hash + command manifest; controller verifies on disk.
5. Implement minimum native Rust; GREEN; refactor; rerun full suite.
6. Regressions: stderr-preserved, ANSI-first, invalid-UTF8, disabled zero-cost.
7. Submit evidence + patch, never acceptance.

## Verification

- `cargo test -p rtk-core` passes RTK-CORE-T01..T05 on frozen hash.
- `cargo check --workspace` clean; `cargo fmt --check` clean.
- Disabled config: benchmark/micro-assert shows no shrink path taken.
