# LANE-WF-TIMELINE scratchpad

## Claim
- Task: LANE-WF-TIMELINE (WF-TUI-TIMELINE: native transcript render state wiring, RAW_FEATURE 3.7)
- Session: ses_worker_wf
- Owned file: crates/cli/src/native_timeline.rs (NEW)

## Source evidence
- `crates/cli/src/native_transcript.rs`: TokenDelta, ToolState, StreamAccumulator, Transcript, TranscriptLine, strip_ansi, window_slice — read-only reference
- `AGENTS.md` worker protocol: claim → own → update → hand back

## Target boundary
- Pure-state TimelineItem {kind, stream_id, preview} bounded (MAX_ITEMS=512, preview<=160 bytes, char-boundary safe)
- TimelineBuilder::from_streams -> VecDeque<TimelineItem> sorted by stream_id
- Group consecutive tool states per stream (dedup by stream_id)
- Scrollback-stable marker (marker_stable method)
- forbid(unsafe_code), std-only

## Tests written
- T01: message+tool+reasoning classified ✓
- T02: bounded window at MAX_ITEMS, oldest evicted ✓
- T03: preview truncation char-boundary safe (4-byte UTF-8) ✓
- T04: tool grouping per stream (dedup same stream_id) ✓
- T05: stable under out-of-order stream_ids ✓
- T06: empty -> empty ✓

## Verification
- `rustc --edition 2021 --test src/native_timeline.rs` — 6/6 pass x2 stable
- Frozen sha256: 2c1b86e6aadbefe5af00b0d2a5b70eb93f76f9ac591b0f1fc565800ebb0d143a
- Zero test edits post-freeze

## Decisions
- ToolState defined locally (mirrors native_transcript::ToolState) for standalone compilation without lib.rs wiring
- Grouping via sort_by_key + dedup: last entry per stream_id wins
- Preview truncation uses char-boundary loop (same pattern as native_transcript)

## Remaining unknowns
- Integrator must add `pub mod native_timeline` to crates/cli/src/lib.rs (out of lane scope)
- ToolState duplication intentional for standalone test; integrator may unify types
