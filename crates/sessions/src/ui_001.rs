use thiserror::Error;

pub const MAX_UI_SESSIONS: usize = 200;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionListItem {
    pub id: String,
    pub title: String,
    pub active: bool,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum SessionListError {
    #[error("session id must not be empty")]
    EmptyId,
    #[error("too many sessions: max {max}, actual {actual}")]
    TooManySessions { max: usize, actual: usize },
}

#[derive(Clone, Debug, Default)]
pub struct SessionListState {
    entries: Vec<SessionListItem>,
}

impl SessionListState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn upsert(&mut self, item: SessionListItem) -> Result<(), SessionListError> {
        if item.id.is_empty() {
            return Err(SessionListError::EmptyId);
        }
        if let Some(slot) = self.entries.iter_mut().find(|e| e.id == item.id) {
            *slot = item;
            return Ok(());
        }
        if self.entries.len() >= MAX_UI_SESSIONS {
            return Err(SessionListError::TooManySessions {
                max: MAX_UI_SESSIONS,
                actual: self.entries.len(),
            });
        }
        self.entries.push(item);
        Ok(())
    }
    #[must_use]
    pub fn select(&self, id: &str) -> Option<&SessionListItem> {
        self.entries.iter().find(|e| e.id == id)
    }
    #[must_use]
    pub fn list(&self) -> &[SessionListItem] {
        &self.entries
    }
    pub fn remove(&mut self, id: &str) -> bool {
        match self.entries.iter().position(|e| e.id == id) {
            Some(i) => {
                self.entries.remove(i);
                true
            }
            None => false,
        }
    }
}
