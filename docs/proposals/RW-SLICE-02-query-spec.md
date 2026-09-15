# RW-SLICE-02 - Native codebase query (keyword search over local index)

Status: PROPOSED. Kind: product. Runtime optional: False.
Mandatory for full declared release: no (lean-tooling proposal).
Requirements: none (new user requirement, local-first repowise complement).
Dependencies: none.
Test obligations: RW-QRY-T01, RW-QRY-T02, RW-QRY-T03, RW-QRY-T04, RW-QRY-T05.

## User-observable outcome

Keyword search over a caller-supplied local index. No network, no disk scan,
no subprocess. Returns ranked hits with `file:line` citations for every hit.
Default-on with opt-out; zero hidden cost when off (no index traversal).

## Source evidence

- PLAN.md sections 5-6: slice template + mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: contract, RED, freeze, GREEN rules.
- tasks/TOOL-014.md: task-card model (bounded OutputStore, byte budget).
- tasks/TOOL-009.md: ToolSearch just-in-time discovery precedent.
- docs/proposals/RTK-SLICE-01-core-spec.md: sibling pure-library pattern.

## Observable contract

- `QueryConfig { enabled: bool, max_hits: usize, max_bytes: usize }`:
  `Default` is `enabled: true, max_hits: 50, max_bytes: 65536`.
  `disabled()` returns opt-out config; `query` then returns empty, untruncated.
- `Hit { path: String, line: u32, snippet: String }`: `line` is 1-based;
  every hit carries a `file:line` citation (`path` + `line` both non-empty/nonzero).
- `IndexedDoc { path: String, line: u32, text: String }`: caller-built local index.
- `Result_ { hits: Vec<Hit>, truncated: bool }`.
- `query(index: &[IndexedDoc], q: &str, cfg: &QueryConfig) -> Result<Result_, QueryError>`.
- Tokenize: split `q` on whitespace, lowercase, drop empties; AND-match all
  tokens as case-insensitive substrings of `text` or `path`.
- Ranking (deterministic): score = path-hit bonus (10 if token in `path`)
  + occurrence count in `text`; sort by `(score desc, path asc, line asc)`.
  Same index + `q` + cfg => byte-identical order. No wall-clock, no hashmap
  iteration, no I/O, no network.
- Enforce `max_hits` then `max_bytes` (byte sum of snippets); over budget sets
  `truncated: true` and appends `TRUNC_MARKER` to the last snippet.
- `TRUNC_MARKER = "\n... [qry:truncated {dropped} hits/bytes]"`.
- No secret logging: never write index text, snippets, or `q` to logs/telemetry;
  hits flow only through the return value.

## Failure states

- Empty/whitespace-only `q`: `Err(QueryError::EmptyQuery)`; no scan performed.
- No match: `Ok` with `hits: []`, `truncated: false`, no marker.
- Disabled (`enabled: false`): `Ok` with `hits: []`, `truncated: false`;
  index never traversed.
- Over budget: `truncated: true`, tail marker present, snippet bytes
  `<= max_bytes + MARKER_MAX`; hit order still deterministic.
- Non-UTF8 not applicable (all `&str`); oversize single snippet truncates at
  `char_boundary` with marker.

## Resource bounds

- Pure library: no `Command`, no thread, no I/O, no network, no retained state.
  Time O(docs * tokens), memory O(max_hits + max_bytes).
- Disabled path: O(1), no traversal, no tokenize, negligible cost.

## Suggested module boundary

- Proposed owner: `crates/rw_query/src/lib.rs` (new crate `rw-query`).
- Integrator wires `pub use` / registration fragment; worker never edits shared
  `lib.rs`, `Cargo.toml`, schemas. Additive only.

## Frozen test obligations (exact asserts)

- RW-QRY-T01 hit: fixture index (TOOL-014 paths + `output_store.rs:12`
  containing `OutputStore`): `let r = query(idx, "OutputStore", &def()).unwrap()`:
  `assert!(!r.hits.is_empty())`, every hit `assert!(!h.path.is_empty() && h.line > 0)`,
  `assert_eq!(r.hits[0].path, "crates/tools/src/output_store.rs")`.
- RW-QRY-T02 miss: `query(idx, "zz-no-such-token-qq", &def()).unwrap()`:
  `assert!(r.hits.is_empty())`, `assert!(!r.truncated)`.
- RW-QRY-T03 empty query error: `q` in `["", "   "]`:
  `assert_eq!(query(idx, q, &def()), Err(QueryError::EmptyQuery))`.
- RW-QRY-T04 byte-budget truncation marker: `cfg = { enabled: true,
  max_hits: 1000, max_bytes: 256 }`, 200 matching docs:
  `assert!(r.truncated)`, last snippet `ends_with(MARKER_SUFFIX)`,
  `snippet_bytes <= 256 + MARKER_MAX`, `query(..) == query(..)`.
- RW-QRY-T05 deterministic order: repeated `query(idx, "tool output", &def())`:
  `assert_eq!(r1.hits, r2.hits)`; tie fixture (same score, paths `b.rs`,`a.rs`):
  `assert!(r.hits[0].path < r.hits[1].path)` (path-asc tiebreak).

## TDD steps

1. Inspect PLAN.md 5-6, docs/TDD.md 2-5, TOOL-014/TOOL-009 cards; cite commit + path:line.
2. Define contract/failures/bounds (above); open discovery proposal for gaps.
3. Author RED: 5 tests compile, fail on missing `rw_query::query`.
4. Freeze test hash + command manifest; controller verifies on disk.
5. Implement minimum native Rust; GREEN; refactor; rerun full suite.
6. Regressions: disabled zero-scan, whitespace query, marker determinism, no-logging.
7. Submit evidence + patch, never acceptance.

## Verification

- `cargo test -p rw-query` passes RW-QRY-T01..T05 on frozen hash.
- `cargo check --workspace` clean; `cargo fmt --check` clean.
- Disabled config: returns empty untruncated without traversing index.
