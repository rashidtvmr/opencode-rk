# BRIDGE-PAR-296
claim: ses_par296 ok
source: packages/tui/src/util/persistence.ts:1-33 (readText/readJson/writeText/appendText/writeJsonAtomic tmp+rename); sibling persistence.rs (disk atomic 1MiB) + json_persist.rs (shape check)
target: crates/opentui-bridge/src/persist_util_full.rs, in-memory twin, 117 lines, forbid unsafe, std-only
tests: roundtrip, overwrite_keeps_one_entry, full_rejects_new_key, oversize rejected, remove present/missing, missing load none (6)
decisions: Vec<(String,String)> LRU-ish move-to-back on overwrite (simplest eviction: reject when full); byte-len caps; bool fail-closed like TS throw boundary
verify: rustfmt --check PASS
