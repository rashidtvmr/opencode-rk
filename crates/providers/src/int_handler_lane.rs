//! INT-007 fallback lane: typed-integration-handler projection.
//!
//! Distinct fallback file from `handler_projection.rs` (same slice, no
//! collision). Pure, no IO. Projects a caller-supplied typed integration
//! handler outcome into its normalized HTTP-facing shape: secret-free
//! projection, `NoContent` mutation ack, or typed `InvalidRequest` / auth /
//! code-required error. Auth failures normalize to one shape with zero
//! provider detail; code-mode completion without a code stays pending as
//! code-required rather than failing as auth. No credentials, env, storage,
//! network, provider callbacks, or events touched.

/// Maximum items admitted in a `List` projection (sorted prefix, cap first).
pub const MAX_ITEMS: usize = 128;
/// Maximum `id` length in bytes (1..=64, charset below).
pub const MAX_ID_LEN: usize = 64;
/// Maximum `label` length in bytes (1..=64, charset below).
pub const MAX_LABEL_LEN: usize = 64;

/// Non-secret integration method tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Method {
    Key,
    OAuth,
}

/// Caller-supplied non-secret metadata. Holds id/label/method only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NonSecretMeta {
    pub id: String,
    pub label: String,
    pub method: Method,
}

/// Typed handler operation. Caller supplies `NonSecretMeta` values and an
/// optional code-presence flag only; never raw key/token bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandlerOp {
    List,
    Get,
    KeyAck,
    OAuthStart,
    OAuthComplete {
        code: Option<String>,
        auth_failed: bool,
    },
    AttemptPoll,
}

/// Normalized typed handler failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum HandlerError {
    #[error("invalid integration request")]
    InvalidRequest,
    #[error("integration auth failed")]
    AuthFailed,
    #[error("authorization code required")]
    CodeRequired,
    #[error("integration not found")]
    NotFound,
}

/// Normalized handler response: projection, bounded list, mutation ack,
/// or typed error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandlerResponse {
    Projection(NonSecretMeta),
    List {
        items: Vec<NonSecretMeta>,
        truncated: bool,
    },
    NoContent,
    Err(HandlerError),
}

/// Validate one text field: len 1..=max, first char ASCII alnum, rest
/// `[A-Za-z0-9._-]`.
fn valid_text(s: &str, max: usize) -> bool {
    if s.is_empty() || s.len() > max {
        return false;
    }
    let mut bytes = s.bytes();
    match bytes.next() {
        Some(b) if b.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    bytes.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn valid_meta(m: &NonSecretMeta) -> bool {
    valid_text(&m.id, MAX_ID_LEN) && valid_text(&m.label, MAX_LABEL_LEN)
}

/// Project `(op, meta, items)` to its normalized response.
///
/// Pure and synchronous: borrows caller-owned `items`/`meta`, spawns no
/// thread, allocates nothing beyond the returned vec/error value.
pub fn project(
    op: &HandlerOp,
    meta: Option<&NonSecretMeta>,
    items: &[NonSecretMeta],
) -> HandlerResponse {
    match op {
        HandlerOp::List => {
            let mut sorted: Vec<NonSecretMeta> = items.to_vec();
            sorted.sort_by(|a, b| a.id.cmp(&b.id));
            let truncated = sorted.len() > MAX_ITEMS;
            sorted.truncate(MAX_ITEMS);
            HandlerResponse::List {
                items: sorted,
                truncated,
            }
        }
        HandlerOp::Get => match meta {
            Some(m) => HandlerResponse::Projection(m.clone()),
            None => HandlerResponse::Err(HandlerError::NotFound),
        },
        HandlerOp::KeyAck | HandlerOp::OAuthStart => match meta {
            Some(m) if valid_meta(m) => HandlerResponse::NoContent,
            _ => HandlerResponse::Err(HandlerError::InvalidRequest),
        },
        HandlerOp::OAuthComplete { code, auth_failed } => {
            let has_code = code.as_deref().is_some_and(|c| !c.is_empty());
            if !has_code {
                // No mutation signal: caller keeps the attempt pending.
                return HandlerResponse::Err(HandlerError::CodeRequired);
            }
            match meta {
                Some(m) if valid_meta(m) => {}
                _ => return HandlerResponse::Err(HandlerError::InvalidRequest),
            }
            if *auth_failed {
                return HandlerResponse::Err(HandlerError::AuthFailed);
            }
            HandlerResponse::NoContent
        }
        HandlerOp::AttemptPoll => match meta {
            Some(m) if valid_meta(m) => HandlerResponse::Projection(m.clone()),
            Some(_) => HandlerResponse::Err(HandlerError::InvalidRequest),
            None => HandlerResponse::Err(HandlerError::NotFound),
        },
    }
}
