//! Typed session-history rewind contract.

use thiserror::Error;

const MAX_HISTORY_ENTRIES: usize = 100_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HistoryBoundary(u64);

impl HistoryBoundary {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryEntry {
    pub id: u64,
    pub text: String,
}

impl HistoryEntry {
    #[must_use]
    pub fn new(id: u64, text: impl Into<String>) -> Self {
        Self {
            id,
            text: text.into(),
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum HistoryError {
    #[error("invalid history boundary: {0:?}")]
    InvalidBoundary(HistoryBoundary),
    #[error("future history is retained; branch or rollback before appending")]
    FutureHistoryRetained,
    #[error("session history reached its retained-entry limit ({MAX_HISTORY_ENTRIES})")]
    HistoryLimitReached,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionHistory {
    entries: Vec<HistoryEntry>,
    head: Option<HistoryBoundary>,
}

impl SessionHistory {
    #[must_use]
    pub fn new(entries: Vec<HistoryEntry>) -> Self {
        let head = entries.last().map(|entry| HistoryBoundary::new(entry.id));
        Self { entries, head }
    }

    #[must_use]
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    #[must_use]
    pub const fn head(&self) -> Option<HistoryBoundary> {
        self.head
    }

    pub fn revert(&mut self, boundary: HistoryBoundary) -> Result<(), HistoryError> {
        self.require_boundary(boundary)?;
        self.head = Some(boundary);
        Ok(())
    }

    pub fn rollback(&mut self, boundary: HistoryBoundary) -> Result<(), HistoryError> {
        let index = self.require_boundary(boundary)?;
        self.entries.truncate(index + 1);
        self.head = Some(boundary);
        Ok(())
    }

    pub fn append(&mut self, entry: HistoryEntry) -> Result<(), HistoryError> {
        let retained_head = self
            .entries
            .last()
            .map(|entry| HistoryBoundary::new(entry.id));
        if self.head != retained_head {
            return Err(HistoryError::FutureHistoryRetained);
        }
        if self.entries.len() >= MAX_HISTORY_ENTRIES {
            return Err(HistoryError::HistoryLimitReached);
        }
        self.head = Some(HistoryBoundary::new(entry.id));
        self.entries.push(entry);
        Ok(())
    }

    fn require_boundary(&self, boundary: HistoryBoundary) -> Result<usize, HistoryError> {
        self.entries
            .iter()
            .position(|entry| HistoryBoundary::new(entry.id) == boundary)
            .ok_or(HistoryError::InvalidBoundary(boundary))
    }
}
