# SYNC-001 scratchpad — versioned sync event log with projector replay

## Claim
- Task SYNC-001 claimed by ses_f387af8b7ffeg2GsF4ErYq07AX, scratchpad worklog/SYNC-001.md, status in-progress → completed on GREEN.

## Source evidence
- tasks/SYNC-001.md: contract, failure states, resource bounds, T01–T05 obligations.
- crates/server/src/sync_log.rs (pre-existing impl, found at HEAD 621ec4f, last touched 1be93d3): SyncEvent/SequencedEvent/SyncLog/SyncError, MAX_SYNC_EVENTS=4096, MAX_PROJECTORS=32, MAX_PAYLOAD_BYTES=64KiB.
- crates/server/tests/sync_log.rs (frozen, untouched): T01–T05.
- crates/server/src/lib.rs:48 `pub mod sync_log;` — wiring already present, no integrator gap.

## Observed scenario
- Focused suite `cargo test -p opencode-rk-server --test sync_log`: 5 passed, 0 failed. Zero writes to impl or tests needed.

## Target boundary inspection (owned file vs contract)
- Sequencing: `append` assigns seq = len+1 from 1; rejects consume no seq. OK.
- Idempotence: duplicate event_id early-returns original seq before fan-out; len unchanged. OK.
- Freeze: one-way `freeze()` + `is_frozen()`; post-freeze unknown type → UnknownType, known types still admitted via seen_types. OK.
- Projector cap: 32, TooManyProjectors, count unchanged. OK. Fan-out is broadcast in registration order, exactly once per event — matches "each see every event".
- Replay cursor: unknown index → UnknownProjector; from_seq==0 or beyond head → BadCursor; head (len+1, non-empty) → Ok(0). Branching convoluted but behavior correct per T01/T04/T05.
- Validation: token charset/length (id ≤128, type ≤64), payload ≤64KiB → InvalidInput, log unchanged. OK.
- Bounded memory: count caps + byte cap checked before insert; new() allocates empty vecs only; sync, no threads/IO/clock; forbid(unsafe_code). OK.
- Safety: Debug for SyncLog prints counts only; Display errors variant names only. OK.

## Tests
- Exact: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120 rtk cargo test -p opencode-rk-server --test sync_log` → 5 passed.
- No RED phase authored: impl + frozen tests pre-existed green; no test edits (frozen file byte-identical).

## Decisions
- No code change: impl already satisfies full contract; minimal-diff principle.
- Wiring gap: none — `pub mod sync_log` present in server lib.rs at HEAD.

## Remaining unknowns
- None in-slice. Cross-type projector filtering intentionally absent (broadcast per outcome statement). SYNC-002/WEB-005/WSX-002 exclusions untouched.
