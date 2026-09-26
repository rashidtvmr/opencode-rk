# SYNC-002 — persisted-versus-ephemeral part-event split

- Claim: `cc.claim(...,'SYNC-002','ses_f384fedd2ffeFXapK9O5i12eT7','worklog/SYNC-002.md')` OK (no collision; `SYNC-002` absent from ledger).
- Owned file: `crates/sessions/src/part_events.rs` only. Scratchpad + own ledger row only.

## Source evidence
- Task card `tasks/SYNC-002.md` (REQ-043): contract, bounds, failure states, T01–T05.
- Impl `crates/sessions/src/part_events.rs:1-182` (pre-landed in `1be93d3`, author = prior lane; zero writes by this lane).
- Frozen tests `crates/sessions/tests/part_events.rs:1-149` (sha256 `1a585f56...`), untouched.
- Wiring pre-existing: `crates/sessions/src/lib.rs:47` `pub mod part_events;` (integrator-owned, not touched).
- PLAN.md:36 pin `95daf90...` vs observed upstream `07619a0...`: never equated; parity claims out of scope.

## Observed scenario
- Target boundary: pure caller-owned in-memory classification; no I/O, clock, threads, globals (grep for `std::fs|std::env|std::net|tokio|SystemTime` → zero hits; `#![forbid(unsafe_code)]`).
- No RED run by this lane: module + frozen tests both pre-landed green at HEAD. Honest gap: RED receipt belongs to prior lane (`1be93d3` "land 82-task evidence waves"). No test edits made here, so no new RED/GREEN cycle was required — verification only.
- Full contract check vs impl: `PartKind{Text,Reasoning,File,Tool,Compaction}` + `decode_kind` rejects unknown (`UnknownKind`); id rule 1..=128 `[A-Za-z0-9][A-Za-z0-9._-]*` (`valid_id:116`); 64 KiB caps via `MAX_DELTA_BYTES`/`MAX_PART_BYTES` (`:8-10`); `classify` Durable/Ephemeral (`:109`); `apply_update` upsert (`:129`); `apply_remove` single-id, unknown→false (`:153`); `apply_delta` ephemeral-only, oversize→`TooLarge` unchanged (`:164`); `filter_compacted_for_provider` pure projection (`:180`); `StoredPart` Debug redacts bytes→len (`:67`), `PartError` carries no content.
- Note: `PartUpdate`/`StoredPart` carry a `compacted: bool` field beyond the card's 3-field sketch; frozen T01–T03 helpers require it, so it is contract, not drift.

## Tests
- cmd: `cargo test -p opencode-rk-sessions --test part_events -- --test-threads=1`
- result: 5/5 GREEN (`sync002_t01..t05 ... ok`, 0 failed). Zero test edits (`shasum` frozen test file unchanged `1a585f56...`).
- `cargo check -p opencode-rk-sessions`: Finished, no errors (5 pre-existing lib warnings: unused MAX_SESSIONS/MAX_MESSAGES/PAGE_SIZE etc., outside ownership).
- No user DB touched; T05 uses `tempfile::tempdir()` disposable fixture only.

## Decisions
- No code change: implementation already satisfies the full observable contract verbatim. Writing a diff for its own sake would violate minimal-diff discipline.
- `completed` is truthful: real no-stub code present, frozen green, zero test edits, ownership respected.

## Remaining unknowns / handoff
- Persistence caller seam: `Durable` outcomes must be persisted by the caller — no caller wiring exists in this lane's scope (module performs no I/O by design). Integrator/verifier owns wiring `apply_update`/`apply_remove` results into SQLite persistence.
- RED receipt for this file lives with prior landing `1be93d3`; verifier re-runs frozen suite on integrated revision.
- Pre-existing warnings in `opencode-rk-sessions` lib (unused consts) are outside this lane.
