# NET-008-rturn worklog

## Claim
Remote turn control types: phone envelope, selection marker, idempotent
submit, targeted interrupt, loud unauthorized/offline errors. Owned file
only: `crates/server/src/remote_turns.rs`.

## Source evidence (repo 5af7884)
- `crates/server/src/remote_ledger.rs:1-97` — current code pattern:
  `#![forbid(unsafe_code)]`, typed errors with Display, bounded in-memory
  store (MAX_LEDGER_ENTRIES=256). Followed same shape.
- `crates/server/src/remote_sync.rs:1-204` — current code bounds pattern
  (MAX_SYNC_QUEUE_ITEMS/BYTES), caller-owned sync, no threads/IO.
- `docs/TDD.md`, `docs/SECURITY.md` — denied permission asserts absence of
  side effects; offline must never silently queue; capability broker owns
  auth (approximated here as explicit authorize set, real broker wires in).
- New requirement (no upstream impl): NET-008 card journey
  phone starts/steers/stops real PC turns; T01..T05 = tests below.

## Observed scenario
RED scaffold written first (single panicking test), confirmed shape, then
replaced with full impl + frozen suite. No cargo build per instructions.

## Target boundary
- `Origin::{Local,Phone{device_id}}` envelope; `engine_request()` strips
  origin → same engine input phone vs local (T01).
- `Selection{model,effort,agent}` closed allowlists; `last_selection`
  per device key persists across turns (T02).
- `submission_id` idempotency: replay → same receipt `deduped:true`, one
  turn even with changed prompt (T04).
- `interrupt(origin, turn_id)` hits intended turn only, appends marker,
  preserves prompt history, idempotent repeat (T03).
- Phone gates: `Unauthorized{device}` / `Offline{device}` clear messages;
  offline stores nothing, persists nothing, queues nothing (T05).
- Bounds: MAX_TURNS=256, MAX_PROMPT_BYTES=32768, MAX_HISTORY_ENTRIES=64,
  id/selection field caps. Sync, std only, no unsafe, no IO/threads.

## Tests (frozen in-file, 9 cases)
T01 phone_and_local_share_engine_input; T02 selection_persists...;
T04 duplicate_submission_id_creates_one_turn,
duplicate_id_with_different_prompt...; T03 interrupt_hits_intended...;
T05 unauthorized_phone_submit..., unauthorized_interrupt...,
offline_submit_errors_and_never_queues; + unknown_turn_and_bad_selection.

## Decisions
- `ponytail:` none needed; allowlists closed by design, broker wiring is
  integrator scope (not this file).
- Offline handled as hard reject (not Error-state like remote_sync) per
  "never invisible queue".

## Remaining unknowns
- Wiring into real engine/broker/lib.rs is integrator scope; this file
  exposes `engine_request()` as the seam.
