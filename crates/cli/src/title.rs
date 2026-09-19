#![forbid(unsafe_code)]
//! App title state: pure state only, no rendering IO.

/// Max chars retained for app name.
pub const MAX_APP: usize = 64;
/// Max chars retained for session name.
pub const MAX_SESSION: usize = 128;
/// Max chars in rendered title.
pub const MAX_RENDER: usize = 256;

/// Title validation failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TitleError {
    EmptySession,
}

impl std::fmt::Display for TitleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TitleError::EmptySession => write!(f, "session title must be non-empty"),
        }
    }
}

impl std::error::Error for TitleError {}

/// App + optional session title. All bounds are char counts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppTitle {
    app: String,
    session: Option<String>,
}

impl AppTitle {
    #[must_use]
    pub fn new(app: impl Into<String>) -> Self {
        Self {
            app: truncate_chars(&app.into(), MAX_APP),
            session: None,
        }
    }

    #[must_use]
    pub fn app(&self) -> &str {
        &self.app
    }

    #[must_use]
    pub fn session(&self) -> Option<&str> {
        self.session.as_deref()
    }

    /// Set session title; rejects empty/whitespace-only input.
    pub fn set_session(&mut self, session: impl Into<String>) -> Result<(), TitleError> {
        let s = session.into();
        if s.trim().is_empty() {
            return Err(TitleError::EmptySession);
        }
        self.session = Some(truncate_chars(&s, MAX_SESSION));
        Ok(())
    }

    pub fn clear_session(&mut self) {
        self.session = None;
    }

    /// `"app — session"` or app alone, capped at `MAX_RENDER` with ellipsis.
    #[must_use]
    pub fn render(&self) -> String {
        let full = match &self.session {
            Some(s) => format!("{} — {s}", self.app),
            None => self.app.clone(),
        };
        if full.chars().count() <= MAX_RENDER {
            return full;
        }
        // ponytail: single-char ellipsis; upgrade to word-boundary wrap when TUI needs it.
        let mut out: String = full.chars().take(MAX_RENDER.saturating_sub(1)).collect();
        out.push('…');
        out
    }

    /// Status line fitted to `width` chars: `render()` plus `" ●"` when dirty.
    #[must_use]
    pub fn status_bar(&self, width: usize, dirty: bool) -> String {
        let mut base = self.render();
        if dirty {
            base.push_str(" ●");
        }
        let len = base.chars().count();
        if len == width {
            base
        } else if len > width {
            base.chars().take(width).collect()
        } else {
            // ponytail: space-pad only; upgrade to alignment/ellipsis when TUI needs it.
            let mut out = base;
            out.extend(std::iter::repeat(' ').take(width - len));
            out
        }
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_session_rejected() {
        let mut t = AppTitle::new("opencode");
        assert_eq!(t.set_session(""), Err(TitleError::EmptySession));
        assert_eq!(t.set_session("   "), Err(TitleError::EmptySession));
        assert_eq!(t.session(), None);
    }

    #[test]
    fn render_app_alone_or_with_session() {
        let t = AppTitle::new("opencode");
        assert_eq!(t.render(), "opencode");
        let mut t = t;
        t.set_session("fix login").unwrap();
        assert_eq!(t.render(), "opencode — fix login");
    }

    #[test]
    fn caps_truncate_and_render_stays_in_budget() {
        let mut t = AppTitle::new("a".repeat(MAX_APP + 10));
        assert_eq!(t.app().chars().count(), MAX_APP);
        t.set_session("s".repeat(MAX_SESSION + 10)).unwrap();
        assert_eq!(t.session().unwrap().chars().count(), MAX_SESSION);
        // Capped parts (64 + 3 + 128 = 195) always fit; render enforces
        // MAX_RENDER defensively with ellipsis if invariants ever change.
        let r = t.render();
        assert!(r.chars().count() <= MAX_RENDER);
        assert!(r.starts_with(&"a".repeat(MAX_APP)));
        assert!(r.ends_with(&"s".repeat(MAX_SESSION)));
    }
}
