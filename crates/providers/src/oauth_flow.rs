//! OAuth-flow planner slice (opencode.integration-auth; INT-006 half).
//! Pure logic: validate ordered (name, url) pairs into planned steps.
//! Secret-free, no network.

/// Maximum number of steps accepted by [`plan_flow`].
pub const MAX_OAUTH_STEPS: usize = 8;

/// A single planned OAuth flow step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthStep {
    /// Step name (non-empty).
    pub name: String,
    /// Step URL (non-empty, must start with `https://`).
    pub url: String,
}

/// Errors from [`plan_flow`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OAuthFlowError {
    /// A step had an empty name.
    #[error("oauth step name is empty")]
    EmptyName,
    /// A step had an empty url.
    #[error("oauth step url is empty")]
    EmptyUrl,
    /// A step url did not start with `https://`.
    #[error("oauth step url must start with https://")]
    BadUrl,
    /// More steps supplied than [`MAX_OAUTH_STEPS`].
    #[error("too many oauth steps: max {max}, got {actual}")]
    TooManySteps {
        /// The cap ([`MAX_OAUTH_STEPS`]).
        max: usize,
        /// Steps actually supplied.
        actual: usize,
    },
}

/// Validate `steps` into an ordered [`Vec<OAuthStep>`], preserving order.
pub fn plan_flow(steps: &[(&str, &str)]) -> Result<Vec<OAuthStep>, OAuthFlowError> {
    if steps.len() > MAX_OAUTH_STEPS {
        return Err(OAuthFlowError::TooManySteps {
            max: MAX_OAUTH_STEPS,
            actual: steps.len(),
        });
    }
    steps
        .iter()
        .map(|(name, url)| {
            if name.is_empty() {
                return Err(OAuthFlowError::EmptyName);
            }
            if url.is_empty() {
                return Err(OAuthFlowError::EmptyUrl);
            }
            if !url.starts_with("https://") {
                return Err(OAuthFlowError::BadUrl);
            }
            Ok(OAuthStep {
                name: (*name).to_owned(),
                url: (*url).to_owned(),
            })
        })
        .collect()
}
