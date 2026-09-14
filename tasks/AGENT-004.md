# AGENT-004: Agent Session Management

## Summary
In-memory session tracking for agents, providing lifecycle states, activity updates, and idle cleanup.

## Scope
- File: `crates/agents/src/session.rs`
- Crate: `opencode-rk-agents`

## Requirements
- `AgentSession`: id, agent_id, started_at, last_activity, messages, state
- `SessionState`: Active, Idle, Terminated
- `SessionManager`: sessions HashMap<String, AgentSession>, create(), get(), activity(), terminate(), cleanup_idle()
- 5 tests: create_and_get, activity_updates, terminate_changes_state, cleanup_idle, multiple_sessions

## Verification
```
cargo test -p opencode-rk-agents
cargo check --workspace
```
