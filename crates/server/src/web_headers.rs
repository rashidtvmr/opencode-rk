//! Pure security-header builder.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecHeaderError {
    EmptyName,
    EmptyValue,
}

impl std::fmt::Display for SecHeaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyName => write!(f, "empty name"),
            Self::EmptyValue => write!(f, "empty value"),
        }
    }
}

impl std::error::Error for SecHeaderError {}

pub fn make_header(name: &str, value: &str) -> Result<SecHeader, SecHeaderError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(SecHeaderError::EmptyName);
    }
    let value = value.trim();
    if value.is_empty() {
        return Err(SecHeaderError::EmptyValue);
    }
    Ok(SecHeader {
        name: name.to_string(),
        value: value.to_string(),
    })
}
