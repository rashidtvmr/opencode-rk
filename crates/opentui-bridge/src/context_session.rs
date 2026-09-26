#![forbid(unsafe_code)]
//! Session/route/sync contract (std-only).
//!
//! Evidence (TS checkout a0d9b6c):
//! - `packages/tui/src/context/route.tsx:6-23`
//!   `HomeRoute{type:"home"}|SessionRoute{type:"session",sessionID}|PluginRoute{type:"plugin",id}`
//! - `packages/tui/src/context/sync.tsx:65` `status:"loading"|"partial"|"complete"`
//! - `packages/tui/src/context/sync.tsx:578-587` derived `session.status()`
//!   `"idle"|"compacting"|"working"`
//! - `packages/schema/src/session-status-event.ts:9-32`
//!   `Info idle|retry|busy` (`session.status` event payload)
//! - `packages/tui/src/context/sdk.tsx:86-116` SSE reconnect with exp backoff
//!   (connection drop -> retry loop)
//! - `packages/tui/src/context/location.tsx:1` `LocationRef{directory,workspaceID?}`
//! - `packages/tui/src/routes/session/index.tsx:203` title truncate 50
//! - `packages/opencode/src/session/session.ts:523` default title
//!   `parentTitlePrefix|childTitlePrefix + ISO`

/// Max session id chars.
pub const MAX_ID: usize = 256;
/// Max title chars (TS truncates to 50 for display; stored bound larger).
pub const MAX_TITLE: usize = 512;
/// Max workspace chars.
pub const MAX_WORKSPACE: usize = 1024;

/// Route names (`route.tsx:6-23` + home/session).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Home,
    Session { session_id: String },
    Plugin { id: String },
}

impl Route {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Session { .. } => "session",
            Self::Plugin { .. } => "plugin",
        }
    }

    /// Roundtrip via `"type"` + payload. Fail-closed `None`.
    #[must_use]
    pub fn from_parts(kind: &str, payload: &str) -> Option<Self> {
        match kind {
            "home" => Some(Self::Home),
            "session" if !payload.is_empty() => Some(Self::Session { session_id: payload.to_string() }),
            "plugin" if !payload.is_empty() => Some(Self::Plugin { id: payload.to_string() }),
            _ => None,
        }
    }

    #[must_use]
    pub fn payload(&self) -> &str {
        match self {
            Self::Home => "",
            Self::Session { session_id } => session_id,
            Self::Plugin { id } => id,
        }
    }
}

/// Connection kind (SSE loop `sdk.tsx:86-116`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncKind {
    Connected,
    Reconnecting,
    Offline,
}

impl SyncKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Reconnecting => "reconnecting",
            Self::Offline => "offline",
        }
    }

    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "connected" => Some(Self::Connected),
            "reconnecting" => Some(Self::Reconnecting),
            "offline" => Some(Self::Offline),
            _ => None,
        }
    }
}

/// Sync state: offline implies stale snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncState {
    pub kind: SyncKind,
    pub stale: bool,
}

impl SyncState {
    #[must_use]
    pub const fn new(kind: SyncKind) -> Self {
        Self { kind, stale: matches!(kind, SyncKind::Offline) }
    }

    #[must_use]
    pub const fn is_stale(self) -> bool {
        self.stale
    }

    pub fn set_kind(&mut self, kind: SyncKind) {
        self.kind = kind;
        if matches!(kind, SyncKind::Offline) {
            self.stale = true;
        }
    }

    pub const fn mark_fresh(&mut self) {
        self.stale = false;
    }
}

/// Session activity (`session-status-event.ts:9-32`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Idle,
    Busy,
    Retry,
}

impl SessionStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Busy => "busy",
            Self::Retry => "retry",
        }
    }

    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "idle" => Some(Self::Idle),
            "busy" => Some(Self::Busy),
            "retry" => Some(Self::Retry),
            _ => None,
        }
    }
}

/// Minimal session snapshot (id + bounded title + status).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnapshot {
    pub id: String,
    pub title: String,
    pub status: SessionStatus,
}

impl SessionSnapshot {
    pub fn new(id: &str, title: &str, status: SessionStatus) -> Result<Self, &'static str> {
        if id.is_empty() {
            return Err("id empty");
        }
        if id.len() > MAX_ID {
            return Err("id too long");
        }
        if title.chars().count() > MAX_TITLE {
            return Err("title too long");
        }
        Ok(Self { id: id.to_string(), title: title.to_string(), status })
    }
}

/// Location handle (`location.tsx:1`; workspace id only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub workspace: String,
}

impl Location {
    pub fn new(workspace: &str) -> Result<Self, &'static str> {
        if workspace.is_empty() {
            return Err("workspace empty");
        }
        if workspace.len() > MAX_WORKSPACE {
            return Err("workspace too long");
        }
        Ok(Self { workspace: workspace.to_string() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_roundtrip_all_variants() {
        for r in [
            Route::Home,
            Route::Session { session_id: "ses_1".into() },
            Route::Plugin { id: "p1".into() },
        ] {
            let back = Route::from_parts(r.as_str(), r.payload()).unwrap();
            assert_eq!(back, r);
        }
        assert_eq!(Route::from_parts("session", ""), None);
        assert_eq!(Route::from_parts("nope", "x"), None);
    }

    #[test]
    fn offline_marks_stale() {
        let s = SyncState::new(SyncKind::Offline);
        assert!(s.is_stale());
        let mut c = SyncState::new(SyncKind::Connected);
        assert!(!c.is_stale());
        c.set_kind(SyncKind::Offline);
        assert!(c.is_stale());
    }

    #[test]
    fn reconnecting_not_stale_kind_roundtrip() {
        for k in [SyncKind::Connected, SyncKind::Reconnecting, SyncKind::Offline] {
            assert_eq!(SyncKind::from_str(k.as_str()), Some(k));
        }
        assert_eq!(SyncKind::from_str("nope"), None);
        assert!(!SyncState::new(SyncKind::Reconnecting).is_stale());
    }

    #[test]
    fn empty_id_errs() {
        assert_eq!(SessionSnapshot::new("", "t", SessionStatus::Idle).unwrap_err(), "id empty");
        assert!(SessionSnapshot::new("a", "t", SessionStatus::Busy).is_ok());
    }

    #[test]
    fn title_bound_enforced() {
        let long = "t".repeat(MAX_TITLE + 1);
        assert!(SessionSnapshot::new("a", &long, SessionStatus::Retry).is_err());
        assert!(SessionSnapshot::new(&"i".repeat(MAX_ID + 1), "t", SessionStatus::Idle).is_err());
    }

    #[test]
    fn status_roundtrip_and_location() {
        for s in [SessionStatus::Idle, SessionStatus::Busy, SessionStatus::Retry] {
            assert_eq!(SessionStatus::from_str(s.as_str()), Some(s));
        }
        assert_eq!(SessionStatus::from_str("working"), None);
        assert!(Location::new("").is_err());
        assert_eq!(Location::new("ws1").unwrap().workspace, "ws1");
    }
}
