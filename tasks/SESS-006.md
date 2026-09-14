# SESS-006

Status: IN PROGRESS. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes. Requirements: None.
Dependencies: crates/contracts provides SessionId, SessionState, Timestamp.

## User-observable outcome

Session state machine in `crates/sessions/src/state.rs` provides:
- `SessionState` struct wrapping contracts' SessionState with version and migration metadata
- `StateTransition` enum for valid transitions: Created, Archived, Respawned, Forked
- `StateMachine` for tracking and validating state transitions per session
- Methods: current_state, can_transition, record_transition, history

## Source evidence

- `crates/contracts/src/lib.rs:108-144` - SessionId, Timestamp, SessionState enum (Active/Archived)
- `crates/sessions/src/types.rs` - patterns for session types using crate paths
- `crates/sessions/src/lib.rs:11-13` - imports from contracts: SessionId, SessionState, Timestamp

## Observable contract

### SessionState struct
- `current: SessionState` (from contracts)
- `version: u32`
- `migrated_at: Option<Timestamp>`

### StateTransition enum
- `Created` - session created
- `Archived` - session archived
- `Respawned` - session respawned
- `Forked` - session forked

### StateMachine struct
- `transitions: HashMap<SessionId, Vec<StateTransition>>`

### Methods
- `current_state(session_id) -> Option<&SessionState>` - get current state
- `can_transition(session_id, to) -> bool` - validate if transition is allowed
- `record_transition(session_id, transition)` - record a transition
- `history(session_id) -> Vec<StateTransition>` - get all transitions for a session

## Test obligations

- SESS-006-T01: current_state - returns None for non-existent, correct state for existing
- SESS-006-T02: can_transition_validates - validates allowed transitions
- SESS-006-T03: record_transition_appends - appends to history
- SESS-006-T04: history_returns_all - returns all transitions
- SESS-006-T05: empty_history - returns empty vec for never-transitioned session

## Verification

```bash
cargo test -p opencode-rk-sessions
cargo check --workspace
```

Both commands pass with no errors.

## Notes

- Uses contracts' SessionState enum (Active/Archived variants) as the inner state
- StateTransition records transitions but doesn't track "From" state - validation in can_transition uses current_state
- Transaction recording is append-only with no deduplication