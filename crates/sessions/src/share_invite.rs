use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareInvite {
    pub email: String,
    pub role: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InviteError {
    #[error("email must not be empty")]
    EmptyEmail,
    #[error("bad email")]
    BadEmail,
    #[error("role must not be empty")]
    EmptyRole,
}

pub fn make_invite(email: &str, role: &str) -> Result<ShareInvite, InviteError> {
    let normalized = email.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(InviteError::EmptyEmail);
    }
    let Some((_, domain)) = normalized.split_once('@') else {
        return Err(InviteError::BadEmail);
    };
    if !domain.contains('.') {
        return Err(InviteError::BadEmail);
    }
    if role.trim().is_empty() {
        return Err(InviteError::EmptyRole);
    }
    Ok(ShareInvite {
        email: normalized,
        role: role.to_owned(),
    })
}
