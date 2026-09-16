# SHARE-002 — bounded share-event coalescing queue

## Claim
Own `crates/sessions/src/share_queue.rs` (lane-variant; task-card `crates/share/src/queue.rs` never existed — no `opencode-rk-share` crate). Pure coalescing queue: latest-value-per-`(session,kind,id)`, location filter, delayed `drain`, retain-on-failure `requeue`, scoped `finalize`. No test/lib.rs edits this session.

## Source evidence (pin 95daf90670b7c039c436c85537da5fbfe2205b41)
- `packages/opencode/src/share/share-next.ts:112-204` (blob `60112e10…` per card): subscription + latest-value queue + delayed sync + scoped finalizer. Deliberate deviation: upstream drops on failure; this slice retains via `requeue`.
- `packages/opencode/test/share/share-next.test.ts:227-324`: coalescing matrix mirrored in T01..T04.
- `disc-003 opencode.sharing` (partial): upstream has no item/byte cap; native contract defines `QueueCaps{4096/8MiB/256}` + evict-oldest + `TooLarge` + retain-then-retry.
- Classification: partial/planned upstream, bounded deviation.

## Observed scenario
- `src/share_queue.rs` header declares SHARE-002 ownership, `#![forbid(unsafe_code)]`, `#[path]`-included by `tests/share_queue.rs` (NOT `pub mod` in `lib.rs` — `lib.rs:23-25` wires only merge/policy/queue? verify: grep shows `share_merge/share_policy/share_queue` as `pub mod`; test header says NOT wired — drift noted below).
- `ShareEvent{session,key:DataKey(kind,id),value:Vec<u8>}`; manual `Debug` renders session+key+`value_len` only (redaction-lane fix, owned by SHARE-001 bundle lane).
- `CoalescingQueue::new(CAPS, accepts)`, `push` latest-wins + evict-oldest (`evicted`), non-accepted dropped (`filtered`), `drain` sorted `(session,kind,id)` idempotent, `finalize` empties and rejects push/requeue with `Finalized`, oversize single event `TooLarge` (>max_bytes/8).

## Target boundary
- Pure: no network/DB/clock/thread/timer/global. Caller owns filter fn, batch, retry scheduling.
- Caps: `max_items 4096 / max_bytes 8_388_608 / max_sessions 256`. `drain`/`finalize`/drop reclaim all.
- No secret logging (keys+counts only); values carry no secrets by contract; in-memory fixtures only.

## Tests (frozen, verify-only rerun)
- Suites: `tests/share_queue.rs` (primary, `#[path]` src) + `tests/share_queue_lane.rs` (lane mirror). 5 tests each, T01 coalesce+drain / T02 determinism+filter / T03 bounds / T04 requeue+finalize / T05 safety+no-side-effects.
- GREEN (2026-09-16, `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`, rev `248f519` + workdir dirt, serial, `timeout 120 rtk`):
  `share_queue` 5/5, `share_queue_lane` 5/5 (each `cargo test -p opencode-rk-sessions --test <suite>` EXIT=0).
- Frozen hashes (sha256): test `share_queue.rs` `7b8ba795…`, `share_queue_lane.rs` `43dba68b…`; src `share_queue.rs` `25057f59…`, `share_queue_lane.rs` `10ef3449…`. No frozen test edited.
- RED history: covered by SHARE-001.md bundle lane §"RED-probe run" (behavior mutations fail-for-behavior); primary-T05 Debug pin gap owned by redaction lane (Verifier-Proposal diff in SHARE-001.md:124-178) — primary T05 pins only logs, never `format!("{:?}", ShareEvent)`; lane mirror T05 does.

## Decisions
- No code change: implementation already GREEN + redacted; dedicated worklog only.
- `crates/share` non-existence is integrator/controller mapping decision, not a code gap; lane-variant is the real implementation.

## Remaining unknowns
- `lib.rs` vs test-header drift: `lib.rs` lists `pub mod share_queue` but test header claims NOT wired. Integrator to confirm which copy the verifier exercises.
- Primary T05 Debug pin missing (see Verifier-Proposal in SHARE-001.md). Verifier decides.
- `crates/share` (`opencode-rk-share`) acceptance mapping pending controller allowlist.
