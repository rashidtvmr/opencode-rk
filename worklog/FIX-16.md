# FIX-16 scratchpad — keymap_chords_full.rs

Claim: attempted via tools/completion_claims.py claim(FIX-16, ses_f26cfc3c1ffezXsZ4X3nTunOOo). FAILED closed: ledger has 502 rows > MAX_ROWS 500, load_ledger raises ClaimError("claims must be a bounded mapping") for ANY op. Pre-existing ledger overflow, not caused by this lane. No ledger write made. Status left unclaimed.

Source evidence:
- crates/opentui-bridge/src/keymap_chords_full.rs (owned, 90 lines post-fix)
- crates/opentui-bridge/src/keymap.rs:100-147 (ModeStack push/pop, read-only ref)
- crates/opentui-bridge/src/keymap_tsx_full.rs (KeymapTsx leader/count, read-only ref)
- crates/opentui-bridge/src/keymap_ts_full.rs (KeymapTs pending buf, read-only ref)
- crates/opentui-bridge/src/lib.rs:481 (keymap_ts_full wired; keymap_chords_full NOT declared as mod)

Observed scenario: file existed at 87 lines, had ChordBuf+push/pending_of/take+resolve and 4 tests, rustfmt clean. Gaps found: (1) push() stored raw case, so ["ESC"] never matched cancel and uppercase chords diverged; (2) resolve() had NO esc/escape=>cancel arm despite task contract requiring Esc cancel — cancel path missing entirely; (3) resolve() used slice-pattern ["ctrl","t"] literal matching which also broke on non-lowercased input; (4) doc comment omitted Esc-cancel and divergence detail.

Target boundary: ONE file keymap_chords_full.rs only. No lib.rs/Cargo.toml/claims.json/keymap.rs edits. No cargo runs.

Tests: 5 tests in-file (empty_resolves_none, full_chords_resolve, esc_cancels_and_drains [NEW], partial_is_pending, overflow_and_drain). Frozen-test discipline: pre-existing 4 tests kept green-compatible (overflow test asserts take().len only; esc test asserts new cancel behavior). No test edits to other files.

Decisions:
- push lowercases via to_ascii_lowercase (bounded, std-only).
- resolve via match on as_slice with guards; esc/escape=>cancel; empty=>none; else pending.
- LIFO divergence + leader upgrade documented in 1-line doc comment (cap 8 fixed; upgrade via pending_count cf KeymapTsx). References keymap_tsx_full.rs KeymapTsx and keymap.rs ModeStack/LEADER_TIMEOUT_DEFAULT_MS as upgrade anchors.
- Line budget: 90 lines exactly (<=90), rustfmt applied (expanded pending_of/take back to multi-line).

Remaining unknowns / gaps for orchestrator:
- UNWIRED: lib.rs has no `pub mod keymap_chords_full;` (checked lib.rs:39-42,135,227,270,330,356,462,481-484). Lane forbidden from editing lib.rs, so file is not compiled into crate. Integration lane must add the mod declaration.
- LEDGER FULL: claims.json 502 rows exceeds MAX_ROWS 500; all claim/update ops fail closed until orchestrator prunes. FIX-16 could not be claimed; treat as blocked-on-ledger, file work complete.
- No cargo run per task constraints; only rustfmt --check (PASS) executed.
