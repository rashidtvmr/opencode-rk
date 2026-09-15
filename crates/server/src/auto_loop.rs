//! Loop-state tracker (AUTO loop-state slice).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopState {
    Idle,
    Running,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopTracker {
    pub state: String,
    pub ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopError {
    AlreadyRunning,
    NotRunning,
}

impl std::fmt::Display for LoopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyRunning => write!(f, "already running"),
            Self::NotRunning => write!(f, "not running"),
        }
    }
}

impl std::error::Error for LoopError {}

pub fn parse_loop_state(s: &str) -> Result<LoopState, LoopError> {
    if s.eq_ignore_ascii_case("idle") {
        Ok(LoopState::Idle)
    } else if s.eq_ignore_ascii_case("running") {
        Ok(LoopState::Running)
    } else if s.eq_ignore_ascii_case("stopped") {
        Ok(LoopState::Stopped)
    } else {
        Err(LoopError::NotRunning)
    }
}

impl LoopTracker {
    pub fn new() -> Self {
        Self {
            state: "idle".to_string(),
            ticks: 0,
        }
    }

    pub fn get_state(&self) -> Result<LoopState, LoopError> {
        parse_loop_state(&self.state)
    }

    pub fn tick(&mut self) -> Result<(), LoopError> {
        match parse_loop_state(&self.state)? {
            LoopState::Running => {
                self.ticks += 1;
                self.state = "running".to_string();
                Ok(())
            }
            _ => Err(LoopError::NotRunning),
        }
    }

    pub fn stop(&mut self) -> Result<(), LoopError> {
        match parse_loop_state(&self.state)? {
            LoopState::Running => {
                self.state = "stopped".to_string();
                Ok(())
            }
            _ => Err(LoopError::NotRunning),
        }
    }
}

impl Default for LoopTracker {
    fn default() -> Self {
        Self::new()
    }
}
