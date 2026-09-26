# HEAD-002 — Deterministic redacted session export

Claim: HEAD-002 completed, session ses_f384fedf9ffeu54QwccVGqCbzv, scratchpad worklog/HEAD-002.md.
Source evidence:
- tasks/HEAD-002.md:1-89 (contract T01-T05, MissingId/TooLarge/NotFound/StoreFailed/InvalidId, EXPORT_MAX_BYTES 16MiB, atomic zero-byte, *** redaction, deterministic first-seen key order).
- crates/cli/tests/session_export.rs:1-260 frozen, 8 tests, zero edits. sha256 e213d1bde7b749ef9a39926085393044ca0471d733f87adbcc8710085247f7bd.
- crates/cli/src/session_export.rs:1-191 owned impl. sha256 c3d8aaa77351ccef9e6cabe88de399903a50ee34d8b5a3e418930b5e73e59113. No stubs/todo/unimplemented; zero diff (already GREEN at HEAD).
Observed: `cargo test -p opencode-rk-cli --test session_export` blocked pre-existing opencode-rk-opentui-bridge native lib missing (aarch64-apple-darwin). Standalone harness `rustc --edition 2021 --test crates/cli/tests/session_export.rs` 8/8 GREEN (T01-T05 + unknown/store-fail/overlong), --test-threads=1.
Target boundary: owned session_export.rs + scratchpad + ledger row only. No frozen-test/lib/Cargo/schema edits.
Failure states verified: MissingId zero store/sink; NotFound sink untouched; TooLarge estimated>16MiB zero bytes; StoreFailed untouched; InvalidId >128 chars pre-store; errors Debug/Display leak-free.
Ownership/persistence/bounds: caller-owned Store/Sink/Opts traits, sync, no threads/statics/cache/network/fs/picker; output lives only in caller sink; single push on success; EXPORT_MAX_BYTES estimated pre-write.
CLI wiring seam: none — tests include module via #[path]; lib.rs/mod wiring left to integrator per card.
