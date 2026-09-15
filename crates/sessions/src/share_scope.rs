use thiserror::Error;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ScopeError {
    #[error("empty scope")]
    EmptyScope,
    #[error("too many scopes: max {max}, actual {actual}")]
    TooMany { max: usize, actual: usize },
}

pub const MAX_SCOPES: usize = 32;

pub fn grant_scope(list: &mut Vec<String>, scope: &str) -> Result<(), ScopeError> {
    if scope.is_empty() {
        return Err(ScopeError::EmptyScope);
    }
    if list.iter().any(|s| s == scope) {
        return Ok(());
    }
    if list.len() >= MAX_SCOPES {
        return Err(ScopeError::TooMany {
            max: MAX_SCOPES,
            actual: list.len(),
        });
    }
    list.push(scope.to_owned());
    Ok(())
}

pub fn has_scope(list: &[String], scope: &str) -> bool {
    list.iter().any(|s| s == scope)
}
