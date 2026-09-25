//! App boot config: initial route + onboarding flag.
//!
//! Upstream: `packages/tui/src/context/route.tsx:6-23`
//! (`Route = Home | Session(sessionID) | Plugin`) and
//! `packages/tui/src/app.tsx:54-55` (`Home`, `Session` routes).
//! Plugin routes intentionally out of scope here.

#![forbid(unsafe_code)]

/// Max session id chars retained by [`AppEntry::session`].
pub const MAX_SESSION_ID_LEN: usize = 64;

/// Boot route. Home or a capped session id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppRoute {
    Home,
    Session(String),
}

/// Boot config: where to land plus onboarding state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppEntry {
    pub route: AppRoute,
    pub onboarded: bool,
}

impl AppEntry {
    /// Land on home. Onboarding defaults to not-done.
    #[must_use]
    pub fn home() -> Self {
        Self {
            route: AppRoute::Home,
            onboarded: false,
        }
    }

    /// Land on a session. Empty id errs; longer ids truncate to
    /// [`MAX_SESSION_ID_LEN`] chars.
    pub fn session(id: &str) -> Result<Self, String> {
        if id.is_empty() {
            return Err(String::from("session id must not be empty"));
        }
        let capped: String = id.chars().take(MAX_SESSION_ID_LEN).collect();
        Ok(Self {
            route: AppRoute::Session(capped),
            onboarded: false,
        })
    }

    /// `"home"` or `"session <first-8-chars>"`.
    #[must_use]
    pub fn route_label(&self) -> String {
        match &self.route {
            AppRoute::Home => String::from("home"),
            AppRoute::Session(id) => {
                let short: String = id.chars().take(8).collect();
                format!("session {short}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_label() {
        assert_eq!(AppEntry::home().route_label(), "home");
    }

    #[test]
    fn onboard_default_false() {
        assert!(!AppEntry::home().onboarded);
        assert!(!AppEntry::session("abc").unwrap().onboarded);
    }

    #[test]
    fn session_ok() {
        let entry = AppEntry::session("ses_123").unwrap();
        assert_eq!(entry.route, AppRoute::Session(String::from("ses_123")));
        assert_eq!(entry.route_label(), "session ses_123");
    }

    #[test]
    fn session_empty_errs() {
        assert!(AppEntry::session("").is_err());
    }

    #[test]
    fn session_id_truncates() {
        let long = "s".repeat(MAX_SESSION_ID_LEN + 10);
        let entry = AppEntry::session(&long).unwrap();
        let AppRoute::Session(id) = &entry.route else {
            panic!("expected session")
        };
        assert_eq!(id.len(), MAX_SESSION_ID_LEN);
        assert_eq!(entry.route_label(), format!("session {}", "s".repeat(8)));
    }
}
