use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShareAudit {
    pub actor: String,
    pub action: String,
    pub seq: u64,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum AuditError {
    #[error("empty actor")]
    EmptyActor,
    #[error("empty action")]
    EmptyAction,
    #[error("audit buffer full: max {max}, actual {actual}")]
    TooMany { max: usize, actual: usize },
}

pub const MAX_SHARE_AUDIT: usize = 512;

pub fn record_audit(
    buf: &mut Vec<ShareAudit>,
    actor: &str,
    action: &str,
) -> Result<(), AuditError> {
    if actor.is_empty() {
        return Err(AuditError::EmptyActor);
    }
    if action.is_empty() {
        return Err(AuditError::EmptyAction);
    }
    if buf.len() >= MAX_SHARE_AUDIT {
        return Err(AuditError::TooMany {
            max: MAX_SHARE_AUDIT,
            actual: buf.len(),
        });
    }
    buf.push(ShareAudit {
        actor: actor.to_owned(),
        action: action.to_owned(),
        seq: buf.len() as u64 + 1,
    });
    Ok(())
}
