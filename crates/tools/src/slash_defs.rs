//! Slash-command definitions qualifier (EXT-002).

use thiserror::Error;

pub const MAX_SLASH: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashDef {
    pub name: String,
    pub desc: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SlashError {
    #[error("empty name")]
    EmptyName,
    #[error("bad name")]
    BadName,
    #[error("too many: max {max}, actual {actual}")]
    TooMany { max: usize, actual: usize },
}

pub fn qualify_slash(items: &[(&str, &str)]) -> Result<Vec<SlashDef>, SlashError> {
    if items.len() > MAX_SLASH {
        return Err(SlashError::TooMany {
            max: MAX_SLASH,
            actual: items.len(),
        });
    }
    items
        .iter()
        .map(|(name, desc)| {
            if name.is_empty() {
                return Err(SlashError::EmptyName);
            }
            if name.len() < 2 || !name.starts_with('/') || name.contains(char::is_whitespace) {
                return Err(SlashError::BadName);
            }
            Ok(SlashDef {
                name: (*name).to_owned(),
                desc: (*desc).to_owned(),
            })
        })
        .collect()
}
