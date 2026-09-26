#![forbid(unsafe_code)]
//! Route page state (mirrors `packages/tui/src/routes/*`, TS checkout a0d9b6c).
//!
//! Evidence:
//! - `routes/home.tsx:22-71` Home page: prompt seed (`route.prompt`, `args.prompt`), no tabs.
//! - `routes/home/session-destination.tsx:13` `HomeSessionDestination =
//!   {directory,subdirectory} | {new}`; port models post-submit target instead:
//!   `New | Resume(id) | Fork(id)` (fork target `dialog-fork-from-timeline.tsx:48-52`
//!   `session.fork({sessionID,messageID})`, resume `dialog-subagent.tsx:16-19`
//!   navigate session).
//! - `routes/session/index.tsx:186 route=useRouteData("session")`, `:339-345`
//!   prompt seed `route.prompt`, `:249-250` sidebar signals,
//!   `:517-530` DialogTimeline `{sessionID,onMove,setPrompt}`,
//!   `:540-552` DialogForkFromTimeline `{sessionID,onMove}`,
//!   `:1258-1265` DialogMessage `{messageID,sessionID,setPrompt}`,
//!   `:1283-1294` PermissionPrompt `{request,directory?}` (permission.tsx:111),
//!   QuestionPrompt `{request,directory?}` (question.tsx:14),
//!   `:1295-1297` SubagentFooter for child sessions (subagent-footer.tsx:11).
//! - `component/dialog-session-rename.tsx:7-9` `{session: string}`.
//! - `ui/dialog-alert.tsx:6-10,59` `{title,message}` + `show(dialog,title,message)`.
//! - `ui/dialog-confirm.tsx:9-15,93` `{title,message,label?}` + `show(dialog,title,message,label?)`.
//! - `ui/dialog-export-options.tsx:8-22,187` `{defaultFilename,defaultThinking,
//!   defaultToolDetails,defaultAssistantMetadata,defaultOpenWithoutSaving}` +
//!   confirm options `{filename,thinking,toolDetails,assistantMetadata,openWithoutSaving}`.
//! - `component/dialog-retry-action.tsx:15-21,150`
//!   `{title,message,label,link?}` + `show(dialog,{title,message,label,link})`;
//!   call site `routes/session/index.tsx:97,364` (`RetryAction` status action).
//! - `routes/session/index.tsx:249-269` sidebar `kv<"auto"|"hide">` + `sidebarOpen`
//!   signal + `wide = width > 120` memo + `sidebarVisible` memo; `:255-261` kv
//!   toggles (`timestamps` hide/show, `tool_details_visibility` true,
//!   `scrollbar_visible` false, conceal `createSignal(true)`, `showThinking()=true`).
//!
//! Destination-type correction: TS `HomeSessionDestination` (`session-destination.tsx:13`)
//! is a pre-submit directory picker (`{directory,subdirectory} | {new}`), while the
//! pre-existing `SessionDestination` (`New|Resume|Fork`) models the post-submit fork/resume
//! target. Both are kept: `HomeDestination` below carries the TS picker shape plus a
//! `phase` pinning the existing enum, so no existing variant is renamed or removed.

/// Max draft input chars (fail-closed; mirrors dialog `MAX_INITIAL` scale).
pub const MAX_DRAFT: usize = 16384;
/// Max id chars (session/message/request ids).
pub const MAX_ID: usize = 256;
/// Max plugin route data chars (serialized JSON of `Record<string, unknown>`; fail-closed).
pub const MAX_DATA: usize = 4096;
/// Max home directory / dialog filename / link chars.
pub const MAX_DIR: usize = 1024;
/// Max dialog title/message chars.
pub const MAX_TEXT: usize = 1024;
/// Max prompt mode chars (`history.tsx:10` `"normal" | "shell"`; whitelist-enforced).
pub const MAX_MODE: usize = 32;

/// Post-submit destination for the home prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionDestination {
    New,
    Resume(String),
    Fork(String),
}

impl SessionDestination {
    fn validate_id(id: &str) -> bool {
        !id.is_empty() && id.chars().count() <= MAX_ID
    }

    /// Fail-closed constructor for id-carrying variants.
    #[must_use]
    pub fn resume(id: &str) -> Option<Self> {
        Self::validate_id(id).then(|| Self::Resume(id.to_string()))
    }

    /// Fail-closed constructor for id-carrying variants.
    #[must_use]
    pub fn fork(id: &str) -> Option<Self> {
        Self::validate_id(id).then(|| Self::Fork(id.to_string()))
    }
}

/// Open dialog on the session page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionDialog {
    ForkFromTimeline { session_id: String },
    Message { session_id: String, message_id: String },
    Subagent { session_id: String },
    Timeline { session_id: String },
    Permission { request_id: String, directory: Option<String> },
    Question { request_id: String, directory: Option<String> },
    Rename { session: String },
    Alert { title: String, message: String },
    Confirm { title: String, message: String, label: Option<String> },
    Export { filename: String, thinking: bool, tool_details: bool, assistant_metadata: bool, open_without_saving: bool },
    RetryAction { title: String, message: String, label: String, link: Option<String> },
}

impl SessionDialog {
    fn id_ok(id: &str) -> bool {
        !id.is_empty() && id.chars().count() <= MAX_ID
    }

    #[must_use]
    pub fn fork_from_timeline(session_id: &str) -> Option<Self> {
        Self::id_ok(session_id).then(|| Self::ForkFromTimeline { session_id: session_id.to_string() })
    }

    #[must_use]
    pub fn message(session_id: &str, message_id: &str) -> Option<Self> {
        (Self::id_ok(session_id) && Self::id_ok(message_id)).then(|| Self::Message {
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
        })
    }

    #[must_use]
    pub fn subagent(session_id: &str) -> Option<Self> {
        Self::id_ok(session_id).then(|| Self::Subagent { session_id: session_id.to_string() })
    }

    #[must_use]
    pub fn timeline(session_id: &str) -> Option<Self> {
        Self::id_ok(session_id).then(|| Self::Timeline { session_id: session_id.to_string() })
    }

    #[must_use]
    pub fn permission(request_id: &str, directory: Option<&str>) -> Option<Self> {
        let dir = match directory {
            Some(d) if d.chars().count() > MAX_ID => return None,
            Some(d) => Some(d.to_string()),
            None => None,
        };
        Self::id_ok(request_id).then(|| Self::Permission { request_id: request_id.to_string(), directory: dir })
    }

    #[must_use]
    pub fn question(request_id: &str, directory: Option<&str>) -> Option<Self> {
        let dir = match directory {
            Some(d) if d.chars().count() > MAX_ID => return None,
            Some(d) => Some(d.to_string()),
            None => None,
        };
        Self::id_ok(request_id).then(|| Self::Question { request_id: request_id.to_string(), directory: dir })
    }
}

/// Session page state (`routes/session/index.tsx:178 Session`, route `context/route.tsx:11-15`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionPage {
    pub session_id: String,
    pub draft: String,
    pub draft_info: Option<PromptDraft>,
    pub dialog: Option<SessionDialog>,
    pub sidebar_open: bool,
    pub sidebar_mode: SidebarMode,
    pub toggles: KvToggles,
}

impl SessionPage {
    pub fn new(session_id: &str) -> Result<Self, &'static str> {
        if session_id.is_empty() || session_id.chars().count() > MAX_ID {
            return Err("bad session id");
        }
        Ok(Self { session_id: session_id.to_string(), draft: String::new(), draft_info: None, dialog: None, sidebar_open: false, sidebar_mode: SidebarMode::Auto, toggles: KvToggles::defaults() })
    }

    pub fn set_draft(&mut self, draft: &str) -> Result<(), &'static str> {
        if draft.chars().count() > MAX_DRAFT {
            return Err("draft too long");
        }
        draft.clone_into(&mut self.draft);
        self.draft_info = Some(PromptDraft { input: self.draft.clone(), mode: None });
        Ok(())
    }

    pub fn set_draft_info(&mut self, draft: PromptDraft) -> Result<(), &'static str> {
        if draft.input.chars().count() > MAX_DRAFT {
            return Err("draft too long");
        }
        draft.input.clone_into(&mut self.draft);
        self.draft_info = Some(draft);
        Ok(())
    }

    pub fn open_dialog(&mut self, dialog: SessionDialog) {
        self.dialog = Some(dialog);
    }

    pub fn close_dialog(&mut self) {
        self.dialog = None;
    }

    pub fn toggle_sidebar(&mut self) {
        self.sidebar_open = !self.sidebar_open;
    }
}

/// Home page state (`routes/home.tsx:22 Home`, destination `session-destination.tsx:13`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomePage {
    pub destination: SessionDestination,
    pub draft: String,
}

impl HomePage {
    #[must_use]
    pub fn new(destination: SessionDestination) -> Self {
        Self { destination, draft: String::new() }
    }

    pub fn set_draft(&mut self, draft: &str) -> Result<(), &'static str> {
        if draft.chars().count() > MAX_DRAFT {
            return Err("draft too long");
        }
        draft.clone_into(&mut self.draft);
        Ok(())
    }
}

/// Plugin route (`context/route.tsx:17-21` `PluginRoute{id,data?: Record<string,unknown>}`).
///
/// `data` carried as its serialized JSON string, bounded fail-closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginRoute {
    pub id: String,
    pub data: Option<String>,
}

impl PluginRoute {
    #[must_use]
    pub fn new(id: &str, data: Option<&str>) -> Option<Self> {
        if id.is_empty() || id.chars().count() > MAX_ID {
            return None;
        }
        let data = match data {
            Some(d) if d.chars().count() > MAX_DATA => return None,
            Some(d) => Some(d.to_string()),
            None => None,
        };
        Some(Self { id: id.to_string(), data })
    }
}

/// Home pre-submit picker (`routes/home/session-destination.tsx:13`
/// `HomeSessionDestination = {directory,subdirectory} | {new}`) plus a `phase`
/// pinning the pre-existing post-submit `SessionDestination` (kept, unrenamed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeDestination {
    pub phase: SessionDestination,
    pub directory: String,
    pub subdirectory: bool,
}

impl HomeDestination {
    #[must_use]
    pub fn new_dir(phase: SessionDestination, directory: &str, subdirectory: bool) -> Option<Self> {
        if directory.is_empty() || directory.chars().count() > MAX_DIR {
            return None;
        }
        Some(Self { phase, directory: directory.to_string(), subdirectory })
    }

    #[must_use]
    pub fn new_fresh() -> Self {
        Self { phase: SessionDestination::New, directory: String::new(), subdirectory: false }
    }

    #[must_use]
    pub fn is_fresh(&self) -> bool {
        self.directory.is_empty()
    }
}

/// Draft `input`+`mode` (`prompt/history.tsx:9-12` `PromptInfo{input,mode?,parts}`);
/// `SessionPage.draft` stays the plain `String`, this rides alongside as detail.
/// `mode` whitelisted to `"normal" | "shell"` (`history.tsx:10`), bounded by `MAX_MODE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptDraft {
    pub input: String,
    pub mode: Option<String>,
}

impl PromptDraft {
    pub fn new(input: &str, mode: Option<&str>) -> Result<Self, &'static str> {
        if input.chars().count() > MAX_DRAFT {
            return Err("draft too long");
        }
        let mode = match mode {
            Some("normal") | Some("shell") => Some(mode.unwrap_or_default().to_string()),
            Some(_) => return Err("bad mode"),
            None => None,
        };
        Ok(Self { input: input.to_string(), mode })
    }
}

/// Sidebar tri-state (`routes/session/index.tsx:249-250,264-269`: `kv<"auto"|"hide">`
/// `sidebar` + `sidebarOpen` signal + `wide = width > 120` memo + `sidebarVisible`
/// memo). `SessionPage.sidebar_open` stays `bool`; this is the richer companion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidebarMode {
    #[default]
    Auto,
    Shown,
    Hidden,
}

/// Sidebar width flag (`index.tsx:263` `wide = width > 120`, `:1326` wide/narrow render).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SidebarOpts {
    pub wide: bool,
}

impl SidebarOpts {
    #[must_use]
    pub fn visible(&self, mode: SidebarMode) -> bool {
        match mode {
            SidebarMode::Shown => true,
            SidebarMode::Hidden => false,
            SidebarMode::Auto => self.wide,
        }
    }
}

/// kv-backed view toggles (`routes/session/index.tsx:251-261`: conceal `createSignal(true)`,
/// `showThinking()=true`, `timestamps` kv hide/show default hide,
/// `tool_details_visibility` default true, `scrollbar_visible` default false).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KvToggles {
    pub timestamps: bool,
    pub details: bool,
    pub scrollbar: bool,
    pub conceal: bool,
    pub thinking: bool,
}

impl KvToggles {
    #[must_use]
    pub fn defaults() -> Self {
        Self { timestamps: false, details: true, scrollbar: false, conceal: true, thinking: true }
    }
}

/// Select-callback wiring ids: `DialogTimeline`/`DialogForkFromTimeline` `onMove`
/// and `DialogTimeline`/`DialogMessage` `setPrompt`
/// (`index.tsx:520,527,542,1262`, `dialog-timeline.tsx:10-14`,
/// `dialog-fork-from-timeline.tsx:12`, `dialog-message.tsx:10-14`).
pub const TIMELINE_ON_MOVE: &str = "timeline.onMove";
pub const FORK_ON_MOVE: &str = "fork.onMove";
pub const TIMELINE_SET_PROMPT: &str = "timeline.setPrompt";
pub const MESSAGE_SET_PROMPT: &str = "message.setPrompt";
pub const TIMELINE_ACTIONS: &[&str] = &[TIMELINE_ON_MOVE, TIMELINE_SET_PROMPT];
pub const MESSAGE_ACTIONS: &[&str] = &[MESSAGE_SET_PROMPT];

impl SessionDialog {
    fn text_ok(s: &str) -> bool {
        !s.is_empty() && s.chars().count() <= MAX_TEXT
    }

    fn dir_opt(s: Option<&str>) -> Option<Option<String>> {
        match s {
            Some(d) if d.chars().count() > MAX_DIR => None,
            Some(d) => Some(Some(d.to_string())),
            None => Some(None),
        }
    }

    /// `dialog-session-rename.tsx:7-9` `{session: string}` (`index.tsx:507`).
    #[must_use]
    pub fn rename(session: &str) -> Option<Self> {
        Self::id_ok(session).then(|| Self::Rename { session: session.to_string() })
    }

    /// `ui/dialog-alert.tsx:6-10,59` `{title,message}` (`index.tsx:431,2303` retry-error alert).
    #[must_use]
    pub fn alert(title: &str, message: &str) -> Option<Self> {
        (Self::text_ok(title) && Self::text_ok(message)).then(|| Self::Alert {
            title: title.to_string(),
            message: message.to_string(),
        })
    }

    /// `ui/dialog-confirm.tsx:9-15,93` `{title,message,label?}` (`index.tsx:481,1197` share/redo).
    #[must_use]
    pub fn confirm(title: &str, message: &str, label: Option<&str>) -> Option<Self> {
        if !Self::text_ok(title) || !Self::text_ok(message) {
            return None;
        }
        let label = match label {
            Some(l) if l.chars().count() > MAX_MODE => return None,
            Some(l) => Some(l.to_string()),
            None => None,
        };
        Some(Self::Confirm { title: title.to_string(), message: message.to_string(), label })
    }

    /// `ui/dialog-export-options.tsx:8-22` confirm options
    /// `{filename,thinking,toolDetails,assistantMetadata,openWithoutSaving}` (`index.tsx:958`).
    #[must_use]
    pub fn export(
        filename: &str,
        thinking: bool,
        tool_details: bool,
        assistant_metadata: bool,
        open_without_saving: bool,
    ) -> Option<Self> {
        if filename.is_empty() {
            return None;
        }
        Self::dir_opt(Some(filename)).and_then(|f| {
            f.map(|filename| Self::Export { filename, thinking, tool_details, assistant_metadata, open_without_saving })
        })
    }

    /// `component/dialog-retry-action.tsx:15-21,150` `{title,message,label,link?}`
    /// (`index.tsx:364` `DialogRetryAction.show`, `:97` `RetryAction` type).
    #[must_use]
    pub fn retry_action(title: &str, message: &str, label: &str, link: Option<&str>) -> Option<Self> {
        if !Self::text_ok(title) || !Self::text_ok(message) || !Self::text_ok(label) {
            return None;
        }
        let link = match Self::dir_opt(link) {
            Some(l) => l,
            None => return None,
        };
        Some(Self::RetryAction { title: title.to_string(), message: message.to_string(), label: label.to_string(), link })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destination_variants() {
        assert_eq!(SessionDestination::New, SessionDestination::New);
        assert_eq!(SessionDestination::resume("s1").unwrap(), SessionDestination::Resume("s1".into()));
        assert_eq!(SessionDestination::fork("s1").unwrap(), SessionDestination::Fork("s1".into()));
        assert!(SessionDestination::resume("").is_none());
        assert!(SessionDestination::fork(&"x".repeat(MAX_ID + 1)).is_none());
    }

    #[test]
    fn dialog_constructors_reject_empty() {
        assert!(SessionDialog::fork_from_timeline("").is_none());
        assert!(SessionDialog::message("s", "").is_none());
        assert!(SessionDialog::subagent("s").is_some());
        assert!(SessionDialog::timeline("s").is_some());
        assert!(SessionDialog::permission("", None).is_none());
        assert!(SessionDialog::question("q1", Some("/tmp")).is_some());
    }

    #[test]
    fn dialog_permission_dir_bounded() {
        assert!(SessionDialog::permission("r", Some(&"d".repeat(MAX_ID + 1))).is_none());
        match SessionDialog::permission("r", None).unwrap() {
            SessionDialog::Permission { request_id, directory } => {
                assert_eq!(request_id, "r");
                assert_eq!(directory, None);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn session_page_lifecycle() {
        let mut p = SessionPage::new("ses_1").unwrap();
        assert!(!p.sidebar_open && p.dialog.is_none());
        p.toggle_sidebar();
        assert!(p.sidebar_open);
        p.set_draft("hello").unwrap();
        assert_eq!(p.draft, "hello");
        p.open_dialog(SessionDialog::timeline("ses_1").unwrap());
        assert!(p.dialog.is_some());
        p.close_dialog();
        assert!(p.dialog.is_none());
    }

    #[test]
    fn session_page_bounds_draft_info() {
        assert!(SessionPage::new("").is_err());
        assert!(SessionPage::new(&"x".repeat(MAX_ID + 1)).is_err());
        let mut p = SessionPage::new("s").unwrap();
        assert!(p.set_draft(&"x".repeat(MAX_DRAFT + 1)).is_err());
        assert_eq!(p.draft, "");
        assert!(p.draft_info.is_none());
        p.set_draft("hi").unwrap();
        assert_eq!(p.draft_info.as_ref().unwrap().input, "hi");
        let info = PromptDraft::new("ready", Some("shell")).unwrap();
        p.set_draft_info(info).unwrap();
        assert_eq!(p.draft, "ready");
        assert_eq!(p.draft_info.as_ref().unwrap().mode.as_deref(), Some("shell"));
    }

    #[test]
    fn plugin_route_bounded() {
        let r = PluginRoute::new("git-status", Some("{\"a\":1}")).unwrap();
        assert_eq!(r.id, "git-status");
        assert_eq!(r.data.as_deref(), Some("{\"a\":1}"));
        assert!(PluginRoute::new("", None).is_none());
        assert!(PluginRoute::new(&"x".repeat(MAX_ID + 1), None).is_none());
        assert!(PluginRoute::new("p", Some(&"x".repeat(MAX_DATA + 1))).is_none());
    }

    #[test]
    fn home_destination_picker() {
        let d = HomeDestination::new_dir(SessionDestination::New, "/tmp/wt", true).unwrap();
        assert_eq!(d.directory, "/tmp/wt");
        assert!(d.subdirectory);
        assert!(HomeDestination::new_dir(SessionDestination::New, "", false).is_none());
        assert!(HomeDestination::new_dir(SessionDestination::New, &"d".repeat(MAX_DIR + 1), false).is_none());
        let fresh = HomeDestination::new_fresh();
        assert!(fresh.is_fresh());
    }

    #[test]
    fn prompt_draft_mode_whitelist() {
        assert_eq!(PromptDraft::new("i", None).unwrap().mode, None);
        assert_eq!(PromptDraft::new("i", Some("normal")).unwrap().mode.as_deref(), Some("normal"));
        assert!(PromptDraft::new("i", Some("vim")).is_err());
        assert!(PromptDraft::new(&"x".repeat(MAX_DRAFT + 1), None).is_err());
    }

    #[test]
    fn dialog_new_variants() {
        assert!(SessionDialog::rename("").is_none());
        assert!(matches!(SessionDialog::rename("s1").unwrap(), SessionDialog::Rename { .. }));
        assert!(SessionDialog::alert("", "m").is_none());
        assert!(matches!(SessionDialog::alert("t", "m").unwrap(), SessionDialog::Alert { .. }));
        assert!(SessionDialog::confirm("t", "m", Some(&"l".repeat(MAX_MODE + 1))).is_none());
        match SessionDialog::confirm("t", "m", Some("Share")).unwrap() {
            SessionDialog::Confirm { title, label, .. } => {
                assert_eq!(title, "t");
                assert_eq!(label.as_deref(), Some("Share"));
            }
            _ => panic!("wrong variant"),
        }
        assert!(SessionDialog::export("", true, true, true, false).is_none());
        assert!(SessionDialog::retry_action("t", "m", "Go", Some(&"l".repeat(MAX_DIR + 1))).is_none());
        match SessionDialog::retry_action("t", "m", "Go", None).unwrap() {
            SessionDialog::RetryAction { label, link, .. } => {
                assert_eq!(label, "Go");
                assert_eq!(link, None);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn sidebar_mode_visibility() {
        assert_eq!(SidebarMode::default(), SidebarMode::Auto);
        assert!(SidebarOpts { wide: true }.visible(SidebarMode::Auto));
        assert!(!SidebarOpts { wide: false }.visible(SidebarMode::Auto));
        assert!(SidebarOpts { wide: false }.visible(SidebarMode::Shown));
        assert!(!SidebarOpts { wide: true }.visible(SidebarMode::Hidden));
        let p = SessionPage::new("s").unwrap();
        assert_eq!(p.sidebar_mode, SidebarMode::Auto);
        assert_eq!(p.toggles, KvToggles::defaults());
    }

    #[test]
    fn kv_toggles_defaults() {
        let t = KvToggles::defaults();
        assert!(!t.timestamps && t.details && !t.scrollbar && t.conceal && t.thinking);
    }

    #[test]
    fn action_id_consts() {
        assert!(TIMELINE_ACTIONS.contains(&"timeline.onMove"));
        assert!(TIMELINE_ACTIONS.contains(&"timeline.setPrompt"));
        assert_eq!(MESSAGE_ACTIONS, &["message.setPrompt"]);
        assert_eq!(FORK_ON_MOVE, "fork.onMove");
    }

    #[test]
    fn home_page_draft_bounded() {
        let mut h = HomePage::new(SessionDestination::New);
        assert!(h.set_draft(&"x".repeat(MAX_DRAFT + 1)).is_err());
        h.set_draft("fix tests").unwrap();
        assert_eq!(h.draft, "fix tests");
        h.destination = SessionDestination::fork("s9").unwrap();
        assert_eq!(h.destination, SessionDestination::Fork("s9".into()));
    }
}
