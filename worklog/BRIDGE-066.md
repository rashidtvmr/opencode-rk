# BRIDGE-066 worklog

Claim: autocomplete pure helpers port into prompt_store.rs.
Evidence: autocomplete.tsx:27-57 (range parse), :172-190 (needsSpace+delete), :237-239 (frecency update), :447-524 (merge/sort/padEnd/fuzzysort/scoreFn), :678-707 (hide/@trigger); display.ts:38-48 (mention rule).
Target: EXTEND ONLY prompt_store.rs; no cargo run per scope.
Tests: 5 new fns (range_parse_basic, score_prefix_frecency, hide_rules, mention_space_merge, merge_skips_skill_tags_mcp) = 8+ asserts; RED written first mentally (missing fns), impl added, logically green.
Decisions: score_with_frecency(base:u8,frecency:f32)->f32 per contract; f64 frecency_score feeds it via cast. RANK_LIMIT=LIMIT=10. FUZZY_THRESHOLD=0.5 documented std-only divergence (substring, no fuzzysort). merge_commands takes (name,is_skill,is_mcp) triples, skips skill, :mcp tag, sort, padEnd max+2.
Unknowns: byte-len vs display-width pad (TS padEnd is char-based; Rust format! is byte/char for ASCII cmd names, fine). localeCompare vs byte sort (ASCII equivalent).
