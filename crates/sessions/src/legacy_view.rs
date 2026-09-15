//! Pure legacy session-view projector (UI-012 half).
//!
//! Projects new `SessionSummary` triples into the legacy shape consumed by
//! legacy clients. Pure: no IO, order-preserving, bounded output.

use thiserror::Error;

/// Maximum number of sessions projectable in one call.
pub const MAX_LEGACY_SESSIONS: usize = 500;

/// Legacy session shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacySessionView {
    pub id: String,
    pub title: String,
    pub archived: bool,
}

/// Projection failures.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LegacyViewError {
    #[error("legacy session id must not be empty")]
    EmptyId,
    #[error("legacy session title must not be empty")]
    EmptyTitle,
    #[error("too many sessions: max {max}, got {actual}")]
    TooManySessions { max: usize, actual: usize },
}

/// Map `(id, title, archived)` triples to [`LegacySessionView`]s in order.
///
/// Fails on oversize input, any empty id, or any empty title.
pub fn project_legacy(
    sessions: &[(&str, &str, bool)],
) -> Result<Vec<LegacySessionView>, LegacyViewError> {
    if sessions.len() > MAX_LEGACY_SESSIONS {
        return Err(LegacyViewError::TooManySessions {
            max: MAX_LEGACY_SESSIONS,
            actual: sessions.len(),
        });
    }
    sessions
        .iter()
        .map(|(id, title, archived)| {
            if id.is_empty() {
                return Err(LegacyViewError::EmptyId);
            }
            if title.is_empty() {
                return Err(LegacyViewError::EmptyTitle);
            }
            Ok(LegacySessionView {
                id: (*id).to_owned(),
                title: (*title).to_owned(),
                archived: *archived,
            })
        })
        .collect()
}
