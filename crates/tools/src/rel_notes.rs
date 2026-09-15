//! REL release-notes qualification: (version, text) pairs -> ordered notes.
use thiserror::Error;

/// Maximum number of notes accepted by [`qualify_notes`].
pub const MAX_NOTES: usize = 256;

/// A qualified release note.
#[derive(Debug, PartialEq, Eq)]
pub struct ReleaseNote {
    /// Version string in `X.Y.Z` numeric form.
    pub version: String,
    /// Note body text.
    pub text: String,
}

/// Release-note qualification failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum NotesError {
    /// The version string is empty.
    #[error("empty version")]
    EmptyVersion,
    /// The version is not numeric `X.Y.Z`.
    #[error("bad version")]
    BadVersion,
    /// The note text is empty.
    #[error("empty text")]
    EmptyText,
    /// The note list exceeds [`MAX_NOTES`].
    #[error("too many notes: max {max}, got {actual}")]
    TooManyNotes { max: usize, actual: usize },
}

fn version_ok(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    parts
        .iter()
        .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Validate `(version, text)` pairs into ordered [`ReleaseNote`]s.
///
/// Length is checked first; entries validate in order: empty version, then
/// numeric `X.Y.Z` shape, then non-empty text. Order is preserved.
pub fn qualify_notes(ns: &[(&str, &str)]) -> Result<Vec<ReleaseNote>, NotesError> {
    if ns.len() > MAX_NOTES {
        return Err(NotesError::TooManyNotes {
            max: MAX_NOTES,
            actual: ns.len(),
        });
    }
    let mut out = Vec::with_capacity(ns.len());
    for (version, text) in ns {
        if version.is_empty() {
            return Err(NotesError::EmptyVersion);
        }
        if !version_ok(version) {
            return Err(NotesError::BadVersion);
        }
        if text.is_empty() {
            return Err(NotesError::EmptyText);
        }
        out.push(ReleaseNote {
            version: (*version).to_string(),
            text: (*text).to_string(),
        });
    }
    Ok(out)
}
