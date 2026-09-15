# SYNC-001 - Versioned sync event log with projector replay

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-043.
Dependencies: none newly introduced by this slice.
Test obligations: SYNC-001-T01, SYNC-001-T02, SYNC-001-T03, SYNC-001-T04, SYNC-001-T05.
Ownership locks: crates/server/src/sync_log.rs only; worker never edits shared lib.rs, Cargo.toml, schemas, migrations.
Suggested module: crates/server/src/sync_log.rs (new module in crate opencode-rk-server; lib.rs wiring left to integrator).

## User-observable outcome

Clients of one caller-owned sync log observe a versioned, sequenced event stream: every admitted event carries a monotonic sequence number, duplicate submissions are idempotent no-ops, up to 32 registered projectors each see every event exactly once in sequence order, a frozen log rejects new event-type definitions, and any projector can rebuild its state by replaying from sequence zero. No network, no persistence, no background task.

## Source evidence

- Observed upstream checkout `07619a09d6da7945ea3fdbb4f8ae5ce8dc2b6eeb` versus plan pin `95daf90670b7c039c436c85537da5fbfe2205b41` (PLAN.md:36, sources/upstream.lock.json:9); never equated; pinned-blob reconciliation outstanding before parity claims.
- worklog/UPSTREAM-V2-core-architecture.md:61: `sync/index.ts:11-265` defines versioned sync types, sequence allocation, projectors, idempotence; old versions, gaps, missing projectors, post-freeze definitions fail.
- worklog/UPSTREAM-V2-core-architecture.md:67: `session/projectors.ts:65-154` projects sync creates/updates/deletes, tolerates late foreign-key updates with warnings.
- worklog/UPSTREAM-V2-sessions-agents.md:40: projectors persist creates/updates/deletes and tolerate late FK updates with warnings; persisted-versus-ephemeral split is a key parity invariant.
- worklog/UPSTREAM-V2-web-acp-integrations.md: sync event types, projectors, sequence allocation, replay, idempotence, event-log gating behind the experimental workspace flag.
- docs/research/UPSTREAM-V2-PARITY-SYNTHESIS.md section 4 (F9, class unverified): local `remote_ledger.rs` / `control_decode.rs` suggest ledger/decode scaffolds; projector/replay/idempotence parity unverified.
- Local absence: `crates/server/src/sync_log.rs` does not exist (verified by directory listing; server lib.rs declares 25 modules, none named sync_log).
- Local partial (not re-owned): crates/server/src/remote_ledger.rs is a pure in-memory pushed-refs ledger with no event types, sequences, projectors, or replay; crates/server/src/control_decode.rs is a decode scaffold only.
- PLAN.md sections 5-6: slice independence rules, mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: lifecycle order, compiling RED, frozen hash, GREEN minimum.
- tasks/WEB-005.md: task-card model mirrored here. tasks/SYNC-002.md: sibling part-event split (this card owns only the aggregate log; SYNC-002 owns part-event classification).
- Classification: discovered scope under REQ-043; deliberate safer/resource-bounded deviation (explicit MAX_SYNC_EVENTS and MAX_PROJECTORS caps; upstream supplies none).

## Observable contract

- `SyncEvent { event_id: String, event_type: String, payload: Vec<u8> }`: event_id 1..=128 chars `[A-Za-z0-9][A-Za-z0-9._-]*`; event_type 1..=64 chars same charset; payload max 64 KiB.
- `SequencedEvent { seq: u64, event: SyncEvent }`: seq allocated monotonically from 1 per log; never reused, never zero.
- `Projector = dyn Fn(&SequencedEvent)`: caller-supplied pure sink; log calls projectors in registration order, exactly once per event per projector, in seq order.
- `SyncLog::new() -> SyncLog` (empty, no I/O, no threads).
  - `append(&mut self, event: SyncEvent) -> Result<u64, SyncError>`: duplicate event_id returns the original seq as `Ok` without re-emitting to projectors; new event_id assigns next seq, appends, fans out to all projectors. Unknown event_type with no registered definition when definitions frozen => `Err(UnknownType)`; log unchanged.
  - `register_projector(&mut self, event_type: &str, projector: Projector) -> Result<(), SyncError>`: more than MAX_PROJECTORS => `Err(TooManyProjectors)`; registration unchanged.
  - `replay(&self, projector_index: usize, from_seq: u64) -> Result<usize, SyncError>`: re-invokes that projector over stored events with seq >= from_seq in order; returns count replayed; unknown index => `Err(UnknownProjector)`, nothing invoked.
  - `freeze(&mut self)`: after freeze, `append` with a previously unseen event_type => `Err(UnknownType)`; previously seen types still append.
- Bounds as explicit public constants: `MAX_SYNC_EVENTS: usize = 4096`, `MAX_PROJECTORS: usize = 32`.
- Deterministic: same append sequence => identical seqs and identical per-projector delivery order; no wall-clock, no I/O, no globals.
- Suggested module boundary: `crates/server/src/sync_log.rs` owning SyncEvent, SequencedEvent, SyncLog, SyncError, MAX_SYNC_EVENTS, MAX_PROJECTORS, append/register_projector/replay/freeze; shared lib.rs wiring left to integrator.
- Explicitly excluded: persisted-versus-ephemeral part-event classification (SYNC-002), network transport, SQLite persistence, SSE delivery (WEB-005), background sync loop (WSX-002).

## State and persistence transitions

- Log states: `Open -> Frozen` via `freeze()`; one-way, no unfreeze; freeze is observable via `is_frozen()`.
- Event lifecycle: admitted (seq assigned, stored, fanned out) or rejected (no seq consumed, no storage, no projector call); rejected appends never leave partial state.
- Projector delivery state per projector: `next_expected_seq` implicit in replay cursor only; the log retains no per-projector cursor between calls except what replay recomputes from stored events.
- Persistence: none in this slice; the log is caller-owned memory. Durable sequencing across restarts is explicitly out of scope and must fail closed if attempted (no file/DB path exists in this module).

## Failure states

- Duplicate event_id: returns original seq `Ok`; log length unchanged; projectors not re-invoked (idempotent, asserted by delivery-count test).
- Log full (`len == MAX_SYNC_EVENTS`): `Err(SyncError::Full)`; log unchanged; no seq consumed.
- Empty/bad-charset/oversize id, type, or payload: `Err(SyncError::InvalidInput)`; log unchanged.
- Post-freeze unknown event_type: `Err(SyncError::UnknownType)`; log unchanged; known types still admitted.
- Projector cap exceeded: `Err(SyncError::TooManyProjectors)`; registration set unchanged.
- Replay with unknown projector index or from_seq beyond head: `Err(SyncError::UnknownProjector)` / `Err(SyncError::BadCursor)`; no projector invoked.
- Secret safety: payloads are opaque bytes, never logged; errors carry variant names only. Tests use fixture event bytes only; no user DB writes.

## Resource bounds

- Count caps: at most MAX_SYNC_EVENTS stored events; at most MAX_PROJECTORS projectors; `append` cap-checks before insert; no unbounded queue.
- Byte caps: payload max 64 KiB per event; retained bytes bounded by MAX_SYNC_EVENTS * 64 KiB worst case; no unbounded retained output.
- Owner/cancel path: caller owns the log and all projector closures; all methods synchronous on `&mut self`/`&self`, spawn no thread; no cancellation token needed (sub-microsecond ops); no detached task, no background subscriber, no network handle.
- Zero hidden cost: `new()` allocates only the empty vecs; idle cost is the caller-owned struct only.

## Test obligations (frozen)

- SYNC-001-T01 (happy path): append 3 distinct events => seqs `[1,2,3]`; single projector receives all 3 in order (`assert_eq!(seen_seqs, vec![1,2,3])`); `replay(0, 1)` returns 3.
- SYNC-001-T02 (idempotence + freeze): re-append event 1 id => returns seq 1, log len still 3, projector delivery count unchanged; `freeze()` then append unknown type => `Err(UnknownType)` with len unchanged; append known type still `Ok(4)`.
- SYNC-001-T03 (caps, log unchanged): fill to MAX_SYNC_EVENTS then one more => `Err(Full)` and len stays MAX_SYNC_EVENTS; register 33rd projector => `Err(TooManyProjectors)` and projector count stays 32.
- SYNC-001-T04 (validation + replay cursor): empty event_id => `Err(InvalidInput)`; 65 KiB+1 payload => `Err(InvalidInput)`; `replay(99, 1)` => `Err(UnknownProjector)` with zero invocations; `replay(0, 9999)` => `Err(BadCursor)`; each error leaves stored seqs byte-identical to before.
- SYNC-001-T05 (purity + safety): full matrix under I/O-denied harness => `fs_calls == 0`, `net_calls == 0`; captured logs contain zero payload bytes; projector call count equals exactly appends + replays (no hidden re-emission).

## TDD steps

1. Inspect: PLAN.md 5-6, TDD.md 2-5, worklogs and synthesis evidence above (done).
2. Contract: defined above.
3. Author tests SYNC-001-T01..T05; establish compiling RED (fail: no sync_log module).
4. Freeze test hash + command manifest.
5. Implement minimum native Rust versioned sync log with projector replay.
6. GREEN, refactor, rerun; negative tests (duplicate no-re-emit, post-freeze reject, full-atomic, bad-cursor no-invoke).
7. Evidence + patch; verifier decides acceptance.

## Verification

```bash
git status --short -- tasks/SYNC-001.md
cargo test -p opencode-rk-server sync_log
cargo check --workspace
```

## Ownership note

Part-event persisted/ephemeral classification stays with SYNC-002; wire/SSE delivery stays with WEB-005; remote sync loop stays with WSX-002. This card specifies the in-memory aggregate log contract only. DISC-003 remains IN PROGRESS / NOT ACCEPTED.
