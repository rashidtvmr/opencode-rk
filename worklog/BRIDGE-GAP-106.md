# BRIDGE-GAP-106 scratchpad

claim: BRIDGE-GAP-106 via cc.claim, session ses_gap106, status in-progress.
owned file: crates/opentui-bridge/src/run_footer_prompt.rs (append only).
no lib.rs / Cargo.toml / cargo / commit per scope.

source evidence:
- crates/opentui-bridge/src/run_footer_prompt.rs:1-153 (PromptRow, MAX_TEXT 4096, tests mod 6 tests, frozen).
- TS truth footer.prompt.tsx:1-150 (createPromptState, draft/history wiring); grep history/draft state.
- prompt.shared.ts:55-94 createPromptHistory/pushPromptHistory (trim-filter, dedup consecutive, HISTORY_LIMIT slice); :96-129 movePromptHistory (index null/draft stash, dir -1/1, cursor guards).

observed: Rust models only draft.text+cursor; history/menus/parts host concerns per doc comment :7-8. Task extends with draft+history subset.

target boundary: APPEND ONLY after final `}` line 153: consts MAX_DRAFT_TEXT 8192 + MAX_DRAFT_HISTORY 50, struct PromptDraft {text,history,hist_cursor}, impl new/push_history/hist_prev/hist_next/word_count/is_empty, mod tests2 6 tests. Existing bytes untouched.

tests: tests2 (6): push_appends_and_resets_cursor, push_skips_empty_and_whitespace, prev_next_bounds, evicts_oldest_at_cap, word_count_splits_whitespace, is_empty_mirrors_text.

decisions:
- push skips trim-empty (mirrors TS trim filter), truncates text to 8KiB char boundary, drain-evicts oldest, resets cursor.
- prev: None->newest, oldest->false. next: None->false, past-newest resets cursor None + false (no draft stash field). No consecutive-dedup (spec silent; shortest diff).
- word_count = split_whitespace().count; is_empty = text.is_empty().

unknowns: none. verify = rustfmt --check only.
