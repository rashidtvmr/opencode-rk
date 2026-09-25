#![forbid(unsafe_code)]
//! Session route state (mirrors `packages/tui/src/routes/session/index.tsx:186`).
//!
//! Evidence: `index.tsx:186` `useRouteData("session")`, `:517-530`
//! `DialogTimeline`, `:540-552` `DialogForkFromTimeline`; default view is chat.

/// Max session id chars (fail-closed).
pub const MAX_SESSION_ID: usize = 64;

/// Session sub-view (chat default; timeline/fork dialogs in TS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RouteView {
    #[default]
    Chat,
    Timeline,
    Fork,
}

impl RouteView {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Timeline => "timeline",
            Self::Fork => "fork",
        }
    }
}

/// Session route: id + active view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRoute {
    pub session_id: String,
    pub view: RouteView,
}

impl SessionRoute {
    /// Fail-closed constructor; empty/too-long errs.
    pub fn open(id: &str) -> Result<Self, String> {
        if id.is_empty() {
            return Err("empty session id".to_string());
        }
        if id.chars().count() > MAX_SESSION_ID {
            return Err("session id too long".to_string());
        }
        Ok(Self {
            session_id: id.to_string(),
            view: RouteView::Chat,
        })
    }

    pub fn switch_view(&mut self, view: RouteView) {
        self.view = view;
    }

    #[must_use]
    pub fn title(&self) -> String {
        let short: String = self.session_id.chars().take(8).collect();
        format!("session {} [{}]", short, self.view.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_empty_errs() {
        assert!(SessionRoute::open("").is_err());
        assert!(SessionRoute::open(&"x".repeat(MAX_SESSION_ID + 1)).is_err());
    }

    #[test]
    fn switch_ok() {
        let mut r = SessionRoute::open("ses_1").unwrap();
        r.switch_view(RouteView::Timeline);
        assert_eq!(r.view, RouteView::Timeline);
    }

    #[test]
    fn title_truncates_id() {
        let r = SessionRoute::open("abcdefghijklmnop").unwrap();
        assert_eq!(r.title(), "session abcdefgh [chat]");
    }

    #[test]
    fn default_chat() {
        assert_eq!(RouteView::default(), RouteView::Chat);
        assert_eq!(SessionRoute::open("s1").unwrap().view, RouteView::Chat);
    }

    #[test]
    fn view_roundtrip() {
        let mut r = SessionRoute::open("s1").unwrap();
        r.switch_view(RouteView::Timeline);
        r.switch_view(RouteView::Fork);
        assert_eq!(r.title(), "session s1 [fork]");
        r.switch_view(RouteView::Chat);
        assert_eq!(r.view, RouteView::Chat);
    }
}
