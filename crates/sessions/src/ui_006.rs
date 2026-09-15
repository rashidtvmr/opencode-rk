use thiserror::Error;

pub const MAX_PANEL_SESSIONS: u64 = 10_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusCounts {
    pub sessions: u64,
    pub active_turns: u64,
    pub errors: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusPanel {
    pub title: String,
    pub counts: StatusCounts,
    pub healthy: bool,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum StatusPanelError {
    #[error("panel title must not be empty")]
    EmptyTitle,
    #[error("too many sessions: max {max}, actual {actual}")]
    TooManySessions { max: u64, actual: u64 },
}

pub fn build_panel(title: &str, counts: &StatusCounts) -> Result<StatusPanel, StatusPanelError> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err(StatusPanelError::EmptyTitle);
    }
    if counts.sessions > MAX_PANEL_SESSIONS {
        return Err(StatusPanelError::TooManySessions {
            max: MAX_PANEL_SESSIONS,
            actual: counts.sessions,
        });
    }
    Ok(StatusPanel {
        title: trimmed.to_owned(),
        counts: counts.clone(),
        healthy: counts.errors == 0,
    })
}
