//! Release stamp builder (REL-003).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelStamp {
    pub tag: String,
    pub seq: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelStampError {
    EmptyTag,
}

impl std::fmt::Display for RelStampError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyTag => write!(f, "empty tag"),
        }
    }
}

impl std::error::Error for RelStampError {}

pub fn make_stamp(tag: &str, seq: u64) -> Result<RelStamp, RelStampError> {
    let trimmed = tag.trim();
    if trimmed.is_empty() {
        return Err(RelStampError::EmptyTag);
    }
    Ok(RelStamp {
        tag: trimmed.to_string(),
        seq,
    })
}

pub fn stamp_label(s: &RelStamp) -> String {
    format!("{}-{:04}", s.tag, s.seq)
}
