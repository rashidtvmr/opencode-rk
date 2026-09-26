# BRIDGE-PAR-199 scratchpad

Claim: BRIDGE-PAR-199 owned by ses_par199, scratchpad worklog/BRIDGE-PAR-199.md.
Source evidence: crates/opentui-bridge/src/prompt_ctx.rs:5 `MAX_DRAFT_CHARS=8192`, struct PromptCtx draft:String capped via take_chars (read-only, not edited).
Observed: no draft_store.rs exists; prompt_ctx owns full draft+history, no char-cursor insert/backspace primitive.
Target boundary: ONE new file crates/opentui-bridge/src/draft_store.rs. No lib.rs/Cargo.toml/prompt_ctx.rs edits. No cargo, no commit.
Tests: 6 unit tests in-file (new_empty, insert_append, insert_middle, backspace, cap_8kib, move_cursor_clamp+clear, unicode).
Decisions: private fields + text()/cursor() getters; char-index cursor; byte_idx via char_indices().nth; cap enforced in insert; std-only; forbid(unsafe_code).
Unknowns: none. Awaiting rustfmt --check.
