# TOOL-018

Status: NOT STARTED. Kind: product. Runtime optional: True.
Mandatory for full declared release: yes.
Requirements: REQ-039 (proposed, change-risk/why/dep-path queries).
Dependencies: none.
Test obligations: TOOL-018-T01, TOOL-018-T02, TOOL-018-T03, TOOL-018-T04, TOOL-018-T05.

## User-observable outcome

Bounded local queries over the workspace symbol index: `blast_radius(sym)`
estimates change risk (direct + transitive dependents, capped), `why(sym)`
explains a symbol (owner file:line, kind, dependent count), `dep_path(a, b)`
returns one short dependency path between two symbols. All answers cite
`file:line`; index is advisory, every cited edge re-verified against source
before acting. Default-on but inert when index disabled or empty: zero
background work, zero network, queries return explicit empty/unavailable,
no hidden cost.

## Source evidence

- PLAN.md sections 5-6: slice independence rules, mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: lifecycle order, compiling RED, frozen hash, GREEN minimum.
- tasks/TOOL-014.md: task-card model (Status/Kind/contract/test obligations).
- tasks/TOOL-021.md: REQ-039 proposed precedent (Runtime optional True, mandatory yes, deps none, T01..T05 mirror pattern).
- docs/proposals/RW-SLICE-03-risk-spec.md: risk spec, contract, bounds,
  RW-RSK-T01..T05 definitions mirrored below as TOOL-018-T01..T05.
- Classification: new user requirement (REQ-039 proposed), deliberate
  resource-bounded deviation (BFS capped at max_nodes/max_depth; presentational
  risk estimate, caller re-verifies cited file:line against source).

## Observable contract

- `RiskIndex::new(caps: RiskCaps)` builds empty graph; no IO, no threads.
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
- Suggested module boundary: new module owning `RiskIndex, RiskCaps,
  SymDef, Edge, BlastRadius, Why, Path, RiskError`; shared `lib.rs`
  wiring left to integrator, worker never edits it.

## Failure states

- `UnknownSymbol(name)` on any query/edge touching a missing symbol.
- `SelfEdge` on `from == to`; `Disabled` on all queries when opted out.
- `LimitExceeded` never hard-errors BFS: instead truncate + flag
  (`truncated=true`, `dependents_truncated=true`, `None` for over-depth path).
- Empty index (enabled, zero symbols): queries return `UnknownSymbol` (blast/
  why/path) or empty vec (owner_of), never panic, never scan FS.
- Over-cap `index_symbol`/`index_edge` rejects with `CapExceeded`, counts unchanged.
- Stale index: caller re-verifies cited `file:line` against source before
  acting; index advisory only.

## Resource bounds

- `RiskCaps { max_symbols: 20_000, max_edges: 100_000, max_nodes: 200,
  max_depth: 8 }`; `index_*` rejects beyond caps with `CapExceeded`.
- BFS early-exits at `max_nodes`/`max_depth`; all queries O(V+E) worst case.
- Memory: two vecs + adjacency map only; no threads, no channels, no global
  state; `Send` but no interior locking required.
- No FS, no network, no wall clock in queries; deterministic fixtures only.
- Disabled path: no heap alloc, O(1) early return; zero background work.

## Test obligations (frozen)

- TOOL-018-T01 (blast radius bounded, mirrors RW-RSK-T01): chain a->b->c
  plus 300 extra dependents of b, `max_nodes=200`; `assert!(r.truncated)`;
  `assert!(r.members.len() <= 200)`;
  `assert!(r.members.iter().any(|m| m.name == "c"))`.
- TOOL-018-T02 (why cites owner, mirrors RW-RSK-T02): index `fn pay` at
  `pay.rs:42` with 2 callers; `assert_eq!(w.def.file, "pay.rs")`;
  `assert_eq!(w.def.line, 42)`; `assert_eq!(w.dependents.len(), 2)`.
- TOOL-018-T03 (dep path depth-bound, mirrors RW-RSK-T03): diamond a->b->d,
  a->c->d, d->e; `dep_path("a", "e")` returns `Some(p)` with `p[0] == "a"`,
  `p[last] == "e"`, `p.len() <= max_depth + 1`;
  `dep_path("e", "a")` returns `None`.
- TOOL-018-T04 (failure states, mirrors RW-RSK-T04): unknown symbol query
  asserts `err == UnknownSymbol`; self-edge asserts `err == SelfEdge`;
  duplicate edge keeps `stats().1` unchanged; over-cap `index_symbol`
  asserts `CapExceeded`.
- TOOL-018-T05 (opt-out zero cost, mirrors RW-RSK-T05):
  `RiskIndex::disabled()`; all queries assert `err == Disabled`;
  `index_symbol` returns `Ok`; `stats() == (0, 0)`; construction spawns
  nothing, holds only empty vecs, no background task.

## TDD steps

1. Inspect: PLAN.md 5-6, TDD.md 2-5, TOOL-014.md, TOOL-021.md,
   RW-SLICE-03-risk-spec.md (done, see evidence).
2. Contract: defined above.
3. Author tests TOOL-018-T01..T05; establish compiling RED (fail: no index).
4. Freeze test hash + command manifest.
5. Implement minimum `RiskIndex` natively in Rust.
6. GREEN, refactor, rerun; negative tests (unknown/self-edge/caps/truncate).
7. Evidence + patch; verifier decides acceptance.

## Verification

```bash
git status --short -- tasks/TOOL-018.md
```
