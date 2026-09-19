# LANE-CONTEXT-ACCOUNT scratchpad

## Claim
- Task: LANE-CONTEXT-ACCOUNT
- Session: ses_worker_ctx_account
- Status: completed

## Source evidence
- Pattern: crates/server/src/context_report.rs (sibling, tokens/bytes/4 heuristic, bounded segments)
- Pattern: crates/server/src/admission_bounds.rs (saturating math, bounded structs)
- Pattern: crates/server/src/turn_parts.rs (snapshot/roundtrip, std-only)

## Target boundary
- NEW file only: crates/server/src/context_accounting.rs
- NEW file only: crates/server/tests/context_accounting.rs
- No existing files touched

## What was built
Bounded context accounting service. Given typed provider-round pieces:
- system_prompt_bytes, rules_bytes, history_messages (capped), tool_schemas_bytes, current_turn_bytes
- Produces per-segment token estimate (bytes/4 documented heuristic)
- Total tokens vs model context limit
- Compaction headroom flag with configurable threshold
- Bounded snapshot struct for /CONTEXT render

## Tests (12/12 GREEN)
- T01: per-segment split sums to total
- T02: estimator bytes/4 consistent
- T03: headroom threshold boundary
- T04: saturation on huge input (no overflow)
- T05: cap behavior (MAX_HISTORY_PIECES = 256)
- T06: deterministic output
- T07: stable segment order
- T08: snapshot roundtrip
- T09: empty input
- T10: zero model limit error
- T11: headroom saturates over limit
- T12: zero tool schemas skipped

## Hashes
- RED sha256 (tests): dac665e8bb9eb5ec8cc4c759f126e9339096dab15615a54551350384be894a7a
- GREEN sha256 (impl): 3be71e98ea5071a1d11ffdd8f8abea6342e6fa24267cee1a9f9735c5c799eae4
- GREEN sha256 (tests): 134ef37ff1e6e154291701e0e63dd16b26c4af35b42c0891b31de7894daabe6d

## Decisions
- f64 threshold can't derive Eq — AccountInput uses Clone+Debug+Default only
- Saturating fold for total_tokens to prevent overflow on huge inputs
- SnapshotSegment.kind is String (for JSON roundtrip), compared via format!("{:?}", kind)
- Stable segment order: System, Rules, History*, ToolSchemas*, CurrentTurn

## Remaining unknowns
- Integrator must wire `pub mod context_accounting` in server/lib.rs
