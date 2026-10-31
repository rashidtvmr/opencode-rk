# FIX-LOOP-RULES — scratchpad

Claim: FIX-LOOP-RULES, session ses_fix_looprules, status in-progress.
Owned file: crates/server/src/rules_globs.rs ONLY.

## Source evidence (HEAD 62f7eb1)
- crates/server/src/rules_globs.rs:42 `loaded: Vec<String>` private field on pub `RuleSet`
- crates/server/src/rules_globs.rs:25 `LoadDecision { load, unload }`
- crates/server/src/rules_globs.rs:70 `evaluate(&mut self, touches) -> LoadDecision`
- crates/server/src/rules_inject.rs:82 `assemble_prompt` (consumer of rules surface)
- crates/server/src/loop_driver.rs:26 `MAX_LOOP_STEPS=256`, :375 `LoopDriver` (pure-state plan driver, no I/O)
- crates/server/src/lib.rs:11,832,977 live turn uses `LoopController`, NOT `LoopDriver`

## Observed scenario
- `RuleSet.loaded` unreachable outside module; tests inside `mod tests` access private field directly (t05/t06), so external callers (inject/prompt assembly, future hysteresis observers) have no read path.
- `LoopDriver` vs `LoopController` boundary undocumented near rules surface; live turn path owned by `LoopController`.

## Target boundary
- Add pub read-only accessors ONLY: `RuleSet::loaded() -> &[String]` + `RuleSet::loaded_count() -> usize`.
- Add module doc line clarifying LoopDriver (pure plan state machine) vs LoopController (live turn path in lib.rs) boundary.
- Do NOT rewire live turn, loop_driver.rs, lib.rs, tests. No commit/push per lane.

## Tests
- Frozen: existing `rules_globs` unit tests t01..t08 (untouched).
- Verify cmd: `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-server --lib rules_globs`

## Decisions
- Method named `loaded()` (field/method namespaces disjoint; in-module `rs.loaded` field access keeps working).
- `loaded()` returns borrowed slice, no clone/bound growth (bounded by MAX_LOADED=32).

## Remaining
- Run verify, paste tail, set honest ledger status.
