use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Theme {
    Light,
    Dark,
    System,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ThemeError {
    #[error("unknown theme: {name}")]
    UnknownTheme { name: String },
}

pub fn parse_theme(name: &str) -> Result<Theme, ThemeError> {
    match name.trim().to_ascii_lowercase().as_str() {
        "light" => Ok(Theme::Light),
        "dark" => Ok(Theme::Dark),
        "system" => Ok(Theme::System),
        _ => Err(ThemeError::UnknownTheme {
            name: name.to_owned(),
        }),
    }
}

#[must_use]
pub fn theme_label(theme: &Theme) -> &'static str {
    match theme {
        Theme::Light => "Light",
        Theme::Dark => "Dark",
        Theme::System => "System",
    }
}
