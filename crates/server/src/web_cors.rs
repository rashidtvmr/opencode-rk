//! Pure CORS allowlist: validate + add origins, bounded, no IO.

/// Maximum number of origins the allowlist may hold.
pub const MAX_CORS_ORIGINS: usize = 64;

/// Allowlist failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorsError {
    EmptyOrigin,
    BadOrigin,
    TooMany { max: usize, actual: usize },
}

impl std::fmt::Display for CorsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyOrigin => write!(f, "empty origin"),
            Self::BadOrigin => write!(f, "bad origin"),
            Self::TooMany { max, actual } => {
                write!(f, "too many origins: max {max}, got {actual}")
            }
        }
    }
}

impl std::error::Error for CorsError {}

/// Validate `origin` and push it onto `origins`. Duplicates are idempotent.
pub fn add_origin(origins: &mut Vec<String>, origin: &str) -> Result<(), CorsError> {
    let origin = origin.trim();
    if origin.is_empty() {
        return Err(CorsError::EmptyOrigin);
    }
    let rest = origin
        .strip_prefix("https://")
        .or_else(|| origin.strip_prefix("http://"));
    let Some(rest) = rest else {
        return Err(CorsError::BadOrigin);
    };
    if !rest.contains('.') {
        return Err(CorsError::BadOrigin);
    }
    if origins.iter().any(|o| o == origin) {
        return Ok(());
    }
    if origins.len() >= MAX_CORS_ORIGINS {
        return Err(CorsError::TooMany {
            max: MAX_CORS_ORIGINS,
            actual: origins.len(),
        });
    }
    origins.push(origin.to_string());
    Ok(())
}
