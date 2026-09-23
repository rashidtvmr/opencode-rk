#![forbid(unsafe_code)]
//! TUI UI-state contracts (theme/clipboard/permission/prompt/epilogue/exit/thinking/event).
//!
//! TS checkout /home/rashid/projects/opencode @ a0d9b6c (NOT pinned 95daf90).
//! Evidence:
//! - theme.tsx:84-98 (mode dark|light, lock dark|light|undef, active default
//!   "opencode"), :289 (locked = lock.is_some), :290-291 (lock/unlock pin/free)
//! - clipboard.tsx:4-8 (ClipboardContent data+mime, ClipboardService opt read/write)
//! - permission.tsx:5 (PermissionMode auto|normal only); reply values
//!   once|always|reject @ sdk/js/src/v2/gen/types.gen.ts:1400 and
//!   ask|allow|deny @ types.gen.ts:1657; routes/session/permission.tsx:168-181,426-427
//! - prompt: no default text; placeholder `Ask anything...` @
//!   component/prompt/index.tsx:1316; prompt.tsx:4-18 (ref holder only)
//! - epilogue.tsx:3-6 (set(value?: string)); presentation.ts:29-37
//!   (sessionEpilogue); app.tsx:361 (stdout write epilogue+"\n")
//! - exit.tsx:3 (Exit=(reason?:unknown)=>void); codes only via CliError numeric
//!   passthrough @ util/error.ts:11-12 and process.exit(1) @ cli/cmd/agent.ts:134,216
//! - thinking.ts:4 (show|hide), :24-27 (show->hide->show cycle), :36 (default hide)
//! - event.ts:12-20 (subscribe filters "sync", delegates to sdk.event);
//!   consumed names @ sync.tsx:170-439, app.tsx:985-1031, routes/session/index.tsx:320,350,
//!   notifications.ts:35-80, dialog-session-list.tsx:96
//!
//! Divergence: exit lib never calls process::exit; returns code for caller
//! (same rule as cli_error.rs). Permission expiry omitted: no evidence.
//! UiEvent covers TUI-subscribed names; unknown strings map to Unknown (fail-closed).
//! NOT wired in lib.rs (scope forbids touching it).

/// Theme mode. Locked = pinned via kv theme_mode_lock (theme.tsx:209-213).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
    Locked,
}

impl ThemeMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Locked => "locked",
        }
    }
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            "locked" => Some(Self::Locked),
            _ => None,
        }
    }
}

/// Active theme name; default "opencode" (theme.tsx:96).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeName(String);

impl ThemeName {
    pub fn new(name: &str) -> Option<Self> {
        let n = name.trim();
        if n.is_empty() || n.len() > 64 {
            return None;
        }
        Some(Self(n.to_string()))
    }
    #[must_use]
    pub fn get(&self) -> &str {
        &self.0
    }
    #[must_use]
    pub fn default_name() -> &'static str {
        "opencode"
    }
}

/// Clipboard capability flags (clipboard.tsx:4-8 optional read/write).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipboardIface {
    pub can_read: bool,
    pub can_write: bool,
}

/// Permission decision. Evidenced via ask|allow|deny (types.gen.ts:1657) and
/// once|always|reject replies (types.gen.ts:1400). No expiry in source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    Ask,
    Allow,
    Deny,
}

impl PermissionState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Allow => "allow",
            Self::Deny => "deny",
        }
    }
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ask" => Some(Self::Ask),
            "allow" | "once" | "always" => Some(Self::Allow),
            "deny" | "reject" => Some(Self::Deny),
            _ => None,
        }
    }
}

/// TUI permission gate mode (permission.tsx:5,11-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionGate {
    Auto,
    Normal,
}

/// Default prompt placeholder stem (prompt/index.tsx:1316).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptSeed(String);

impl PromptSeed {
    #[must_use]
    pub fn placeholder(example: &str) -> String {
        format!("Ask anything... \"{example}\"")
    }
    pub fn new(text: &str) -> Option<Self> {
        let t = text.trim();
        if t.is_empty() || t.len() > 4096 {
            return None;
        }
        Some(Self(t.to_string()))
    }
    #[must_use]
    pub fn get(&self) -> &str {
        &self.0
    }
}

/// Epilogue line(s) printed on exit (app.tsx:361). Bounded 8 KiB.
pub const MAX_EPILOGUE_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Epilogue {
    pub message: Option<String>,
}

impl Epilogue {
    pub fn new(message: Option<&str>) -> Option<Self> {
        match message {
            None => Some(Self { message: None }),
            Some(m) if m.len() > MAX_EPILOGUE_BYTES => None,
            Some(m) => Some(Self {
                message: Some(m.to_string()),
            }),
        }
    }
}

/// Exit code. Evidenced: 0 ok; 1 via agent.ts:134,216 / github.handler.ts:392;
/// arbitrary numeric passthrough via error.ts:12.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    Success,
    Failure,
    Other(i32),
}

impl ExitCode {
    #[must_use]
    pub const fn code(self) -> i32 {
        match self {
            Self::Success => 0,
            Self::Failure => 1,
            Self::Other(n) => n,
        }
    }
    #[must_use]
    pub const fn from_code(n: i32) -> Self {
        match n {
            0 => Self::Success,
            1 => Self::Failure,
            n => Self::Other(n),
        }
    }
}

/// Thinking toggle. show=enabled (thinking.ts:4,24-27,36 default hide).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thinking {
    pub enabled: bool,
}

impl Thinking {
    #[must_use]
    pub const fn from_mode(mode: &str) -> Self {
        match mode.as_bytes() {
            b"show" => Self { enabled: true },
            _ => Self { enabled: false },
        }
    }
    #[must_use]
    pub const fn mode_str(self) -> &'static str {
        if self.enabled {
            "show"
        } else {
            "hide"
        }
    }
    #[must_use]
    pub const fn toggle(self) -> Self {
        Self {
            enabled: !self.enabled,
        }
    }
}

/// TUI-subscribed UI events (event.ts delegates; names evidenced in sync.tsx,
/// app.tsx, routes/session/index.tsx, notifications.ts). "sync" never surfaces
/// (event.ts:14-16) so it is absent here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiEvent {
    SessionCreated,
    SessionUpdated,
    SessionDeleted,
    SessionDiff,
    SessionStatus,
    SessionError,
    SessionCompacted,
    MessageUpdated,
    MessageRemoved,
    MessagePartUpdated,
    MessagePartRemoved,
    MessagePartDelta,
    PermissionAsked,
    PermissionReplied,
    QuestionAsked,
    QuestionReplied,
    QuestionRejected,
    TodoUpdated,
    LspUpdated,
    VcsBranchUpdated,
    TuiPromptAppend,
    TuiCommandExecute,
    TuiToastShow,
    TuiSessionSelect,
    InstallationUpdateAvailable,
    Unknown,
}

impl UiEvent {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SessionCreated => "session.created",
            Self::SessionUpdated => "session.updated",
            Self::SessionDeleted => "session.deleted",
            Self::SessionDiff => "session.diff",
            Self::SessionStatus => "session.status",
            Self::SessionError => "session.error",
            Self::SessionCompacted => "session.compacted",
            Self::MessageUpdated => "message.updated",
            Self::MessageRemoved => "message.removed",
            Self::MessagePartUpdated => "message.part.updated",
            Self::MessagePartRemoved => "message.part.removed",
            Self::MessagePartDelta => "message.part.delta",
            Self::PermissionAsked => "permission.asked",
            Self::PermissionReplied => "permission.replied",
            Self::QuestionAsked => "question.asked",
            Self::QuestionReplied => "question.replied",
            Self::QuestionRejected => "question.rejected",
            Self::TodoUpdated => "todo.updated",
            Self::LspUpdated => "lsp.updated",
            Self::VcsBranchUpdated => "vcs.branch.updated",
            Self::TuiPromptAppend => "tui.prompt.append",
            Self::TuiCommandExecute => "tui.command.execute",
            Self::TuiToastShow => "tui.toast.show",
            Self::TuiSessionSelect => "tui.session.select",
            Self::InstallationUpdateAvailable => "installation.update-available",
            Self::Unknown => "unknown",
        }
    }
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "session.created" => Self::SessionCreated,
            "session.updated" => Self::SessionUpdated,
            "session.deleted" => Self::SessionDeleted,
            "session.diff" => Self::SessionDiff,
            "session.status" => Self::SessionStatus,
            "session.error" => Self::SessionError,
            "session.compacted" => Self::SessionCompacted,
            "message.updated" => Self::MessageUpdated,
            "message.removed" => Self::MessageRemoved,
            "message.part.updated" => Self::MessagePartUpdated,
            "message.part.removed" => Self::MessagePartRemoved,
            "message.part.delta" => Self::MessagePartDelta,
            "permission.asked" => Self::PermissionAsked,
            "permission.replied" => Self::PermissionReplied,
            "question.asked" => Self::QuestionAsked,
            "question.replied" => Self::QuestionReplied,
            "question.rejected" => Self::QuestionRejected,
            "todo.updated" => Self::TodoUpdated,
            "lsp.updated" => Self::LspUpdated,
            "vcs.branch.updated" => Self::VcsBranchUpdated,
            "tui.prompt.append" => Self::TuiPromptAppend,
            "tui.command.execute" => Self::TuiCommandExecute,
            "tui.toast.show" => Self::TuiToastShow,
            "tui.session.select" => Self::TuiSessionSelect,
            "installation.update-available" => Self::InstallationUpdateAvailable,
            _ => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_mode_roundtrip() {
        assert_eq!(ThemeMode::parse("dark"), Some(ThemeMode::Dark));
        assert_eq!(ThemeMode::Dark.as_str(), "dark");
        assert_eq!(ThemeMode::parse("locked"), Some(ThemeMode::Locked));
        assert_eq!(ThemeMode::parse("nope"), None);
    }

    #[test]
    fn permission_state_maps_replies() {
        assert_eq!(PermissionState::parse("once"), Some(PermissionState::Allow));
        assert_eq!(PermissionState::parse("reject"), Some(PermissionState::Deny));
        assert_eq!(PermissionState::parse("ask"), Some(PermissionState::Ask));
        assert_eq!(PermissionState::Ask.as_str(), "ask");
    }

    #[test]
    fn exit_code_roundtrip() {
        assert_eq!(ExitCode::from_code(0), ExitCode::Success);
        assert_eq!(ExitCode::from_code(1), ExitCode::Failure);
        assert_eq!(ExitCode::Other(3).code(), 3);
    }

    #[test]
    fn thinking_toggle_cycles() {
        let t = Thinking::from_mode("hide");
        assert!(!t.enabled && t.mode_str() == "hide");
        assert!(Thinking::from_mode("show").toggle().mode_str() == "hide");
    }

    #[test]
    fn ui_event_roundtrip_and_unknown() {
        assert_eq!(UiEvent::parse("session.deleted"), UiEvent::SessionDeleted);
        assert_eq!(UiEvent::SessionDeleted.as_str(), "session.deleted");
        assert_eq!(UiEvent::parse("sync"), UiEvent::Unknown);
        assert_eq!(UiEvent::parse("tui.toast.show"), UiEvent::TuiToastShow);
    }

    #[test]
    fn epilogue_bounded() {
        assert!(Epilogue::new(None).unwrap().message.is_none());
        assert!(Epilogue::new(Some(&"x".repeat(MAX_EPILOGUE_BYTES + 1))).is_none());
        assert_eq!(ThemeName::new(""), None);
        assert_eq!(ThemeName::new("opencode").unwrap().get(), "opencode");
        assert_eq!(PromptSeed::placeholder("hi"), "Ask anything... \"hi\"");
    }
}
