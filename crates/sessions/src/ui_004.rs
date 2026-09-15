//! Fork indicator state (UI-004, REQ-008 session forking).
//!
//! Pure logic, no IO. Caller owns the mark buffer.

use thiserror::Error;

/// Max fork depth accepted by [`mark_fork`].
pub const MAX_FORK_DEPTH: u32 = 8;
/// Max fork marks retained in the caller-owned buffer.
pub const MAX_FORK_MARKS: usize = 256;

/// One fork indicator row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForkMark {
    pub session_id: String,
    pub fork_of: Option<String>,
    pub depth: u32,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ForkViewError {
    #[error("session id is empty")]
    EmptyId,
    #[error("fork too deep: max {max}, asked {asked}")]
    TooDeep { max: u32, asked: u32 },
    #[error("too many forks: max {max}, actual {actual}")]
    TooManyForks { max: usize, actual: usize },
}

/// Insert or replace a fork mark. Same id replaces in place.
pub fn mark_fork(marks: &mut Vec<ForkMark>, m: ForkMark) -> Result<(), ForkViewError> {
    if m.session_id.is_empty() {
        return Err(ForkViewError::EmptyId);
    }
    if m.depth > MAX_FORK_DEPTH {
        return Err(ForkViewError::TooDeep {
            max: MAX_FORK_DEPTH,
            asked: m.depth,
        });
    }
    if let Some(pos) = marks.iter().position(|e| e.session_id == m.session_id) {
        marks[pos] = m;
        return Ok(());
    }
    if marks.len() >= MAX_FORK_MARKS {
        return Err(ForkViewError::TooManyForks {
            max: MAX_FORK_MARKS,
            actual: marks.len(),
        });
    }
    marks.push(m);
    Ok(())
}

/// Follow `fork_of` links from `id` up to MAX depth. Starts with `id`,
/// stops at missing mark or root (`fork_of: None`).
#[must_use]
pub fn fork_chain(marks: &[ForkMark], id: &str) -> Vec<String> {
    let mut out = vec![id.to_owned()];
    let mut current = id.to_owned();
    for _ in 0..MAX_FORK_DEPTH {
        let next = marks
            .iter()
            .find(|m| m.session_id == current)
            .and_then(|m| m.fork_of.clone());
        match next {
            Some(parent) => {
                out.push(parent.clone());
                current = parent;
            }
            None => break,
        }
    }
    out
}
