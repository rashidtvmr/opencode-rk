# RW-SLICE-03: change-risk, why, dependency-path over local index

Status: PROPOSED. Kind: product. Runtime optional: False.
Mandatory for full declared release: no (opt-out default-on slice).
Requirements: none (new user requirement, repowise-adjacent local queries).
Dependencies: local symbol index slice (RW-SLICE-01 or equivalent owner).
Test obligations: RW-RSK-T01, RW-RSK-T02, RW-RSK-T03, RW-RSK-T04, RW-RSK-T05.

## User-observable outcome

Bounded local queries over the workspace symbol index: `blast_radius(sym)`
estimates change risk (direct + transitive dependents, capped), `why(sym)`
explains a symbol (owner file:line, kind, dependent count), `dep_path(a, b)`
returns one short dependency path between two symbols. All answers cite
`file:line`; index is advisory, every cited edge re-verified against source.
Feature is default-on but inert when index disabled or empty: zero background
work, zero network, queries return explicit empty/unavailable, no hidden cost.

## Source evidence

- crates/catalog/src/search.rs:13 - `SearchIndex` in-memory index pattern
  (index/search/clear/stats) to mirror for symbol graph.
- crates/tools/src/output_store.rs - bounded eviction pattern (`max_size`,
  oldest-first) to reuse for result caps.
- crates/tools/src/lib.rs:4-46 - additive `pub mod` registration pattern;
  integrator wires `lib.rs`, worker owns only new module file.
- PLAN.md:5-6 - slice independence + RED/GREEN/freeze lifecycle this card follows.
- docs/TDD.md:2-5 - compiling RED, frozen hash, GREEN/refactor rules.

## Observable contract

- `RiskIndex::new caps: RiskCaps` builds empty graph; no IO, no threads.
- `index_symbol(SymDef { name, kind, file, line }) -> Result<(), RiskError>`
  upserts; duplicate name overwrites, count unchanged.
- `index_edge(Edge { from, to }) -> Result<(), RiskError>`; both ends must
  exist else `UnknownSymbol`; self-edge rejected (`SelfEdge`); duplicate edge
  ignored, count unchanged.
- `blast_radius(name, limit) -> Result<BlastRadius, RiskError>`: BFS over
  reverse edges from `name`; `truncated=true` when dependents exceed
  `limit.max_nodes`; `members.len() <= max_nodes`; unknown name =>
  `UnknownSymbol`. `depth` per member = BFS distance; sorted by (depth, name).
- `why(name) -> Result<Why, RiskError>`: returns `{ def, dependents,
  dependents_truncated }`; `def` carries verified `file:line`; unknown name
  => `UnknownSymbol`. Dependent list capped at `limit.max_nodes`.
- `dep_path(a, b, limit) -> Result<Option<Path>, RiskError>`: BFS forward
  from `a` following edges; returns node list `[a..b]` with
  `len <= limit.max_depth + 1`; `None` when unreachable within depth;
  unknown endpoint => `UnknownSymbol`.
- `owner_of(file) -> Vec<&SymDef>`: file-defined symbols in line order;
  empty when none. Pure local scan of indexed defs, no FS read.
- `clear()` empties defs + edges, resets counts to zero.
- `stats() -> (symbols, edges)` reflects current counts.
- Opt-out: `RiskIndex::disabled()` (or `enabled=false` config) makes every
  query return `RiskError::Disabled`; `index_*` becomes no-op `Ok`;
  construction spawns nothing, holds only empty vecs.

## Failure states

- `UnknownSymbol(name)` on any query/edge touching a missing symbol.
- `SelfEdge` on `from == to`; `Disabled` on all queries when opted out.
- `LimitExceeded` never hard-errors BFS: instead truncate + flag
  (`truncated=true`, `dependents_truncated=true`, `None` for over-depth path).
- Empty index (enabled, zero symbols): queries return `UnknownSymbol` (blast/
  why/path) or empty vec (owner_of), never panic, never scan FS.
- Stale index: caller re-verifies cited `file:line` against source before
  acting; `verify ingester` hook left to integrator (out of slice scope).

## Resource bounds

- `RiskCaps { max_symbols: 20_000, max_edges: 100_000, max_nodes: 200,
  max_depth: 8 }`; `index_*` rejects beyond caps with `CapExceeded`.
- All queries O(V+E) worst case, BFS early-exits at `max_nodes`/`max_depth`.
- Memory: two vecs + adjacency map only; no threads, no channels, no global
  state; `Send` but no interior locking required.
- No FS, no network, no wall clock in queries; deterministic fixtures only.

## Suggested module boundary

- Owned file: `crates/catalog/src/risk_graph.rs` (new; `pub struct RiskIndex,
  RiskCaps, SymDef, Edge, BlastRadius, Why, Path; pub enum RiskError`).
- Tests: `crates/catalog/tests/risk_graph.rs` (frozen RW-RSK-T01..T05).
- Integrator pre-wires `crates/catalog/src/lib.rs: pub mod risk_graph;`
  worker never edits shared `lib.rs`.

## Frozen test obligations (exact asserts)

- RW-RSK-T01 blast radius bounded: chain a->b->c plus 300 extra dependents
  of b, `max_nodes=200`; `assert!(r.truncated)`; `assert!(r.members.len()
  <= 200)`; `assert!(r.members.iter().any(|m| m.name=="c"))`.
- RW-RSK-T02 why cites owner: index `fn pay` at `pay.rs:42` with 2 callers;
  `assert_eq!(w.def.file, "pay.rs")`; `assert_eq!(w.def.line, 42)`;
  `assert_eq!(w.dependents.len(), 2)`.
- RW-RSK-T03 dep path depth-bound: diamond a->b->d, a->c->d, d->e;
  `dep_path("a","e")` returns `Some(p)` with `p[0]=="a"`, `p[last]=="e"`,
  `p.len() <= max_depth+1`; `dep_path("e","a")` returns `None`.
- RW-RSK-T04 failure states: unknown symbol query asserts
  `err == UnknownSymbol`; self-edge asserts `err == SelfEdge`; duplicate
  edge keeps `stats().1` unchanged; over-cap `index_symbol` asserts
  `CapExceeded`.
- RW-RSK-T05 opt-out zero cost: `RiskIndex::disabled()`; all queries assert
  `err == Disabled`; `index_symbol` returns `Ok`; `stats() == (0, 0)`; no
  thread spawn (assert `std::thread::available_parallelism` unaffected is
  N/A; assert no background task via `stats` + query-only behavior).

## TDD steps

1. Cite commit + file:line per behavior (search.rs:13, output_store evict,
   lib.rs mods). Record contract above; no product edit.
2. Author `crates/catalog/tests/risk_graph.rs` with RW-RSK-T01..T05; run
   `cargo test -p catalog --test risk_graph` expecting compile error / RED.
3. Freeze test hash + command manifest via controller; worker never edits
   tests after freeze.
4. Implement minimum `risk_graph.rs` to GREEN; rerun full `cargo test
   -p catalog`; clippy clean.
5. Negative/resource pass: T04/T05 plus truncate flags; submit evidence +
   patch, never acceptance.

## Verification

- `cargo test -p catalog --test risk_graph` (frozen T01..T05 GREEN).
- `cargo test -p catalog` (no sibling regression).
- `cargo clippy -p catalog -- -D warnings` (lint clean).
- `git status --short` shows only `docs/proposals/RW-SLICE-03-risk-spec.md`
  (this spec lane) plus integrator-owned module/test files at build time.
