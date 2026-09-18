# LANE-GLOBS — wave2 scratchpad

## Integrator completion (orchestrator)
Worker stalled after T06 failure (eviction deadlock: `always` rule re-pushed to
queue FRONT permanently blocked eviction). Integrator fixed evict_if_full:
always rules rotate to BACK of load_order; oldest non-always evicted; bounded
rotation. Final: rustc --test rules_globs 8/8 green. Zero frozen-test edits.
