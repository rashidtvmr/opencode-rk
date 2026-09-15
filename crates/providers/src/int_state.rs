//! Connection-state machine. Pure, no IO.

/// Connection lifecycle state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnState {
    Idle,
    Open,
    Shut,
}

/// Advance state on operation outcome (`ok` = success).
pub fn next_state(s: &ConnState, ok: bool) -> ConnState {
    match (s, ok) {
        (ConnState::Shut, _) => ConnState::Shut,
        (ConnState::Idle, true) => ConnState::Open,
        (ConnState::Idle, false) => ConnState::Idle,
        (ConnState::Open, true) => ConnState::Open,
        (ConnState::Open, false) => ConnState::Shut,
    }
}
