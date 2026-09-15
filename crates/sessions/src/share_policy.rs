use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Visibility {
    Private,
    Link,
    Workspace,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum SharePolicyError {
    #[error("unknown visibility: {name}")]
    UnknownVisibility { name: String },
}

pub fn parse_visibility(name: &str) -> Result<Visibility, SharePolicyError> {
    match name.trim().to_ascii_lowercase().as_str() {
        "private" => Ok(Visibility::Private),
        "link" => Ok(Visibility::Link),
        "workspace" => Ok(Visibility::Workspace),
        _ => Err(SharePolicyError::UnknownVisibility {
            name: name.to_owned(),
        }),
    }
}

#[must_use]
pub fn visibility_label(visibility: &Visibility) -> &'static str {
    match visibility {
        Visibility::Private => "Private",
        Visibility::Link => "Link",
        Visibility::Workspace => "Workspace",
    }
}
