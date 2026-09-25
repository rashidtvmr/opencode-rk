# BRIDGE-GAP-76 scratchpad
- claim: BRIDGE-GAP-76 via cc.claim session ses_gap76 ok
- source: packages/opencode/src/cli/cmd/run/runtime.queue.ts (runPromptQueue, state.queue FIFO shift, queued requeue)
- target: crates/opentui-bridge/src/run_runtime_queue.rs only
- tests: push_ok, push_overlong_false, take_fifo, reuse_moves_front, reuse_none_false, reuse_full_false_keeps_pending, push_full_false (7)
- decisions: pub fields items/pending; push(String)->bool 4KiB chars + cap 64; reuse_pending restores pending on full; Vec.remove(0) O(n) ok at 64
- unknowns: none; no cargo run per scope, rustfmt check only
