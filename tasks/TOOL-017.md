# TOOL-017

Status: NOT STARTED. Kind: product. Runtime optional: True.
Mandatory for full declared release: yes.
Requirements: REQ-039 (proposed, native codebase query).
Dependencies: none.
Test obligations: TOOL-017-T01, TOOL-017-T02, TOOL-017-T03, TOOL-017-T04, TOOL-017-T05.

## User-observable outcome

Native codebase query: keyword search over a caller-supplied local index.
No network, no disk scan, no subprocess. Returns ranked hits with
`file:line` citations for every hit. Default-on with opt-out; zero hidden
cost when off (no index traversal).

## Source evidence

- Commit `a453067f1df20a9c7e2f0ecba5dc4c28189860c5` (HEAD at card authoring).
- PLAN.md sections 5-6: slice independence rules, mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: lifecycle order, compiling RED, frozen hash, GREEN minimum.
- tasks/TOOL-014.md: task-card model (Status/Kind/contract/test obligations; bounded OutputStore, byte budget).
- tasks/TOOL-009.md: ToolSearch just-in-time discovery precedent.
- docs/proposals/RW-SLICE-02-query-spec.md: query spec, contract, bounds,
  RW-QRY-T01..T05 definitions mirrored below as TOOL-017-T01..T05.
- Classification: new user requirement (REQ-039 proposed), deliberate
  resource-bounded deviation (presentational ranking over caller-built index;
  caller re-verifies cited file:line against source).

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
- Suggested module boundary: new module owning `QueryConfig, Hit, IndexedDoc,
  Result_, QueryError, query, TRUNC_MARKER` (proposed owner
  `crates/rw_query/src/lib.rs`, new crate `rw-query`); shared `lib.rs`
  wiring left to integrator, worker never edits it.

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
- No unbounded queue, no unbounded retained output; owner and cancel path defined
  per TOOL-014 precedent (TOOL-014 byte-budget eviction).

## Test obligations (frozen, mirror RW-QRY-T01..T05 exact asserts)

- TOOL-017-T01 (hit, mirrors RW-QRY-T01): fixture index (TOOL-014 paths +
  `output_store.rs:12` containing `OutputStore`):
  `let r = query(idx, "OutputStore", &def()).unwrap()`:
  `assert!(!r.hits.is_empty())`, every hit
  `assert!(!h.path.is_empty() && h.line > 0)`,
  `assert_eq!(r.hits[0].path, "crates/tools/src/output_store.rs")`.
- TOOL-017-T02 (miss, mirrors RW-QRY-T02):
  `query(idx, "zz-no-such-token-qq", &def()).unwrap()`:
  `assert!(r.hits.is_empty())`, `assert!(!r.truncated)`.
- TOOL-017-T03 (empty query error, mirrors RW-QRY-T03): `q` in `["", "   "]`:
  `assert_eq!(query(idx, q, &def()), Err(QueryError::EmptyQuery))`.
- TOOL-017-T04 (byte-budget truncation marker, mirrors RW-QRY-T04):
  `cfg = { enabled: true, max_hits: 1000, max_bytes: 256 }`, 200 matching docs:
  `assert!(r.truncated)`, last snippet `ends_with(MARKER_SUFFIX)`,
  `snippet_bytes <= 256 + MARKER_MAX`, `query(..) == query(..)`.
- TOOL-017-T05 (deterministic order, mirrors RW-QRY-T05): repeated
  `query(idx, "tool output", &def())`: `assert_eq!(r1.hits, r2.hits)`;
  tie fixture (same score, paths `b.rs`, `a.rs`):
  `assert!(r.hits[0].path < r.hits[1].path)` (path-asc tiebreak).

## TDD steps

1. Inspect: PLAN.md 5-6, TDD.md 2-5, TOOL-014.md, TOOL-009.md,
   RW-SLICE-02-query-spec.md (done, see evidence).
2. Contract: defined above.
3. Author tests TOOL-017-T01..T05; establish compiling RED (fail: no `query`).
4. Freeze test hash + command manifest.
5. Implement minimum native Rust `query` in new crate/module.
6. GREEN, refactor, rerun; negative tests (disabled zero-scan, whitespace
   query, marker determinism, no-logging).
7. Evidence + patch; verifier decides acceptance.

## Verification

```bash
git status --short -- tasks/TOOL-017.md
cargo test -p rw-query
cargo check --workspace
```

## Remaining gaps / unknowns

None.
