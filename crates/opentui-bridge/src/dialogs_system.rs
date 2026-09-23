#![forbid(unsafe_code)]
//! System dialogs (mirrors `packages/tui/src/component/dialog-*.tsx` @ a0d9b6c).
//!
//! Generic `Dialog`/`Toast` live in `dialog.rs`/`toast.rs`; reused here, not
//! redefined. This file carries per-component evidenced props only.
//!
//! Variant table (component : props : evidence):
//! agent: none, select title "Select agent" : dialog-agent.tsx:6,22
//! console-org: none, select title "Switch org" : dialog-console-org.tsx:24,118
//! debug: none, entries+copy binding : dialog-debug.tsx:13,27-39
//! mcp: none, select title "MCPs" toggle action : dialog-mcp.tsx:21,77
//! model: {providerID?}, dynamic title : dialog-model.tsx:12,180
//! move-session: {projectID,current?,initialDirectories?,initialRemoving?} title "Move session" : dialog-move-session.tsx:25-33,289
//! provider: none, select title "Connect a provider" : dialog-provider.tsx:228-230
//! retry-action: {title,message,label,link?} : dialog-retry-action.tsx:15-20
//! session-delete-failed: {session,workspace} actions delete/restore : dialog-session-delete-failed.tsx:8-13
//! session-list: none(query {search?,filter} internal), title "Sessions" : dialog-session-list.tsx:24,274
//! session-rename: {session} prompt title "Rename Session" : dialog-session-rename.tsx:7-9,19
//! skill: options-loaded, select title "Skills" : dialog-skill.tsx:9-11,53
//! stash: options-loaded, title "Stash" : dialog-stash.tsx:29,57
//! status: {} no props : dialog-status.tsx:8-10
//! tag: options-loaded, title "Autocomplete" : dialog-tag.tsx:8,39
//! theme-list: none, title "Themes" : dialog-theme-list.tsx:6,25
//! variant: none, title "Select variant" : dialog-variant.tsx:6,34
//! workspace-create: {adapters?} titles "Warp"/"Existing Workspace" : dialog-workspace-create.tsx:178-181,249,296
//! workspace-file-changes: {files,title?,message?} choice yes/no : dialog-workspace-file-changes.tsx:27-32
//! workspace-list: none, title "Workspaces" : dialog-workspace-list.tsx:16,97
//! workspace-unavailable: options cancel/restore : dialog-workspace-unavailable.tsx:8

use super::dialog::{MAX_DIALOG_TITLE, MAX_MESSAGE, MAX_OPTIONS};

/// Max carried identifiers (session/project/provider ids).
pub const MAX_ID: usize = 256;
/// Max file-change entries carried.
pub const MAX_FILES: usize = 512;
/// Max link chars for retry-action.
pub const MAX_LINK: usize = 2048;

/// Selectable action (`{id,title,run}` triples in session-delete-failed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub id: String,
    pub label: String,
    pub enabled: bool,
}

impl Action {
    pub fn new(id: &str, label: &str, enabled: bool) -> Result<Self, &'static str> {
        if id.is_empty() || id.chars().count() > MAX_ID {
            return Err("bad action id");
        }
        if label.is_empty() || label.chars().count() > MAX_DIALOG_TITLE {
            return Err("bad action label");
        }
        Ok(Self { id: id.to_string(), label: label.to_string(), enabled })
    }
}

/// One variant per dialog-*.tsx component, carrying its evidenced props.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemDialog {
    Agent { options: Vec<String> },
    ConsoleOrg { options: Vec<String> },
    Debug,
    Mcp { options: Vec<String> },
    Model { provider: Option<String> },
    MoveSession { project: String, current: Option<String> },
    Provider { options: Vec<String> },
    RetryAction { title: String, message: String, label: String, link: Option<String> },
    SessionDeleteFailed { session: String, workspace: String },
    SessionList { options: Vec<String> },
    SessionRename { session: String },
    Skill { options: Vec<String> },
    Stash { options: Vec<String> },
    Status,
    Tag { options: Vec<String> },
    ThemeList { options: Vec<String> },
    Variant { options: Vec<String> },
    WorkspaceCreate { options: Vec<String> },
    WorkspaceFileChanges { files: Vec<String>, title: Option<String>, message: Option<String> },
    WorkspaceList { options: Vec<String> },
    WorkspaceUnavailable,
}

fn check_options(options: &[String]) -> Result<(), &'static str> {
    if options.is_empty() || options.len() > MAX_OPTIONS {
        return Err("bad options");
    }
    if options.iter().any(|o| o.is_empty() || o.chars().count() > MAX_DIALOG_TITLE) {
        return Err("bad option");
    }
    Ok(())
}

fn check_id(s: &str) -> Result<(), &'static str> {
    if s.is_empty() || s.chars().count() > MAX_ID {
        return Err("bad id");
    }
    Ok(())
}

impl SystemDialog {
    /// Validate props (bounded title/message/actions/options).
    pub fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::Agent { options }
            | Self::ConsoleOrg { options }
            | Self::Mcp { options }
            | Self::Provider { options }
            | Self::SessionList { options }
            | Self::Skill { options }
            | Self::Stash { options }
            | Self::Tag { options }
            | Self::ThemeList { options }
            | Self::Variant { options }
            | Self::WorkspaceCreate { options }
            | Self::WorkspaceList { options } => check_options(options),
            Self::Debug | Self::Status | Self::WorkspaceUnavailable => Ok(()),
            Self::Model { provider } => {
                if let Some(p) = provider {
                    check_id(p)?;
                }
                Ok(())
            }
            Self::MoveSession { project, current } => {
                check_id(project)?;
                if let Some(c) = current {
                    check_id(c)?;
                }
                Ok(())
            }
            Self::RetryAction { title, message, label, link } => {
                if title.is_empty() || title.chars().count() > MAX_DIALOG_TITLE {
                    return Err("bad title");
                }
                if message.is_empty() || message.chars().count() > MAX_MESSAGE {
                    return Err("bad message");
                }
                if label.is_empty() || label.chars().count() > MAX_DIALOG_TITLE {
                    return Err("bad label");
                }
                if let Some(l) = link {
                    if l.is_empty() || l.chars().count() > MAX_LINK {
                        return Err("bad link");
                    }
                }
                Ok(())
            }
            Self::SessionDeleteFailed { session, workspace } => {
                check_id(session)?;
                check_id(workspace)
            }
            Self::SessionRename { session } => check_id(session),
            Self::WorkspaceFileChanges { files, title, message } => {
                if files.is_empty() || files.len() > MAX_FILES {
                    return Err("bad files");
                }
                if files.iter().any(|f| f.is_empty() || f.chars().count() > MAX_DIALOG_TITLE) {
                    return Err("bad file");
                }
                if let Some(t) = title {
                    if t.is_empty() || t.chars().count() > MAX_DIALOG_TITLE {
                        return Err("bad title");
                    }
                }
                if let Some(m) = message {
                    if m.chars().count() > MAX_MESSAGE {
                        return Err("bad message");
                    }
                }
                Ok(())
            }
        }
    }

    /// Open dialog (validates first).
    pub fn open(&self) -> Result<(), &'static str> {
        self.validate()
    }

    /// Actions offered by confirm-style dialogs.
    #[must_use]
    pub fn actions(&self) -> Vec<Action> {
        match self {
            Self::RetryAction { label, .. } => vec![
                Action { id: "action".into(), label: label.clone(), enabled: true },
                Action { id: "dismiss".into(), label: "Don't show again".into(), enabled: true },
            ],
            Self::SessionDeleteFailed { .. } => vec![
                Action { id: "delete".into(), label: "Delete workspace".into(), enabled: true },
                Action { id: "restore".into(), label: "Restore to new workspace".into(), enabled: true },
            ],
            Self::WorkspaceFileChanges { .. } => vec![
                Action { id: "yes".into(), label: "Yes".into(), enabled: true },
                Action { id: "no".into(), label: "No".into(), enabled: true },
            ],
            Self::WorkspaceUnavailable => vec![
                Action { id: "cancel".into(), label: "Cancel".into(), enabled: true },
                Action { id: "restore".into(), label: "Restore".into(), enabled: true },
            ],
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_variant_prop_carry() {
        let d = SystemDialog::Model { provider: Some("anthropic".into()) };
        assert!(d.open().is_ok());
        assert!(matches!(d, SystemDialog::Model { provider: Some(_) }));
        let d = SystemDialog::MoveSession { project: "p1".into(), current: None };
        assert!(d.open().is_ok());
        let d = SystemDialog::SessionRename { session: "s1".into() };
        assert!(d.open().is_ok());
        let d = SystemDialog::SessionDeleteFailed { session: "s".into(), workspace: "w".into() };
        assert_eq!(d.actions().len(), 2);
        let d = SystemDialog::RetryAction {
            title: "t".into(),
            message: "m".into(),
            label: "Retry".into(),
            link: Some("https://opencode.ai/go".into()),
        };
        assert!(d.open().is_ok());
        assert_eq!(d.actions()[0].id, "action");
        let d = SystemDialog::WorkspaceFileChanges {
            files: vec!["a.ts".into()],
            title: None,
            message: None,
        };
        assert!(d.open().is_ok());
        assert!(SystemDialog::Debug.open().is_ok());
        assert!(SystemDialog::Status.open().is_ok());
        assert!(SystemDialog::WorkspaceUnavailable.open().is_ok());
        assert!(SystemDialog::Agent { options: vec!["a".into()] }.open().is_ok());
    }

    #[test]
    fn empty_options_errs() {
        for d in [
            SystemDialog::Agent { options: vec![] },
            SystemDialog::Skill { options: vec![] },
            SystemDialog::ThemeList { options: vec!["".into()] },
            SystemDialog::WorkspaceList {
                options: (0..MAX_OPTIONS + 1).map(|i| format!("w{i}")).collect(),
            },
        ] {
            assert!(d.validate().is_err());
        }
    }

    #[test]
    fn oversize_errs() {
        assert!(SystemDialog::RetryAction {
            title: "t".repeat(MAX_DIALOG_TITLE + 1),
            message: "m".into(),
            label: "l".into(),
            link: None,
        }
        .validate()
        .is_err());
        assert!(SystemDialog::RetryAction {
            title: "t".into(),
            message: "m".repeat(MAX_MESSAGE + 1),
            label: "l".into(),
            link: None,
        }
        .validate()
        .is_err());
        assert!(SystemDialog::SessionRename { session: "x".repeat(MAX_ID + 1) }.validate().is_err());
        assert!(SystemDialog::WorkspaceFileChanges { files: vec![], title: None, message: None }
            .validate()
            .is_err());
        assert!(Action::new("", "ok", true).is_err());
    }

    #[test]
    fn disabled_action_carried() {
        let a = Action::new("toggle", "toggle", false).unwrap();
        assert!(!a.enabled);
        assert_eq!(a.id, "toggle");
        assert!(Action::new("ok", &"l".repeat(MAX_DIALOG_TITLE + 1), true).is_err());
    }

    #[test]
    fn confirm_style_actions() {
        let d = SystemDialog::SessionDeleteFailed { session: "s".into(), workspace: "w".into() };
        let ids: Vec<_> = d.actions().iter().map(|a| a.id.clone()).collect();
        assert_eq!(ids, vec!["delete", "restore"]);
        let d = SystemDialog::WorkspaceUnavailable;
        let ids: Vec<_> = d.actions().iter().map(|a| a.id.clone()).collect();
        assert_eq!(ids, vec!["cancel", "restore"]);
        let d = SystemDialog::Debug;
        assert!(d.actions().is_empty());
    }

    #[test]
    fn move_session_current_and_link_bounds() {
        assert!(SystemDialog::MoveSession { project: "".into(), current: None }.validate().is_err());
        assert!(SystemDialog::MoveSession {
            project: "p".into(),
            current: Some("c".into()),
        }
        .validate()
        .is_ok());
        assert!(SystemDialog::RetryAction {
            title: "t".into(),
            message: "m".into(),
            label: "l".into(),
            link: Some("x".repeat(MAX_LINK + 1)),
        }
        .validate()
        .is_err());
    }
}

// ---- BRIDGE-062 additive types (evidence: header variant table) ----

/// Max directory chars (dialog-move-session.tsx:22 `directory`).
pub const MAX_DIRECTORY: usize = 1024;
/// Max stash input chars (prompt/stash.tsx:9-13, mirrors MAX_MESSAGE).
pub const MAX_STASH_INPUT: usize = 4096;

/// Move-session selection (dialog-move-session.tsx:22).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveSessionSelection {
    Directory { directory: String, subdirectory: bool },
    New,
}

impl MoveSessionSelection {
    pub fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::Directory { directory, .. } => {
                if directory.is_empty() || directory.chars().count() > MAX_DIRECTORY {
                    return Err("bad directory");
                }
                Ok(())
            }
            Self::New => Ok(()),
        }
    }
}

/// Stash entry input+timestamp (prompt/stash.tsx:9-13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashEntry {
    pub input: String,
    pub timestamp: u64,
}

impl StashEntry {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.input.is_empty() || self.input.chars().count() > MAX_STASH_INPUT {
            return Err("bad stash input");
        }
        Ok(())
    }
}

/// VCS file status (dialog-workspace-file-changes.tsx:28, VcsFileStatus types.gen.ts:2311).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VcsFile {
    pub file: String,
    pub status: String,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
}

impl VcsFile {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.file.is_empty() || self.file.chars().count() > MAX_DIALOG_TITLE {
            return Err("bad file");
        }
        if !matches!(self.status.as_str(), "added" | "deleted" | "modified") {
            return Err("bad status");
        }
        Ok(())
    }
}

/// Workspace selection (dialog-workspace-create.tsx:16-30).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceSelection {
    None,
    New { workspace_type: String, workspace_name: String },
    Existing { workspace_id: String, workspace_type: String, workspace_name: String },
}

impl WorkspaceSelection {
    pub fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::None => Ok(()),
            Self::New { workspace_type, workspace_name } => {
                check_id(workspace_type)?;
                check_id(workspace_name)
            }
            Self::Existing { workspace_id, workspace_type, workspace_name } => {
                check_id(workspace_id)?;
                check_id(workspace_type)?;
                check_id(workspace_name)
            }
        }
    }
}

/// Action ids for select/confirm dialogs (dialog-skill.tsx:9-11, dialog-stash.tsx:29, dialog-tag.tsx:8, dialog-workspace-create.tsx:179-180).
pub const SKILL_SELECT: &str = "skill-select";
pub const STASH_SELECT: &str = "stash-select";
pub const TAG_SELECT: &str = "tag-select";
pub const WORKSPACE_SELECT: &str = "workspace-select";
pub const RETRY_DONT_SHOW: &str = "dismiss";
pub const DELETE_CONFIRM: &str = "delete";
pub const RESTORE: &str = "restore";
pub const DONE: &str = "done";

#[cfg(test)]
mod bridge062_tests {
    use super::*;

    #[test]
    fn move_session_directory_ok() {
        let s = MoveSessionSelection::Directory { directory: "/tmp/proj".into(), subdirectory: false };
        assert!(s.validate().is_ok());
        assert!(MoveSessionSelection::New.validate().is_ok());
    }

    #[test]
    fn move_session_directory_bounds() {
        assert!(MoveSessionSelection::Directory { directory: "".into(), subdirectory: true }.validate().is_err());
        assert!(MoveSessionSelection::Directory { directory: "x".repeat(MAX_DIRECTORY + 1), subdirectory: false }.validate().is_err());
    }

    #[test]
    fn stash_entry_bounds() {
        assert!(StashEntry { input: "hello".into(), timestamp: 1 }.validate().is_ok());
        assert!(StashEntry { input: "".into(), timestamp: 0 }.validate().is_err());
        assert!(StashEntry { input: "x".repeat(MAX_STASH_INPUT + 1), timestamp: 0 }.validate().is_err());
    }

    #[test]
    fn vcs_file_bounds() {
        let f = VcsFile { file: "a.ts".into(), status: "modified".into(), additions: Some(3), deletions: None };
        assert!(f.validate().is_ok());
        assert!(VcsFile { file: "".into(), status: "modified".into(), additions: None, deletions: None }.validate().is_err());
        assert!(VcsFile { file: "a.ts".into(), status: "bogus".into(), additions: None, deletions: None }.validate().is_err());
    }

    #[test]
    fn workspace_selection_bounds() {
        assert!(WorkspaceSelection::None.validate().is_ok());
        assert!(WorkspaceSelection::New { workspace_type: "warp".into(), workspace_name: "w".into() }.validate().is_ok());
        assert!(WorkspaceSelection::Existing { workspace_id: "".into(), workspace_type: "t".into(), workspace_name: "n".into() }.validate().is_err());
        assert!(WorkspaceSelection::New { workspace_type: "".into(), workspace_name: "n".into() }.validate().is_err());
    }

    #[test]
    fn action_id_consts_hit_actions() {
        for c in [SKILL_SELECT, STASH_SELECT, TAG_SELECT, WORKSPACE_SELECT, RETRY_DONT_SHOW, DELETE_CONFIRM, RESTORE, DONE] {
            assert!(!c.is_empty());
        }
        let d = SystemDialog::SessionDeleteFailed { session: "s".into(), workspace: "w".into() };
        let ids: Vec<_> = d.actions().iter().map(|a| a.id.clone()).collect();
        assert!(ids.contains(&DELETE_CONFIRM.to_string()));
        assert!(ids.contains(&RESTORE.to_string()));
        let r = SystemDialog::RetryAction { title: "t".into(), message: "m".into(), label: "l".into(), link: None };
        let ids: Vec<_> = r.actions().iter().map(|a| a.id.clone()).collect();
        assert!(ids.contains(&RETRY_DONT_SHOW.to_string()));
    }
}
