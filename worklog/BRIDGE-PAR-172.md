# BRIDGE-PAR-172 scratchpad

- Claim: BRIDGE-PAR-172 via completion_claims, session ses_par172. OK.
- Source evidence:
  - TS truth: packages/opencode/src/cli/cmd/run/scrollback.shared.ts:1-92 (entry look/color helpers, no ring buffer; scroll semantics new).
  - Rust prior art: crates/opentui-bridge/src/run_scrollback_shared.rs:1-131 (ScrollStore, CAP 2000, LINE_CAP 4KiB bytes, evict oldest via remove(0)).
- Target boundary: ONE new file crates/opentui-bridge/src/scrollback_shared_full.rs. No lib.rs/Cargo.toml edits. No cargo.
- Design: ScrollbackSharedFull { lines: Vec<String>, offset: usize }; MAX_LINES 500; LINE_CAP_CHARS 2048 chars; offset 0 = pinned bottom; scroll delta>0 = up/older, delta<0 = down/newer, clamp [0, len-height]; visible = window ending at len-offset; push evicts oldest + resets offset 0.
- Tests: evict+reset, clamp top, clamp bottom, visible window, trunc ascii+multibyte, oversize/zero height (6 tests).
- Decisions: char-count truncation (spec says chars); fast byte path; remove(0) matches prior art, fine at cap 500.
- Unknowns: none blocking.
