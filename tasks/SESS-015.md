# SESS-015: Session Lifecycle V2

## Scope
ONLY modify `crates/sessions/src/lifecycle.rs`

## Goal
Implement a V2 session lifecycle with event tracking capabilities.

## Deliverable
Add to `crates/sessions/src/lifecycle.rs`:

### Types
1. `LifecycleEventType` enum: Created, Modified, Archived, Restored, Deleted
2. `LifecycleEvent` struct: event_type, timestamp, data: Option<String>, source: Option<String>
3. `SessionLifecycleV2` struct: session_id, events: Vec<LifecycleEvent>, current_state, started_at

### Methods
- `record_event(event_type)` - creates LifecycleEvent and pushes to events
- `get_state()` - returns current_state
- `events_for(type) -> Vec<&LifecycleEvent>` - filters events by type
- `reset()` - clears events, resets to initial state

### Tests (5 required)
1. `record_event_creates` - verifies event recording
2. `get_state_returns_current` - verifies state retrieval
3. `events_for_type` - verifies event filtering by type
4. `reset_clears` - verifies reset functionality
5. `chronological_order` - verifies events maintain proper order

## Verification
```bash
cargo test -p opencode-rk-sessions
cargo check --workspace
```