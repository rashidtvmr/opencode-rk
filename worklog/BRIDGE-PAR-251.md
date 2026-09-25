# BRIDGE-PAR-251 scratchpad
claim: BRIDGE-PAR-251 ses_par251 worklog/BRIDGE-PAR-251.md
source: crates/opentui-bridge/src/prompt_history.rs (PromptHistory prev/next/floor, MAX_TEXT 4096) read-only truth
target: crates/opentui-bridge/src/prompt_history_full.rs HistoryFull plain-item mirror
tests: 6 in-file (push_fronts_lens, evict_caps_50, walk_bounds, trunc_8kib_blank, push_resets_cursor, mb_cut_boundary) written unrun per scope
decisions: pub fields per deliverable; trim+floor(8KiB char-boundary)+front-insert+truncate50+cursor-reset; std-only forbid(unsafe_code); 119 lines rustfmt clean
