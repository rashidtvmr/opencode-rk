//! Release checklist state (REL-002 slice).
//!
//! Pure checklist: flip or append an item, return its new state.
//! No IO, no threads.

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelCheck {
    pub item: String,
    pub done: bool,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RelCheckError {
    #[error("checklist item must not be empty")]
    EmptyItem,
}

pub fn set_done(items: &mut Vec<RelCheck>, item: &str, done: bool) -> Result<bool, RelCheckError> {
    let trimmed = item.trim();
    if trimmed.is_empty() {
        return Err(RelCheckError::EmptyItem);
    }
    if let Some(found) = items.iter_mut().find(|c| c.item == trimmed) {
        found.done = done;
        return Ok(found.done);
    }
    items.push(RelCheck {
        item: trimmed.to_string(),
        done,
    });
    Ok(done)
}
