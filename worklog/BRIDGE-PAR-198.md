# BRIDGE-PAR-198 scratchpad

Claim: BRIDGE-PAR-198 via cc.claim session ses_par198, scratchpad worklog/BRIDGE-PAR-198.md. Status in-progress.

Source evidence:
- crates/opentui-bridge/src/scrollback_shared_full.rs:8 MAX_LINES=500, :10 LINE_CAP_CHARS=2*1024, :13-17 ScrollbackSharedFull struct, :25-31 push evict-oldest, :55-63 truncate by chars.
- crates/opentui-bridge/src/native_frame.rs:17 MAX_NATIVE_TRANSCRIPT=500, :37 saturating_sub drain cap.

Observed scenario: new file only, read truth, no edits to lib.rs/Cargo.toml/scrollback_shared_full.rs/transcript.rs.

Target boundary: ONE file crates/opentui-bridge/src/transcript_store.rs, std-only, forbid(unsafe_code), <130 lines, TranscriptStore {lines: Vec<String>} cap 500 + 2KiB chars/line, push evict oldest, len/clear, tail(n)->&[String], as_vec()->Vec<String>, >=5 tests.

Tests written: 6 in-file (push_and_len, evicts_oldest_at_cap, truncates_to_2kib_chars, clear_empties, tail_returns_newest_n, as_vec_clones_all). Unrun per scope (no cargo).

Decisions: `pub lines` per spec shape; truncate by chars().count like truth; is_empty helper (len==0 idiom, zero cost).

Remaining unknowns: none. Integrator prewires `pub mod transcript_store;`.
