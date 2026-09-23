# BRIDGE-023 prompt_store

Claim: lane-owned `prompt_store.rs` drafted, lib.rs untouched (owner must add `pub mod prompt_store;`).
Do NOT run cargo per task card; logical GREEN only, gate unproven.

Evidence per item:
- frecency score+cap+evict: `packages/tui/src/prompt/frecency.tsx:10,35,64-67`; shim `component/prompt/frecency.tsx:1`
- history cap+cursor+dup: `packages/tui/src/prompt/history.tsx:27,77-78,87-90`; shim `component/prompt/history.tsx:1`
- stash cap+overflow+pop: `packages/tui/src/prompt/stash.tsx:15,56-58,70`; shim `component/prompt/stash.tsx:1`
- cwd: `component/prompt/cwd.ts` 0 bytes; mirrors `component/prompt/index.tsx:445` paths.cwd
- workspace: `component/prompt/workspace.tsx:16-137` transient signals only, not ported
- move: `component/prompt/move.tsx:14-16` reminder text + transient signals, not ported
- attachment: `component/prompt/local-attachment.ts:25-34,39-44`

Tests (8): decay_math_exact, frecency_missing_scores_zero, frecency_record_counts_and_bound,
history_cursor_clamps, history_duplicate_push_skips, stash_overflow_drops_oldest,
cwd_trust_flag, attachment_kinds_and_over_cap.

Unknowns: lane gates demand lib.rs wiring + cargo run which task forbids; hand to orchestrator.
Bounds follow lane contract (256/500/16) not TS (1000/50/50); noted in module docs.
