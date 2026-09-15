# SYNC-002 - Persisted versus ephemeral part-event split

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-043.
Dependencies: none newly introduced by this slice.
Test obligations: SYNC-002-T01, SYNC-002-T02, SYNC-002-T03, SYNC-002-T04, SYNC-002-T05.
Ownership locks: crates/sessions/src/part_events.rs only; worker never edits shared lib.rs, Cargo.toml, schemas, migrations.
Suggested module: crates/sessions/src/part_events.rs (new module in crate opencode-rk-sessions; lib.rs wiring left to integrator).

## User-observable outcome

Callers updating one session observe the persisted-versus-ephemeral invariant: message/part updates and removals are durable operations returning a persisted record the caller must store, while streaming part deltas are ephemeral hints that never touch durability and are safely droppable at any time. A compacted-context filter drops compacted attachments from provider-bound conversion without mutating stored parts.

## Source evidence

- Observed upstream checkout `07619a09d6da7945ea3fdbb4f8ae5ce8dc2b6eeb` versus plan pin `95daf90670b7c039c436c85537da5fbfe2205b41` (PLAN.md:36, sources/upstream.lock.json:9); never equated; pinned-blob reconciliation outstanding before parity claims.
- worklog/UPSTREAM-V2-sessions-agents.md: `session/message-v2.ts:90-353` defines snapshot, patch, text, reasoning, file, agent, compaction, subtask, retry, step-start, step-finish, and tool parts; tool states at `:276-342` are pending, running, completed, error with bounded metadata/attachments expected by caller.
- worklog/UPSTREAM-V2-sessions-agents.md: events at `:460-509` distinguish persisted message/part updates and removals from ephemeral part deltas; this persisted-versus-ephemeral split is a key parity invariant.
- worklog/UPSTREAM-V2-sessions-agents.md: `:585+` converts stored messages to provider messages, skips empty/ignored content, handles media by provider, removes compacted attachments; `:929-948` filters compacted context.
- worklog/UPSTREAM-V2-core-architecture.md:38: `session/message-v2.ts:27-947` models user/assistant messages and text (comparison point for the part model).
- docs/research/UPSTREAM-V2-PARITY-SYNTHESIS.md section 5 (F4, class partial): local `turn_parts.rs` / `transcript_lane.rs` are comparison points; part-model parity unverified.
- Local absence: `crates/sessions/src/part_events.rs` does not exist (verified by directory listing; sessions lib.rs declares 41 modules, none named part_events).
- Local partial (not re-owned): crates/server/src/turn_parts.rs and transcript_lane.rs cover turn/ transcript projection only, with no persisted/ephemeral classification; crates/sessions/src/events.rs and export.rs are one-line stubs.
- PLAN.md sections 5-6: slice independence rules, mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: lifecycle order, compiling RED, frozen hash, GREEN minimum.
- tasks/WEB-005.md: task-card model mirrored here. tasks/SYNC-001.md: sibling aggregate log (this card owns only the part-event split; SYNC-001 owns sequencing/replay).
- Classification: discovered scope under REQ-043; deliberate safer/resource-bounded deviation (explicit MAX_DELTA_BYTES cap; upstream supplies none).

## Observable contract

- `PartKind { Text, Reasoning, File, Tool, Compaction }`: closed enum; unknown wire kinds rejected, never defaulted.
- `PartUpdate { part_id: String, kind: PartKind, bytes: Vec<u8> }`: part_id 1..=128 chars `[A-Za-z0-9][A-Za-z0-9._-]*`; bytes max 64 KiB.
- `PartEvent { PersistedUpdate(PartUpdate), PersistedRemove { part_id }, EphemeralDelta { part_id, delta_bytes: Vec<u8> } }`: delta_bytes max MAX_DELTA_BYTES.
- `classify(event: &PartEvent) -> Durability { Durable, Ephemeral }`: PersistedUpdate/PersistedRemove => Durable; EphemeralDelta => Ephemeral.
- `apply_update(parts: &mut Vec<StoredPart>, update: PartUpdate) -> Result<(), PartError>`: upserts by part_id; unknown part_id inserts; stored bytes capped per part.
- `apply_remove(parts: &mut Vec<StoredPart>, part_id: &str) -> bool`: removes only that id; unknown id => false, vec otherwise unchanged.
- `apply_delta(deltas: &mut Vec<EphemeralDelta>, delta) -> Result<(), PartError>`: appends to the ephemeral buffer only; never touches `parts`; oversize => `Err(TooLarge)` with buffer unchanged.
- `filter_compacted_for_provider(parts: &[StoredPart]) -> Vec<&StoredPart>`: returns only non-compacted parts in stored order; never mutates the input slice.
- Bounds as explicit public constant: `MAX_DELTA_BYTES: usize = 65_536` (64 KiB) per delta; stored part bytes likewise capped at 64 KiB; total stored parts per call bounded by caller-supplied vec (no module-level growth beyond the passed vec).
- Deterministic: same parts plus same event sequence => byte-identical stored vec and delta buffer; no wall-clock, no I/O, no globals.
- Suggested module boundary: `crates/sessions/src/part_events.rs` owning PartKind, PartUpdate, PartEvent, StoredPart, Durability, PartError, classify, apply_update, apply_remove, apply_delta, filter_compacted_for_provider; shared lib.rs wiring left to integrator.
- Explicitly excluded: aggregate sequencing/replay (SYNC-001), provider message conversion beyond the compacted filter, tool-state machine (pending/running/completed/error), SQLite persistence, network.

## State and persistence transitions

- Stored-part lifecycle: absent -> present (apply_update insert) -> mutated (apply_update upsert) -> absent (apply_remove); each transition is synchronous and caller-observed via the returned vec.
- Ephemeral-delta lifecycle: buffered -> dropped at any time with zero effect on stored parts; drops are always safe and never reported as errors.
- Durability boundary: Durable outcomes must be persisted by the caller (this module performs no I/O); Ephemeral outcomes must never be persisted (no API exists here to persist them).
- Compaction filter: pure projection; input slice unchanged; filtered-out compacted parts remain stored and retrievable.

## Failure states

- Unknown part kind on decode: `Err(PartError::UnknownKind)`; parts vec and delta buffer unchanged.
- Invalid part_id (empty/bad-charset/oversize): `Err(PartError::InvalidInput)`; both collections unchanged.
- Oversize update bytes or delta bytes (> 64 KiB): `Err(PartError::TooLarge)`; target collection unchanged; never partially appended.
- `apply_remove` unknown id: returns `false`; vec unchanged (harmless).
- Ephemeral delta applied to durability path: impossible by type (no conversion function exists); compile-time guarantee asserted by a no-persist test.
- Secret safety: part bytes treated as opaque; errors carry variant names only, never part content. Tests use fixture bytes only; no user DB writes.

## Resource bounds

- Byte caps: every update/delta bounded by 64 KiB; rejected before retention (no unbounded queue/output).
- Count discipline: module grows only caller-owned vecs; no module-level map, cache, or retained history beyond the return values.
- Owner/cancel path: synchronous pure functions, no threads, no detached task; caller owns all lifetimes; cancellation N/A.
- Zero hidden cost: classify/filter allocate only the returned vec; idle cost is zero (no statics).

## Test obligations (frozen)

- SYNC-002-T01 (happy path): update text part => Durable, stored len 1; remove => true, len 0; delta => Ephemeral, stored len still 0, delta buffer len 1.
- SYNC-002-T02 (durability split): `classify` maps PersistedUpdate/PersistedRemove to Durable and EphemeralDelta to Ephemeral; dropping the delta buffer leaves stored parts byte-identical.
- SYNC-002-T03 (compaction filter): parts [text, compacted, file] => filter returns [text, file] in order; input slice length still 3 (no mutation).
- SYNC-002-T04 (failure states): unknown kind => `Err(UnknownKind)`; empty part_id => `Err(InvalidInput)`; 65 KiB+1 delta => `Err(TooLarge)` with buffer unchanged; remove unknown id => false with vec unchanged.
- SYNC-002-T05 (purity + safety): full matrix under I/O-denied harness => `fs_calls == 0`, `env_reads == 0`; captured logs contain zero part bytes; no writes outside the disposable fixture dir.

## TDD steps

1. Inspect: PLAN.md 5-6, TDD.md 2-5, worklogs and synthesis evidence above (done).
2. Contract: defined above.
3. Author tests SYNC-002-T01..T05; establish compiling RED (fail: no part_events module).
4. Freeze test hash + command manifest.
5. Implement minimum native Rust part-event split with compacted filter.
6. GREEN, refactor, rerun; negative tests (unknown-kind, bad-id, oversize-atomic, unknown-remove harmless, delta-never-durable).
7. Evidence + patch; verifier decides acceptance.

## Verification

```bash
git status --short -- tasks/SYNC-002.md
cargo test -p opencode-rk-sessions part_events
cargo check --workspace
```

## Ownership note

Aggregate sequencing/projector replay stays with SYNC-001; provider conversion, tool states, and storage stay with their domain owners. This card specifies the in-memory part-event classification contract only. DISC-003 remains IN PROGRESS / NOT ACCEPTED.
