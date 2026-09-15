//! Run-window check (AUTO run-window slice).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunWindow {
    pub start_h: u8,
    pub end_h: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowError {
    BadRange,
}

impl std::fmt::Display for WindowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadRange => write!(f, "bad range"),
        }
    }
}

impl std::error::Error for WindowError {}

pub fn make_window(start_h: u8, end_h: u8) -> Result<RunWindow, WindowError> {
    if start_h > 23 || end_h > 23 {
        return Err(WindowError::BadRange);
    }
    if start_h == end_h {
        return Err(WindowError::BadRange);
    }
    Ok(RunWindow { start_h, end_h })
}

pub fn in_window(w: &RunWindow, h: u8) -> bool {
    if w.start_h < w.end_h {
        h >= w.start_h && h < w.end_h
    } else {
        h >= w.start_h || h < w.end_h
    }
}
