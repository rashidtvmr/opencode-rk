# LANE-WF-CREATE: Workflow Schema + Validator

## Claim
- Task: RAW_FEATURE 3.7 lane WF-CREATE
- Session: ses_orch_wave4
- Status: in-progress -> completed

## Source Evidence
- `crates/agents/src/workflow_schema.rs` (NEW, owned)
- `crates/agents/tests/workflow_schema.rs` (NEW, test file)
- `crates/agents/Cargo.toml` - already has serde, thiserror; serde_json in dev-deps

## Target Boundary
- Typed schema: Workflow {name, description, steps}, Step {id, kind, args, next}
- DAG validation: single entry, no cycles, unreachable-step, duplicate-id, unknown-ref
- Bounded caps: MAX_STEPS=256, MAX_EDGES=1024, MAX_DEPTH=64
- Deterministic topological order (Kahn's with sorted neighbors)
- JSON serialize/deserialize round-trip via serde
- Human-readable render_plan function
- 13 tests: valid DAG, cycle, multi-root, unreachable, topo stability, caps, round-trip, render, depth, empty, duplicate id, duplicate root

## Decisions
- Validation order: empty -> steps cap -> edges cap -> duplicates -> unknown refs -> roots -> reachability -> depth
- Cycle detection via DFS with stack tracking
- Topological order via Kahn's algorithm with deterministic sorted neighbor selection
- Unreachable steps checked after root determination (single root required)
- Depth measured from root to deepest leaf via memoized DFS

## Evidence
- RED sha256 (tests): cf9cf30cb9ad1ecc2d228776cf9d560d4cbdca7ea0eb52c753d69654588f7113
- GREEN sha256 (impl): 696d187124d6ac30927895f548157b3fa3a49f0c48e31388140a4302978169de
- Tests: CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-agents --test workflow_schema = 13 passed

## Remaining Unknowns
- lib.rs integration (pub mod workflow_schema) left to integrator per lane rules
- Execution engine that runs validated workflows is a follow-up lane
- STEP args schema validation (currently free-form JSON string) could be tightened
