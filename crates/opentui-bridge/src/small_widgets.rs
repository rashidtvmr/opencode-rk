#![forbid(unsafe_code)]
//! Small widget contracts (checkout a0d9b6c, NOT 95daf90).
//! link.tsx:5-12 LinkProps{href,children}; :28 open(href).
//! plugin-route-missing.tsx:8 "Unknown plugin route: {id}".
//! startup-loading.tsx:6 text ready?"Finishing startup...":"Loading plugins...".
//! todo-item.tsx:19 marker completed="✓" in_progress="•" else " ".
//! use-connected.tsx:6-10 some provider id!="opencode" or cost.input!=0.
//! workspace-label.tsx:16 "{name} ({type})".
//! register-spinner.ts:5 register iff catalogue.spinner missing.

/// href byte bound (fail-closed).
pub const MAX_HREF: usize = 2048;
/// route/text bounds.
pub const MAX_ROUTE: usize = 512;
pub const MAX_TEXT: usize = 1024;
pub const MAX_PATH: usize = 4096;
/// startup-loading.tsx:42 show delay, :22 minimum visible hold.
pub const SHOW_DELAY_MS: u64 = 500;
pub const MIN_SHOW_MS: u64 = 3000;
/// todo-item.tsx:4 status bound; content reuses MAX_TEXT (1024).
pub const MAX_TODO_STATUS: usize = 32;
/// todo-item.tsx:19 in-progress marker.
pub const IN_PROGRESS_MARKER: &str = "•";
/// workspace-label.tsx:5 status bound.
pub const MAX_WS_STATUS: usize = 64;
/// spinner.tsx:10 frame count bound.
pub const MAX_FRAMES: usize = 32;

/// Validated hyperlink. TS link.tsx:5 `href: string`, :28 opens on click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub href: String,
    pub label: String,
}

impl Link {
    pub fn new(href: &str, label: &str) -> Result<Self, &'static str> {
        if href.is_empty() {
            return Err("href empty");
        }
        if href.len() > MAX_HREF {
            return Err("href too long");
        }
        if !(href.starts_with("http://") || href.starts_with("https://")) {
            return Err("href scheme");
        }
        if label.chars().count() > MAX_TEXT {
            return Err("label too long");
        }
        Ok(Self { href: href.to_string(), label: label.to_string() })
    }

    /// Display text: explicit label else href (link.tsx:19 `children ?? href`).
    #[must_use]
    pub fn display(&self) -> &str {
        if self.label.is_empty() { &self.href } else { &self.label }
    }
}

/// TS plugin-route-missing.tsx:8. `onHome: () => void` callback has no
/// Rust equivalent; `home_action` is an opaque action-id naming that callback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteMissing {
    pub route: String,
    pub home_action: Option<String>,
}

impl RouteMissing {
    pub fn new(route: &str) -> Result<Self, &'static str> {
        if route.is_empty() {
            return Err("route empty");
        }
        if route.chars().count() > MAX_ROUTE {
            return Err("route too long");
        }
        Ok(Self { route: route.to_string(), home_action: None })
    }

    pub fn with_home_action(mut self, action: &str) -> Result<Self, &'static str> {
        if action.is_empty() {
            return Err("action empty");
        }
        if action.chars().count() > MAX_ROUTE {
            return Err("action too long");
        }
        self.home_action = Some(action.to_string());
        Ok(self)
    }

    #[must_use]
    pub fn message(&self) -> String {
        format!("Unknown plugin route: {}", self.route)
    }
}

/// Startup phases (startup-loading.tsx:6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupPhase {
    LoadingPlugins,
    Finishing,
}

impl StartupPhase {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::LoadingPlugins => "Loading plugins...",
            Self::Finishing => "Finishing startup...",
        }
    }
}

/// startup-loading.tsx:42 gate (elapsed < SHOW_DELAY_MS hides),
/// :22 hold (visible until elapsed >= MIN_SHOW_MS). Pure predicate; the
/// TS timers/side-effects are UI-runtime only and not ported.
#[must_use]
pub const fn should_show(elapsed_ms: u64) -> bool {
    elapsed_ms >= SHOW_DELAY_MS
}

/// Remaining hold: MIN_SHOW_MS saturating minus elapsed since shown.
#[must_use]
pub const fn hold_remaining_ms(elapsed_since_shown_ms: u64) -> u64 {
    MIN_SHOW_MS.saturating_sub(elapsed_since_shown_ms)
}

/// Ordered startup steps; advance marks current done, moves cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupStep {
    pub label: String,
    pub done: bool,
}

#[derive(Debug, Default, Clone)]
pub struct StartupSequence {
    steps: Vec<StartupStep>,
    cursor: usize,
}

impl StartupSequence {
    #[must_use]
    pub fn new(labels: &[&str]) -> Self {
        Self {
            steps: labels.iter().map(|l| StartupStep { label: l.to_string(), done: false }).collect(),
            cursor: 0,
        }
    }

    /// Advance: mark current done, return its label; None when drained.
    pub fn advance(&mut self) -> Option<&str> {
        if self.cursor >= self.steps.len() {
            return None;
        }
        self.steps[self.cursor].done = true;
        self.cursor += 1;
        Some(self.steps[self.cursor - 1].label.as_str())
    }

    #[must_use]
    pub fn done(&self) -> bool {
        self.cursor >= self.steps.len()
    }

    #[must_use]
    pub fn steps(&self) -> &[StartupStep] {
        &self.steps
    }
}

/// TS todo-item.tsx:19 marker map. TS uses `status: string` + `content: string`
/// (:4-5); `text` maps to content, `done` maps to status=="completed", and
/// `status`/`content` are kept as additive validated fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoItem {
    pub text: String,
    pub done: bool,
    pub status: String,
    pub content: String,
}

impl TodoItem {
    pub fn new(text: &str, done: bool) -> Result<Self, &'static str> {
        if text.is_empty() {
            return Err("text empty");
        }
        if text.chars().count() > MAX_TEXT {
            return Err("text too long");
        }
        let status = if done { "completed" } else { "pending" };
        Ok(Self { text: text.to_string(), done, status: status.to_string(), content: text.to_string() })
    }

    pub fn with_status(status: &str, content: &str) -> Result<Self, &'static str> {
        if status.chars().count() > MAX_TODO_STATUS {
            return Err("status too long");
        }
        if content.is_empty() {
            return Err("text empty");
        }
        if content.chars().count() > MAX_TEXT {
            return Err("text too long");
        }
        Ok(Self {
            text: content.to_string(),
            done: status == "completed",
            status: status.to_string(),
            content: content.to_string(),
        })
    }

    pub fn toggle(&mut self) {
        self.done = !self.done;
    }

    /// `[✓]` done, `[•]` in-progress (render-only status), else `[ ]`.
    /// `status` field drives the in-progress branch.
    #[must_use]
    pub fn marker(&self) -> &'static str {
        if self.done { "[✓]" } else if self.status == IN_PROGRESS_MARKER || self.status == "in_progress" { "[•]" } else { "[ ]" }
    }
}

/// TS use-connected.tsx:4-10 derives bool from `providers: Provider[]` (:7-9).
/// The Provider[] model cannot port without sync context, so this is the
/// boolean predicate projection: true iff at least one provider is present.
#[must_use]
pub const fn providers_present(present: bool) -> bool {
    present
}

/// TS use-connected.tsx:6-10 boolean projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Connection {
    pub online: bool,
}

impl Connection {
    #[must_use]
    pub const fn new(online: bool) -> Self {
        Self { online }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        if self.online { "connected" } else { "disconnected" }
    }
}

/// TS workspace-label.tsx:16 `{name} ({type})`; path variant keeps basename.
/// TS also takes `status?: WorkspaceStatus` + `icon?: boolean` (:5);
/// kept as additive fields (status bounded 64, icon flag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceLabel {
    pub path: String,
    pub status: Option<String>,
    pub icon: bool,
}

impl WorkspaceLabel {
    pub fn new(path: &str) -> Result<Self, &'static str> {
        if path.is_empty() {
            return Err("path empty");
        }
        if path.chars().count() > MAX_PATH {
            return Err("path too long");
        }
        Ok(Self { path: path.to_string(), status: None, icon: false })
    }

    pub fn with_status(mut self, status: &str) -> Result<Self, &'static str> {
        if status.chars().count() > MAX_WS_STATUS {
            return Err("status too long");
        }
        self.status = Some(status.to_string());
        Ok(self)
    }

    #[must_use]
    pub fn with_icon(mut self, icon: bool) -> Self {
        self.icon = icon;
        self
    }

    /// Basename after `/` or `\`; trailing slashes trimmed; `/` stays `/`.
    #[must_use]
    pub fn short(&self) -> &str {
        let t = self.path.trim_end_matches(['/', '\\']);
        let base = t.rsplit(['/', '\\']).next().unwrap_or(t);
        if base.is_empty() { "/" } else { base }
    }

    #[must_use]
    pub fn display(&self, kind: &str) -> String {
        format!("{} ({kind})", self.short())
    }
}

/// spinner.tsx:10 frames; :17 `animations_enabled` kv flag selects the
/// static `⋯` fallback when false. register-spinner.ts:4 registers iff the
/// component catalogue lacks `spinner`; here: build the def iff missing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpinnerDef {
    pub frames: Vec<String>,
    pub animations_enabled: bool,
}

impl SpinnerDef {
    pub fn new(frames: &[&str], animations_enabled: bool) -> Result<Self, &'static str> {
        if frames.is_empty() {
            return Err("frames empty");
        }
        if frames.len() > MAX_FRAMES {
            return Err("frames too many");
        }
        Ok(Self { frames: frames.iter().map(|f| f.to_string()).collect(), animations_enabled })
    }

    /// Static fallback glyph used when animations are disabled (:17).
    #[must_use]
    pub const fn fallback() -> &'static str {
        "⋯"
    }
}

/// Port of register-spinner.ts:4: return Some(def) iff catalogue lacks
/// spinner (`spinner_present == false`), else None (already registered).
#[must_use]
pub fn register_opencode_spinner(spinner_present: bool) -> Option<SpinnerDef> {
    if spinner_present {
        return None;
    }
    SpinnerDef::new(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"], true).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_validate_scheme_bound() {
        assert!(Link::new("https://a.b", "").is_ok());
        assert_eq!(Link::new("https://a.b", "").unwrap().display(), "https://a.b");
        assert_eq!(Link::new("http://a.b", "x").unwrap().display(), "x");
        assert!(Link::new("ftp://a.b", "x").is_err());
        assert!(Link::new("", "x").is_err());
        assert!(Link::new(&format!("https://a/{}", "x".repeat(MAX_HREF)), "x").is_err());
    }

    #[test]
    fn route_missing_message() {
        let r = RouteMissing::new("foo").unwrap();
        assert_eq!(r.message(), "Unknown plugin route: foo");
        assert!(RouteMissing::new("").is_err());
    }

    #[test]
    fn startup_phase_labels_and_advance() {
        assert_eq!(StartupPhase::LoadingPlugins.label(), "Loading plugins...");
        assert_eq!(StartupPhase::Finishing.label(), "Finishing startup...");
        let mut s = StartupSequence::new(&["a", "b"]);
        assert_eq!(s.advance(), Some("a"));
        assert!(!s.done());
        assert!(s.steps()[0].done);
        assert_eq!(s.advance(), Some("b"));
        assert!(s.done());
        assert_eq!(s.advance(), None);
    }

    #[test]
    fn todo_toggle_marker() {
        let mut t = TodoItem::new("write", false).unwrap();
        assert_eq!(t.marker(), "[ ]");
        t.toggle();
        assert_eq!(t.marker(), "[✓]");
        t.toggle();
        assert!(!t.done);
        assert!(TodoItem::new("", false).is_err());
    }

    #[test]
    fn connection_label() {
        assert_eq!(Connection::new(true).label(), "connected");
        assert_eq!(Connection::new(false).label(), "disconnected");
    }

    #[test]
    fn workspace_short_and_display() {
        assert_eq!(WorkspaceLabel::new("/a/b/c").unwrap().short(), "c");
        assert_eq!(WorkspaceLabel::new("C:\\a\\b").unwrap().short(), "b");
        assert_eq!(WorkspaceLabel::new("/a/b/").unwrap().short(), "b");
        assert_eq!(WorkspaceLabel::new("/").unwrap().short(), "/");
        assert_eq!(WorkspaceLabel::new("/a/b").unwrap().display("local"), "b (local)");
        assert!(WorkspaceLabel::new("").is_err());
    }

    #[test]
    fn show_delay_and_hold() {
        assert_eq!(SHOW_DELAY_MS, 500);
        assert_eq!(MIN_SHOW_MS, 3000);
        assert!(!should_show(0));
        assert!(!should_show(499));
        assert!(should_show(500));
        assert_eq!(hold_remaining_ms(0), 3000);
        assert_eq!(hold_remaining_ms(3000), 0);
        assert_eq!(hold_remaining_ms(9999), 0);
    }

    #[test]
    fn todo_status_content_mapping() {
        let t = TodoItem::with_status("in_progress", "write").unwrap();
        assert_eq!(t.marker(), "[•]");
        assert_eq!(t.text, "write");
        assert_eq!(t.content, "write");
        assert!(!t.done);
        let d = TodoItem::with_status("completed", "x").unwrap();
        assert!(d.done);
        assert_eq!(d.marker(), "[✓]");
        assert_eq!(IN_PROGRESS_MARKER, "•");
        assert!(TodoItem::with_status(&"s".repeat(33), "x").is_err());
        assert!(TodoItem::with_status("pending", "").is_err());
    }

    #[test]
    fn providers_present_predicate() {
        assert!(providers_present(true));
        assert!(!providers_present(false));
    }

    #[test]
    fn workspace_status_icon_additive() {
        let w = WorkspaceLabel::new("/a/b").unwrap().with_status("connected").unwrap().with_icon(true);
        assert_eq!(w.status.as_deref(), Some("connected"));
        assert!(w.icon);
        assert_eq!(w.display("local"), "b (local)");
        assert!(WorkspaceLabel::new("/a").unwrap().with_status(&"s".repeat(65)).is_err());
        assert!(WorkspaceLabel::new("/a").unwrap().status.is_none());
    }

    #[test]
    fn route_missing_home_action() {
        let r = RouteMissing::new("foo").unwrap().with_home_action("go-home").unwrap();
        assert_eq!(r.message(), "Unknown plugin route: foo");
        assert_eq!(r.home_action.as_deref(), Some("go-home"));
        assert!(RouteMissing::new("foo").unwrap().home_action.is_none());
        assert!(RouteMissing::new("foo").unwrap().with_home_action("").is_err());
    }

    #[test]
    fn spinner_def_and_register() {
        let d = register_opencode_spinner(false).unwrap();
        assert_eq!(d.frames.len(), 10);
        assert_eq!(d.frames[0], "⠋");
        assert!(d.animations_enabled);
        assert_eq!(SpinnerDef::fallback(), "⋯");
        assert!(register_opencode_spinner(true).is_none());
        assert!(SpinnerDef::new(&[], true).is_err());
    }
}
