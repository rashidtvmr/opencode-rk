//! Pure web route-table: path prefix -> backend target, bounded, upsert on dup.

/// Maximum routes retained in the table.
pub const MAX_WEB_ROUTES: usize = 128;

/// A single path-to-target mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebRoute {
    pub path: String,
    pub target: String,
}

/// Route-table failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebRouteError {
    EmptyPath,
    EmptyTarget,
    TooMany { max: usize, actual: usize },
}

impl std::fmt::Display for WebRouteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPath => write!(f, "empty path"),
            Self::EmptyTarget => write!(f, "empty target"),
            Self::TooMany { max, actual } => {
                write!(f, "too many routes: max {max}, got {actual}")
            }
        }
    }
}

impl std::error::Error for WebRouteError {}

/// Insert a route. Duplicate `path` updates the target in place (no dup push).
pub fn add_route(
    routes: &mut Vec<WebRoute>,
    path: &str,
    target: &str,
) -> Result<(), WebRouteError> {
    if path.is_empty() {
        return Err(WebRouteError::EmptyPath);
    }
    if target.is_empty() {
        return Err(WebRouteError::EmptyTarget);
    }
    if let Some(slot) = routes.iter_mut().find(|r| r.path == path) {
        slot.target = target.to_string();
        return Ok(());
    }
    if routes.len() >= MAX_WEB_ROUTES {
        return Err(WebRouteError::TooMany {
            max: MAX_WEB_ROUTES,
            actual: routes.len(),
        });
    }
    routes.push(WebRoute {
        path: path.to_string(),
        target: target.to_string(),
    });
    Ok(())
}
