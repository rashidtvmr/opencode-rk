# LANE-RULES-INJECT scratchpad

## Claim
Roadmap 1.7 + 3.5 remainder: typed PROMPT-ASSEMBLY contract — a pure function
that composes rules_loader discovery + rules_globs LoadDecision + bounded base
system prompt into final system prompt + per-round InjectRecord.

## Source evidence
- `crates/server/src/rules_loader.rs`: `RulesSnapshot { entries: Vec<RuleEntry> }`, `RuleEntry { path, glob, bytes }`, constants MAX_FILES/MAX_FILE_BYTES/TOTAL_BUDGET
- `crates/server/src/rules_globs.rs`: `LoadDecision { load: Vec<String>, unload: Vec<String> }`, `RuleSet::evaluate`
- `crates/server/src/context_report.rs:196-225`: `MemoryReport { loaded: Vec<MemoryFile>, skipped: Vec<SkippedFile> }` — InjectRecord mirrors this shape

## Contract
- Input: `&RulesSnapshot`, `&LoadDecision`, `&str` (base prompt)
- Output: `(String, InjectRecord)` — assembled prompt + per-round record
- Injection policy: no-glob entries always inject; glob entries inject only if name in `decision.load`
- Byte cap: `INJECT_BYTE_CAP = 128 KiB` total injected rule content
- Record cap: `MAX_RECORD_ENTRIES = 64` total loaded+skipped entries
- Markers: `--- rules:NAME ---` / `--- end rules:NAME ---`
- Deterministic: entries sorted by path before injection

## Tests (11 total)
- T01 empty rules passthrough
- T02 single always-rule with markers
- T03 deterministic sorted order
- T04 conditional rule injected when loaded
- T05 conditional rule skipped when not loaded
- T06 byte cap enforced (over-budget entry skipped)
- T07 determinism across runs
- T08 glob preserved in loaded record
- T09 base prompt after rules
- T10 budget exceeded skip reason
- T11 record size bounded

## RED evidence
- RED sha256 (tests): 728d4ea56280791dcc76136ee951083a8299004678257ff0cd06aea3b427ba2c
- RED sha256 (impl): c457bf2b3d41330fabc1a9d27a2ada0dcd1b30be029bdbd097c9dd22e968e510
- cmd: CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-server --test rules_inject
- result: 0 passed, 11 failed (todo! panic)

## GREEN evidence
- GREEN sha256 (tests): d74015c38ca5766b05896341b1df485d77059a5b4fb03e8464081f13789703f9
- GREEN sha256 (impl): e29026f06cd4e7df93bfb2e122100d8fb080adcd4fafba93669b55692a239a01
- cmd: CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-server --test rules_inject
- result: 11 passed, 0 failed
- no-regress: cargo test -p opencode-rk-server --lib 194 passed

## Decisions
- Import from `opencode_rk_server::` since file is compiled via #[path] in integration test
- INJECT_BYTE_CAP = 128 KiB (generous for rule files, well under TOTAL_BUDGET=512 KiB)
- MAX_RECORD_ENTRIES = 64 (matches rules_loader MAX_FILES)
- InjectRecord types defined independently (same shape as context_report MemoryReport)
- Test T06 corrected: single over-budget entry → skipped (not loaded)

## Integrator follow-ups
- `pub mod rules_inject;` needed in server/lib.rs
- `/MEMORY` (3.4) rendering should use InjectRecord directly
