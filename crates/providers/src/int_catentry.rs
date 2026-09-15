//! Catalog entry validation. Pure, no IO.

use thiserror::Error;

/// Catalog entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatEntry {
    pub id: String,
    pub title: String,
}

/// Catalog entry validation failure.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum CatEntryError {
    #[error("catalog entry id must not be empty")]
    EmptyId,
    #[error("catalog entry title must not be empty")]
    EmptyTitle,
}

/// Build entry, trimming id and title. Empty id wins.
pub fn make_entry(id: &str, title: &str) -> Result<CatEntry, CatEntryError> {
    let id = id.trim();
    if id.is_empty() {
        return Err(CatEntryError::EmptyId);
    }
    let title = title.trim();
    if title.is_empty() {
        return Err(CatEntryError::EmptyTitle);
    }
    Ok(CatEntry {
        id: id.to_string(),
        title: title.to_string(),
    })
}
