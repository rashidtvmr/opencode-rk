# SHARE-002

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: none (DISC-002 discovered; scope described by behavior-surface-rules.json).
Dependencies: none.
Test obligations: SHARE-002-T01, SHARE-002-T02, SHARE-002-T03, SHARE-002-T04, SHARE-002-T05.

## User-observable outcome

Bounded local share-event coalescing queue: location-filtered session/message/part/diff/deletion events collapse to latest-value-per-data-key; a delayed flush drains the batch once; scoped finalization clears queue and cache. Failed flush retains the batch for retry instead of dropping it. No network, no DB mutation, no secret logging.

## Source evidence

- Pinned OpenCode commit `95daf90670b7c039c436c85537da5fbfe2205b41`.
- `packages/opencode/src/share/share-next.ts:112-204` (blob `60112e10d964b3d9693727501e1e40dac6b1fdf1`): location-filtered subscription, latest-value-per-data-key queue, delayed sync, scoped finalizer (upstream failure path logs-and-drops; this slice deliberately retains on failure).
- `packages/opencode/test/share/share-next.test.ts:227-324` (blob `fe036be8c959b889516a894266bf4bb390ce1827`): direct coalescing test matrix mirrored below.
- `sources/sharing-ownership-gap.json`: partition `share-event-subscription-and-coalescing-queue` (`boundEstablished: false`), candidate fragment `share-event-coalescing`; unresolved `event-subscription-coalescing-backpressure-and-retry-lifetime`.
- `sources/disc-003-reconciliation.json` finding `opencode.sharing` (partial): queue has no explicit item/byte cap upstream; native contract must define bounded queue/retention/backpressure/slow-client behavior before any subscriber or network sync implementation.
- PLAN.md sections 5-6: slice independence rules, mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: lifecycle order, compiling RED, frozen hash, GREEN minimum.
- tasks/TOOL-016.md: task-card model (Status/Kind/contract/test obligations).
- Classification: partial/planned upstream (coalescing shape exists; bounds + retain-on-failure are new), deliberate resource-bounded deviation (item/byte/session caps, retain-then-retry instead of drop-on-failure).

## Observable contract

- `ShareEvent { session: SessionId, key: DataKey, value: CanonicalJson }`; `DataKey = (kind, id)`; location filter `accepts(session) -> bool` supplied by caller.
- `CoalescingQueue::new(QueueCaps { max_items: 4096, max_bytes: 8_388_608, max_sessions: 256 })`; `push(event)` keeps latest value per key, evicts oldest key on overflow with `evicted: u64` count; non-accepted locations dropped with `filtered: u64` count.
- `drain() -> Vec<ShareEvent>` returns pending batch sorted by `(session, kind, id)` and clears the queue exactly once (idempotent second drain is empty).
- `finalize()` clears queue + cache; no further push accepted after finalize (`Err(QueueError::Finalized)`).
- Deterministic: same push sequence => identical drain order; no wall-clock in ordering (delay is caller-driven, not embedded).
- Suggested module boundary: `crates/share/src/queue.rs` (crate `opencode-rk-share`); worker ships additive fragment only, never edits shared `lib.rs`, `Cargo.toml`, schemas, migrations.

## Failure states

- Overflow: oldest key evicted, `evicted += 1`; queue length and bytes never exceed caps.
- Failed downstream flush: caller keeps the drained batch and calls `requeue(batch)`; entries are not lost (deliberate deviation from upstream drop-on-failure).
- Finalized queue: `push`/`requeue` => `Err(QueueError::Finalized)`; no state change.
- Oversized single event (> `max_bytes / 8`): rejected with `Err(QueueError::TooLarge)`; queue unchanged.
- Secret safety: event values carry no secrets by contract; queue never logs values, only keys and counts. No SQLite/OpenCode DB writes. No changes to the user's existing OpenCode database; tests use disposable in-memory fixtures only.

## Resource bounds

- Item cap `max_items`, byte cap `max_bytes` (summed value bytes), session cap `max_sessions`; no unbounded queue of pending events.
- One call-local queue object; single owner; no thread, no timer, no background task (caller schedules the delayed flush).
- `drain`/`finalize` reclaim all entries promptly; dropped queue frees all memory; no detached scope without an owner.
- Zero hidden cost when unused: construction allocates nothing beyond empty maps.

## Test obligations (frozen)

- SHARE-002-T01 (coalescing happy path): push 5 events with 2 duplicate keys: `assert_eq!(queue.len(), 3)`, latest value wins per key, `drain()` returns 3 sorted by `(session, kind, id)` and second `drain()` is empty.
- SHARE-002-T02 (determinism + location filter): same push sequence twice => identical drains; non-accepted location events dropped with `assert_eq!(filtered, 2)` and absent from drain.
- SHARE-002-T03 (bounds): fill beyond `max_items`/`max_bytes`: `assert!(evicted >= 1)`, `assert!(queue.len() <= max_items)`, `assert!(queue.bytes() <= max_bytes)`; oversized single event => `assert_eq!(err, TooLarge)`.
- SHARE-002-T04 (retry + finalize): `drain()` then `requeue(batch)` => batch present again in full; `finalize()` => queue empty and `push` returns `assert_eq!(err, Finalized)`.
- SHARE-002-T05 (safety + no side effects): no test writes outside disposable fixture dir (assert DB/fixture-outside untouched); captured logs contain zero event-value bytes; dropped queue frees memory (no retained entries observable after drop).

## TDD steps

1. Inspect: PLAN.md 5-6, TDD.md 2-5, gap files + disc-003 `opencode.sharing` finding (done, see evidence).
2. Contract: defined above.
3. Author tests SHARE-002-T01..T05; establish compiling RED (fail: no queue module).
4. Freeze test hash + command manifest.
5. Implement minimum native Rust coalescing queue.
6. GREEN, refactor, rerun; negative tests (overflow, oversized event, finalized push, requeue round-trip).
7. Evidence + patch; verifier decides acceptance.

## Verification

```bash
git status --short -- tasks/SHARE-002.md
cargo test -p opencode-rk-share queue
cargo check --workspace
```
