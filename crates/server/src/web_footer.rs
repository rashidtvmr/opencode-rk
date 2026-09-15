//! Pure app-footer builder.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebFooter {
    pub text: String,
    pub version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FooterError {
    EmptyText,
    EmptyVersion,
}

impl std::fmt::Display for FooterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyText => write!(f, "empty text"),
            Self::EmptyVersion => write!(f, "empty version"),
        }
    }
}

impl std::error::Error for FooterError {}

pub fn build_footer(text: &str, version: &str) -> Result<WebFooter, FooterError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(FooterError::EmptyText);
    }
    let version = version.trim();
    if version.is_empty() {
        return Err(FooterError::EmptyVersion);
    }
    Ok(WebFooter {
        text: text.to_string(),
        version: version.to_string(),
    })
}
