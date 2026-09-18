# LANE-CONTEXT-CMD

## Claim
- Task: /CONTEXT and /MEMORY snapshot service (roadmap 3.3+3.4)
- Session: ses_worker_ctx
- Owned file: crates/server/src/context_report.rs (NEW)

## Source evidence
- RulesSnapshot: crates/server/src/rules_loader.rs:35-37 (entries: Vec<RuleEntry>)
- RuleEntry: crates/server/src/rules_loader.rs:24-31 (path, glob, bytes)
- AutoReport pattern: crates/server/src/auto_report.rs (simple data + typed error + builder)
- TurnService pattern: crates/server/src/turn_service.rs (forbid(unsafe_code), std-only)

## Target boundary
- ContextReport: total_tokens, model_limit, segments (bounded MAX_SEGMENTS=16)
- Segment kinds: System, Rules, History, ToolResults, CurrentTurn
- MemoryReport: loaded (MemoryFile), skipped (Skip with reason)
- from_turn builds bounded ContextReport
- MemoryReport from RulesSnapshot-shaped input
- Typed errors, no panic, forbid(unsafe_code), std-only

## Tests (in-file)
- T01: segment split sums to total
- T02: bounded segments (MAX_SEGMENTS)
- T03: over-limit flag + headroom
- T04: memory loaded/skipped classification incl. reasons
- T05: deterministic order
- T06: estimator documented (bytes/4) applied consistently

## Decisions
- Token estimator: bytes / 4 (documented, consistent)
- MAX_SEGMENTS = 16
- Segment kinds as enum, not stringly typed
- from_turn takes a simple input struct (no async, no I/O)
- MemoryReport built from slice of RuleEntry-like input

## Completion evidence
- GREEN 11/11 standalone rustc --test (T01-T06 + 5 edge cases)
- Zero test edits post-freeze
- cmd: `rustc --edition 2021 --test crates/server/src/context_report.rs`
- All 11 tests pass: t01-t06 required + edge_empty_input, edge_zero_model_limit, edge_memory_zero_budget, edge_memory_budget_exceeded, edge_zero_tool_results_skipped

## Remaining unknowns
- Integrator wires pub mod context_report in lib.rs
