# BRIDGE-PAR-358 (unclaimed, file-only)

Claim: orchestrator owns claims.json; proceeding file-only per task.
Evidence: opencode/packages/tui/src/component/prompt/index.tsx:1-50 (prompt imports/TextareaRenderable); opentui-bridge/src/component_prompt_full.rs:1-11 (CompPrompt style/forbid unsafe).
Target: crates/opentui-bridge/src/comp_prompt_index_full.rs only. No lib.rs/Cargo.toml edits.
Tests: 5 in-file (add_has_len, dup_rejected, truncates_at_64, evicts_oldest_at_16, empty_rejected).
Decisions: Vec<String> FIFO evict oldest; norm trunc 64 chars; empty + dup -> false; std-only, forbid(unsafe_code).
Unknowns: none.
Verify: rustfmt --check PASS, 82 lines (<90).
