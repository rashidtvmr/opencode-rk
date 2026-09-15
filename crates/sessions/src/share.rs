//! Pure share-link planner (SHARE-001). No network, no persistence.

use thiserror::Error;

/// Maximum number of retained share links (active + revoked).
pub const MAX_SHARE_LINKS: usize = 64;

/// A single share token bound to a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareLink {
    pub token: String,
    pub session_id: String,
    pub created_at: u64,
    pub revoked: bool,
}

/// Failure modes for share-link operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ShareError {
    #[error("session id must not be empty")]
    EmptySession,
    #[error("bad share token")]
    BadToken,
    #[error("too many share links: max {max}, actual {actual}")]
    TooManyLinks { max: usize, actual: usize },
    #[error("unknown share token: {token}")]
    UnknownToken { token: String },
}

/// In-memory ordered ledger of share links.
#[derive(Debug, Default)]
pub struct ShareLedger {
    links: Vec<ShareLink>,
}

impl ShareLedger {
    #[must_use]
    pub fn new() -> Self {
        Self { links: Vec::new() }
    }

    pub fn create(&mut self, session_id: &str, token: &str, now: u64) -> Result<(), ShareError> {
        if session_id.is_empty() {
            return Err(ShareError::EmptySession);
        }
        if !valid_token(token) || self.links.iter().any(|l| l.token == token) {
            return Err(ShareError::BadToken);
        }
        if self.links.len() >= MAX_SHARE_LINKS {
            return Err(ShareError::TooManyLinks {
                max: MAX_SHARE_LINKS,
                actual: self.links.len(),
            });
        }
        self.links.push(ShareLink {
            token: token.to_owned(),
            session_id: session_id.to_owned(),
            created_at: now,
            revoked: false,
        });
        Ok(())
    }

    pub fn revoke(&mut self, token: &str) -> Result<(), ShareError> {
        match self.links.iter_mut().find(|l| l.token == token) {
            Some(link) => {
                link.revoked = true;
                Ok(())
            }
            None => Err(ShareError::UnknownToken {
                token: token.to_owned(),
            }),
        }
    }

    #[must_use]
    pub fn active(&self) -> Vec<&ShareLink> {
        self.links.iter().filter(|l| !l.revoked).collect()
    }

    #[must_use]
    pub fn list(&self) -> &[ShareLink] {
        &self.links
    }
}

fn valid_token(token: &str) -> bool {
    !token.is_empty() && !token.contains('/') && !token.chars().any(char::is_whitespace)
}
