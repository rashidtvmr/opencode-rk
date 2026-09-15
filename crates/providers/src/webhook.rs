//! Pure webhook route planner (INT-007).
//!
//! Pure function of caller-supplied inputs: no I/O, env, net, clock, or threads.

use thiserror::Error;

/// Planned webhook route: local path receiving events for `topic`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebhookRoute {
    pub path: String,
    pub topic: String,
}

/// Typed route-planning failures.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum WebhookError {
    #[error("empty path")]
    EmptyPath,
    #[error("path must start with '/'")]
    BadPath,
    #[error("empty topic")]
    EmptyTopic,
    #[error("too many routes: {actual} exceeds max {max}")]
    TooManyRoutes { max: usize, actual: usize },
}

/// Maximum accepted webhook routes.
pub const MAX_WEBHOOK_ROUTES: usize = 64;

/// Plan webhook routes as a pure function of `rs`.
///
/// Order: enforce the route bound, then validate each entry in order,
/// preserving input order in the output.
pub fn plan_routes(rs: &[(&str, &str)]) -> Result<Vec<WebhookRoute>, WebhookError> {
    if rs.len() > MAX_WEBHOOK_ROUTES {
        return Err(WebhookError::TooManyRoutes {
            max: MAX_WEBHOOK_ROUTES,
            actual: rs.len(),
        });
    }
    rs.iter()
        .map(|(path, topic)| {
            if path.is_empty() {
                return Err(WebhookError::EmptyPath);
            }
            if !path.starts_with('/') {
                return Err(WebhookError::BadPath);
            }
            if topic.is_empty() {
                return Err(WebhookError::EmptyTopic);
            }
            Ok(WebhookRoute {
                path: (*path).to_owned(),
                topic: (*topic).to_owned(),
            })
        })
        .collect()
}
