use thiserror::Error;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum RevokeError {
    #[error("empty token")]
    EmptyToken,
    #[error("unknown token")]
    UnknownToken,
}

pub fn revoke_token(tokens: &mut Vec<String>, tok: &str) -> Result<(), RevokeError> {
    if tok.is_empty() {
        return Err(RevokeError::EmptyToken);
    }
    match tokens.iter().position(|t| t == tok) {
        Some(idx) => {
            tokens.remove(idx);
            Ok(())
        }
        None => Err(RevokeError::UnknownToken),
    }
}
