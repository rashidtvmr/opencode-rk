//! Pure app-client endpoint config builder (native half of opencode.app-client).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppClientConfig {
    pub origin: String,
    pub api_path: String,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppClientError {
    EmptyOrigin,
    BadOrigin,
    EmptyPath,
    ZeroTimeout,
}

impl std::fmt::Display for AppClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyOrigin => write!(f, "empty origin"),
            Self::BadOrigin => write!(f, "bad origin"),
            Self::EmptyPath => write!(f, "empty path"),
            Self::ZeroTimeout => write!(f, "zero timeout"),
        }
    }
}

impl std::error::Error for AppClientError {}

pub fn build_app_client_config(
    origin: &str,
    api_path: &str,
    timeout_ms: u64,
) -> Result<AppClientConfig, AppClientError> {
    if origin.is_empty() {
        return Err(AppClientError::EmptyOrigin);
    }
    if !origin.starts_with("http://") && !origin.starts_with("https://") {
        return Err(AppClientError::BadOrigin);
    }
    if !api_path.starts_with('/') {
        return Err(AppClientError::EmptyPath);
    }
    if timeout_ms == 0 {
        return Err(AppClientError::ZeroTimeout);
    }
    Ok(AppClientConfig {
        origin: origin.trim_end_matches('/').to_string(),
        api_path: api_path.to_string(),
        timeout_ms,
    })
}
