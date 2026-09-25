#![forbid(unsafe_code)]
//! Unified route state: Home, Session, Plugin (packages/tui/src/routes).
/// Max id chars for session/plugin ids (fail-closed truncate).
pub const MAX_ID: usize = 128;

/// Unified route: home default, session(id), or plugin(id).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Route2 {
    #[default]
    Home,
    Session(String),
    Plugin(String),
}

fn trunc(value: &str) -> String {
    value.chars().take(MAX_ID).collect()
}
impl Route2 {
    /// Session route; empty input collapses Home (fail-closed).
    #[must_use]
    pub fn open_session(id: &str) -> Self {
        if id.is_empty() {
            Self::Home
        } else {
            Self::Session(trunc(id))
        }
    }

    /// Plugin route; empty input collapses Home (fail-closed).
    #[must_use]
    pub fn open_plugin(id: &str) -> Self {
        if id.is_empty() {
            Self::Home
        } else {
            Self::Plugin(trunc(id))
        }
    }

    /// True only on `Home`.
    #[must_use]
    pub fn is_home(&self) -> bool {
        matches!(self, Self::Home)
    }
    /// Borrow held id; `None` for Home.
    #[must_use]
    pub fn id_of(&self) -> Option<&str> {
        match self {
            Self::Home => None,
            Self::Session(id) | Self::Plugin(id) => Some(id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_session_basic() {
        let r = Route2::open_session("ses_1");
        assert_eq!(r, Route2::Session("ses_1".to_string()));
        assert!(!r.is_home());
    }

    #[test]
    fn empty_goes_home() {
        assert_eq!(Route2::open_session(""), Route2::Home);
        assert_eq!(Route2::open_plugin(""), Route2::Home);
    }

    #[test]
    fn truncates_to_128() {
        for r in [
            Route2::open_session(&"x".repeat(200)),
            Route2::open_plugin(&"y".repeat(200)),
        ] {
            assert_eq!(r.id_of().unwrap().chars().count(), MAX_ID);
        }
    }

    #[test]
    fn id_of_and_is_home() {
        assert_eq!(Route2::Home.id_of(), None);
        assert!(Route2::Home.is_home());
        assert_eq!(Route2::open_session("abc").id_of(), Some("abc"));
        let p = Route2::open_plugin("plug");
        assert_eq!(p.id_of(), Some("plug"));
        assert!(!p.is_home());
    }
}
