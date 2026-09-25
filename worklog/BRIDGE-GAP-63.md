# BRIDGE-GAP-63 scratchpad
- claim: BRIDGE-GAP-63 via ses_gap63, ledger in-progress
- source: packages/opencode/src/cli/cmd/run/prompt.shared.ts (promptCopy/promptSame/history ring, pure fns)
- target: crates/opentui-bridge/src/run_prompt_shared.rs, queue/types only per task spec
- tests: enqueue_ok, full_errs, fifo_order, peek_none_empty, dequeue_none_empty, peek_returns_head
- decision: char-count caps (id 64, text 4096, queue 64); enqueue/dequeue/peek/len/is_empty; std-only, forbid(unsafe_code)
