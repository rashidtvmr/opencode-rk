# APP-007-cursor worklog

## Claim
Own `crates/server/src/event_cursor.rs` only: replay cursor (`seq`+digest),
resync-required error, ephemeral-vs-durable classifier, 512 bound, slow-client
disconnect helper. std only, `forbid(unsafe_code)`.

## Source evidence (HEAD 5af7884)
- `crates/server/src/event_bus.rs:1-30` — fan-out bus, no cursor history.
- `crates/server/src/event_stream.rs:1-372` — SSE envelope; per-subscriber
  bounded FIFO, slow-consumer disconnect, no durable replay.
- `crates/server/src/sync_log.rs:1-202` — versioned log; `replay(idx,from_seq)`
  but seq-only cursor, no digest, no durability classifier, cap 4096.
- `crates/server/src/app_protocols.rs:234-326` — `EventCursor(u64)` seq-only,
  `check_cursor` (BadCursor zero / ResyncRequired stale-or-ahead),
  `classify` 8 durable types; mirrored here with added chain digest.
- `crates/contracts/src/lib.rs:270-280` — `EventCursor(u64)` wire shape.
- APP-007 card (completion/local.json): 5 tests T01..T05; this lane covers the
  cursor/type slice (once-delivery, expired→resync, ephemeral≠durable,
  bounded 512 + slow-disconnect).

## Observed scenario
RED run (`rustc --edition 2021 --test ... -o /tmp/opencode/ec`): 4 fail —
replay returned empty, no BadCursor/ResyncRequired enforcement. GREEN after
implementing `replay` with digest-checked cursor + durable-only filter:
5 passed, 0 failed.

## Target boundary
`Cursor` (seq+digest, genesis), `digest_of` (FNV-1a 64 chain check, NOT a
cryptographic integrity proof), `CursorError::{BadCursor,ResyncRequired}`,
`Durability` + total `classify`, `StoredEvent`, `ReplayBuffer` (FIFO cap 512,
seq from 1), `REPLAY_BUFFER_CAP=512`, `slow_client_should_disconnect`.
Crash-recovery between commit and publish stays with sync_log owner, not here.

## Tests
`rustc --edition 2021 --test crates/server/src/event_cursor.rs -o /tmp/opencode/ec && /tmp/opencode/ec`
- replay-once, expired→resync, ephemeral-never-durable, slow-disconnect-bound,
  malformed-cursor→BadCursor. No cargo build per lane instructions.

## Decisions
Digest = FNV-1a 64, std only, no new dep; marked `ponytail:` ceiling for
cross-trust-boundary upgrade. Eviction = oldest-first; stale cursor fails
closed to snapshot instead of gapped replay.

## Remaining unknowns
Wiring into `lib.rs` / `event_stream` / `sync_log` owned by integrator.
Snapshot format for resync decided by parent slice.
