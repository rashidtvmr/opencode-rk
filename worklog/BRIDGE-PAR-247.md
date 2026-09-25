# Claim: BRIDGE-PAR-247 ses_par247
- Source evidence: crates/opentui-bridge/src/sync_search.rs:1-45 (exact binary search, MAX_ITEMS 5000, forbid unsafe, std-only)
- Observed: no substring/case-insensitive search exists; sync_search covers exact only
- Target boundary: ONE new file crates/opentui-bridge/src/sync_search_full.rs; no lib.rs/Cargo.toml/sync_search.rs edits; no cargo/commit
- Tests: >=5 unit tests in-file; verification rustfmt --check only
- Decisions: private fields + accessors (caps enforced); query truncated by chars to 128; empty query clears hits; per-item to_lowercase (ponytail: skip lowercase cache, add when profiling needs it)
- Remaining: write file, rustfmt check, flip completed
