# FIX-10 scratchpad

Claim: verify scrollback_model_full.rs meets spec.
Source: crates/opentui-bridge/src/scrollback_model_full.rs:1-80 (80 lines).
Observed: ScrollModel {rows: VecDeque<String>}, CAP=500, ROW_CAP=4KiB, push trunc char-boundary safe (is_char_boundary loop), pop_front O(1) evict, window(n)->Vec<String>, len, is_empty, forbid(unsafe_code), std-only, 4 tests.
Target: <=90 lines, >=4 tests, rustfmt clean.
Tests: existing 4 (new_is_empty, push_adds_row, window_returns_last_n, push_evicts_oldest).
Decisions: no edit; all properties hold. Trunc path mirrors scrollback_family.rs:41-50.
Remaining: none.
Verification: `rustfmt --check crates/opentui-bridge/src/scrollback_model_full.rs` -> FMT_OK. `wc -l` 80, `grep -c #[test]` 4.
