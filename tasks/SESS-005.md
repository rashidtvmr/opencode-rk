# SESS-005

Status: IN PROGRESS. Kind: product. Runtime optional: False.
Mandatory for full declared release: no.
Requirements: None.
Dependencies: crates/contracts provides SessionId and MessageId; crates/sessions provides SessionRecord.

## User-observable outcome

`SessionEvent` enum and `SessionEventBus` struct in `crates/sessions/src/events.rs` enable in-process event-driven session lifecycle notifications within the sessions crate.

## Source evidence

- `SessionId` defined in contracts/src/lib.rs:59 via `uuid_id!` macro, provides `new()` and `as_uuid()`
- `MessageId` defined in contracts/src/lib.rs:60 via `uuid_id!` macro
- SessionRecord is type alias for SessionSummary (lib.rs:13)
- SessionSummary defined in contracts/src/lib.rs:146-154

## Observable contract

### SessionEvent enum

- `Created { id: SessionId }` - session created
- `Saved { id: SessionId }` - session mutated
- `Archived { id: SessionId }` - session archived
- `Deleted { id: SessionId }` - session deleted
- `MessageAppended { session: SessionId, message_id: MessageId }` - message appended

### SessionEventBus

- `new()` - creates empty bus with default capacity
- `MAX_SUBSCRIBERS: u64 = 100` - hard ceiling
- `subscribe(handler) -> Option<SubscriptionId>` - registers handler, returns None if at capacity
- `unsubscribe(id: SubscriptionId) -> Option<u64>` - removes handler, returns None if not found
- `emit(event)` - dispatches to all subscribers, records last event
- `stats() -> (subscribers: u64, last_event: Option<SessionEvent>)` - returns count and last event

## Test obligations

- SESS-005-T01: emit_delivers - verify emit notifies all subscribers
- SESS-005-T02: subscribe_returns_id - verify subscribe returns incrementing IDs
- SESS-005-T03: unsubscribe_works - verify unsubscribe removes correct handler
- SESS-005-T04: max_subscribers - verify bus rejects beyond MAX_SUBSCRIBERS
- SESS-005-T05: stats_track - verify stats tracks subscriber count and last event

## Verification

```bash
cargo test -p opencode-rk-sessions
cargo check --workspace
```

Both commands pass with no errors.

## Notes

The events module is pure Rust with no external dependencies beyond what the sessions crate already uses (opencode-rk-contracts for SessionId/MessageId). Handlers are trait objects implementing `Send + Sync` to enable safe cross-thread use. The `last_event` tracking is optional and cleared only on explicit emit; initial state is None.