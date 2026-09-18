# NET-012-rrec worklog: `crates/server/src/remote_recovery.rs`

## Claim
Recovery types: valid-cursor rebuild vs expired-cursor bounded resync,
pending-command outcome enum (never fake exactly-once), version check,
offline-draft-local-only marker.

## Source evidence (HEAD 5af7884)
- `crates/server/src/event_cursor.rs:40-47` — FNV-1a 64 `digest_of(seq, type)`;
  `:222-241` — `ReplayBuffer::replay`: valid cursor replays durable after
  cursor, stale/ahead/mismatch → `ResyncRequired`; `:248-250` — cap 512.
- `crates/server/src/remote_sync.rs:70-76,107-123` — backoff + connect/resync
  queued events; no command-outcome type exists (gap this file fills).
- `crates/server/src/sync_log.rs:140-147` — dedup by event id (no dup append).
- `crates/server/src/remote_ledger.rs:62-80` — upsert-by-name precedent.

## Target boundary
OWNED FILE ONLY: `crates/server/src/remote_recovery.rs`. No other edits.
`#![forbid(unsafe_code)]`, std only (`std::fmt`), no I/O/clock/threads.

## Tests (7, frozen in-file `#[cfg(test)]`)
- valid cursor rebuilds same session, catch-up replay empty (T01).
- expired/ahead/tampered cursor → `Resync{invalidate_ui:true, bound<=512}`;
  malformed → `BadCursor` (T02).
- uncertain never committed/settled; reject-with-ack stays rejected (T03).
- committed requires `ack_seq>0` proof (T03 ack half).
- `check_version`: equal → `actual+1`; stale → `VersionConflict` (T04).
- sensitive draft `can_auto_replay=false` under valid AND expired
  authority; plain draft replays only while authority valid (T05).
- draft store upserts, preserves on failed store, rejects bad id/oversize.

## Decisions
- Self-contained (own `Cursor`/`StoredEvent`/`digest_of`) so file compiles
  as single `rustc --test` target; no crate imports, no `lib.rs` edit.
- Ephemeral exclusion list mirrors `event_cursor::classify` durable set.
- `check_version` returns next version `actual+1` saturating.
- Upsert-replace in `DraftStore::store` accounts byte budget before swap.

## Remaining unknowns
- Integration wiring into `lib.rs`/control-plane replay path owned by
  integrator, not this lane.

## Evidence
- `rustc --edition 2021 --test crates/server/src/remote_recovery.rs -o /tmp/opencode/rr && /tmp/opencode/rr` → 7 passed, 0 failed.
