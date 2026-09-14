# SESS-010: Session Lifecycle State Machine

## Summary
Implement `LifecycleAction` enum and `SessionLifecycle` struct in `crates/sessions/src/lifecycle.rs`.

## Requirements
- `LifecycleAction` enum: Create, Activate, Pause, Archive, Delete, Fork
- `SessionLifecycle` struct with fields:
  - `state: SessionState`
  - `actions: Vec<(LifecycleAction, Timestamp)>`
  - `transitions: Vec<SessionSummary>`
- Methods:
  - `transition(action) -> Result<SessionState, LifecycleError>` - performs state transition
  - `can_transition(action) -> bool` - checks if transition is valid
  - `history() -> &[Action]` - returns action history (aliased to actions)
- 5 tests: create_transitions, archive_works, delete_final, pause_blocks, history_records

## State Transitions
- `Create` -> Active
- `Activate` -> Active (from Paused)
- `Pause` -> Paused (from Active)
- `Archive` -> Archived (from Active or Paused)
- `Delete` -> Deleted (from any non-terminal state; terminal state, no further transitions)
- `Fork` -> Active (creates a fork, does not consume original)

## Owned File
`crates/sessions/src/lifecycle.rs`

## Verification
- `cargo test -p opencode-rk-sessions`
- `cargo check --workspace`