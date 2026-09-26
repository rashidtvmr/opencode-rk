# BRIDGE-GAP-28 scratchpad (ses_gap28)

- Claim: BRIDGE-GAP-28 via tools/completion_claims.py, session ses_gap28. Status: claimed.
- Source evidence: BRIDGE_MIGRATION_DETAIL.md:279 cites `FooterApi/StreamCommit/FooterView` as part of run/* gap (writeSessionOutput family). No `run/stream.ts` in repo (find returned nothing). No existing `StreamCommit`/`StreamBuf` symbols in .rs/.ts sources (grep hit only the migration doc).
- Observed scenario: nothing to port line-by-line; greenfield boundary type per lane spec.
- Target boundary: ONE new file `crates/opentui-bridge/src/run_stream.rs`, no lib.rs/cargo/commit edits.
- Tests: 6 unit tests in-file (commit text, empty none, oversize err, no-partial on reject, id increments, cap boundary).
- Decisions: `push(&str) -> Result<(), StreamError>` with `StreamError::Oversize`; `commit() -> Option<StreamCommit>`; id `c-<n>` from 1-based counter; cap 64 KiB = 65536 bytes on pending byte len.
- Remaining unknowns: none for lane scope. Wiring into callers/lib.rs is orchestrator's job.
