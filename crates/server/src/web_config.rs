//! Pure web client config builder (native half of desktop-client web view).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebConfig {
    pub title: String,
    pub port: u16,
    pub dev: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebConfigError {
    EmptyTitle,
    ZeroPort,
}

impl std::fmt::Display for WebConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyTitle => write!(f, "empty title"),
            Self::ZeroPort => write!(f, "zero port"),
        }
    }
}

impl std::error::Error for WebConfigError {}

pub fn build_web_config(
    title: &str,
    port: u16,
    dev: bool,
) -> Result<WebConfig, WebConfigError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(WebConfigError::EmptyTitle);
    }
    if port == 0 {
        return Err(WebConfigError::ZeroPort);
    }
    Ok(WebConfig {
        title: title.to_string(),
        port,
        dev,
    })
}
