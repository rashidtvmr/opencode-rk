# LANE-GLOBS-LIVE scratchpad

## Claim
- Task: LANE-GLOBS-LIVE
- Session: ses_worker_globs_live
- Owned file: crates/server/tests/rules_globs_live.rs (NEW)

## Source evidence
- rules_globs.rs:41 `loaded: Vec<String>` — private field, no public accessor
- rules_globs.rs:43 `hysteresis: HashMap<String, RuleState>` — private
- rules_globs.rs:34 `struct RuleState { last_match_round, round_started_loaded }` — not exposed
- rules_globs.rs:30 `HYSTERESIS_ROUNDS = 3`
- rules_globs.rs:31 `MAX_LOADED = 32`
- rules_loader.rs:129 `pub fn load_rules(workspace_root: &Path) -> Result<RulesSnapshot, RulesError>`
- lib.rs:29-30 `pub mod rules_globs; pub mod rules_loader;` — both wired

## Observed scenario
- `rules_loader::load_rules` discovers AGENTS.md, CLAUDE.md, rules/*.md
- Frontmatter `globs:` field parsed into `RuleEntry.glob`
- `snapshot_to_rules` bridge converts entries → `RuleWithGlob`
- `RuleSet::evaluate` uses `GlobMatcher::matches` against touches
- Hysteresis: matched rules stay loaded for HYSTERESIS_ROUNDS (3) after last match
- Eviction: `evict_if_full` rotates always-rules to back, evicts oldest non-always
- No deadlock: always-rules cannot block the eviction queue

## Target boundary
- 3 integration tests: pipeline, bounds, transcript replay
- All use public API only (evaluate returns LoadDecision)
- Fixture helpers in test file (snapshot_to_rules bridge, workspace setup)

## Tests written
- scenario_a_pipeline_discover_and_evaluate: loader→RuleSet→evaluate→hysteresis
- scenario_b_bounds_max_loaded_no_deadlock_with_always: cap + always rule survival
- scenario_c_decision_transcript_replay: record decisions, verify consistency

## RED/GREEN
- RED: initial compilation failed (private field `loaded` access) — observed
- GREEN: refactored to use only public API (LoadDecision vectors)
- RED sha256: 1fc99991b0ddbbbee100ea1688287ad4b1a8addbc8970ec735451aa9969c030f
- GREEN: 3/3 pass, zero test edits post-freeze

## WIRING GAPS (document for orchestrator)
1. `RuleSet.loaded` is private (rules_globs.rs:41) — no `loaded()` or `loaded_count()` accessor
   - Tests track state indirectly via LoadDecision load/unload vectors
   - A public accessor would enable direct loaded-set assertions
2. No `DecisionReason` or record surface — `hysteresis` HashMap is private (rules_globs.rs:43)
   - Transcript replay can verify WHAT happened but not WHY
   - Would need `pub fn decision_reason(&self, name: &str) -> Option<DecisionReason>` or
     `RecordedDecision { name, reason }` in LoadDecision
3. No live session path feeds file touches through the pipeline
   - The public API works end-to-end (proven by these tests)
   - Missing: a session/daemon code path that calls load_rules + RuleSet::evaluate
     on actual file-touch events

## Decisions
- Use `snapshot_to_rules` bridge in test file (fixture helper, not product code)
- Track loaded state via accumulated LoadDecision vectors (compensates for private field)
- Scenario C documents the missing WHY surface as a gap, not a fake assertion

## Gate results
- `cargo test -p opencode-rk-server --test rules_globs_live`: 3/3 pass
- `cargo test -p opencode-rk-server`: 1 pre-existing flaky failure (session_turn_stream_api.rs:380), unrelated to this lane
