# BRIDGE-GAP-81 run_trace
claim: ses_gap81 ok (in-progress)
source: packages/opencode/src/cli/cmd/run/trace.ts (Trace write JSONL, lazy env gate)
style ref: crates/opentui-bridge/src/run_subagent_data.rs:18-27 truncate
boundary: TraceSpan cap128, TraceLog cap256, record/total_ms saturating/slowest; std-only forbid unsafe; 148 lines
tests: record_ok, name_truncated_to_cap, cap_drops, total_saturates, slowest_picks_max, empty_none (6)
verify: rustfmt --check crates/opentui-bridge/src/run_trace.rs PASS; no cargo per scope
