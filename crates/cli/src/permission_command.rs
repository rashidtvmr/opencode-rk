#![forbid(unsafe_code)]
//! `/permission` slash command: manual per-tool allow/deny (user-visible).
//!
//! Deny-unless-explicit:
//! - `allow <tool>` enables a tool, `disallow <tool>` blocks it.
//! - Deny always wins over allow, so a later allow never lifts an explicit
//!   disallow.
//! - Wildcards and reserved namespaces (`human.`, `system.`, `authority.`,
//!   `grant.`) are rejected: this command is a reviewer convenience, not a
//!   way around mandatory policy.
//! - The set is bounded and persisted next to the daemon state as
//!   `permissions.json` (temp file + rename, never a partial write).
//!
//! Pure std: parsing and state only; the caller renders the returned text
//! and passes [`PermissionStore::enabled`] to the real turn policy.

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

/// Upper bound on each of the allow and deny sets.
pub const MAX_TOOLS: usize = 64;
/// Longest accepted tool name in bytes.
pub const MAX_TOOL_CHARS: usize = 64;
/// File name under the caller's data directory.
pub const STORE_FILE: &str = "permissions.json";

/// Reserved prefixes that this command can never grant.
const RESERVED: [&str; 4] = ["human.", "system.", "authority.", "grant."];

/// One parsed `/permission` invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PermissionAction {
    /// Show the current sets.
    List,
    /// Enable one tool.
    Allow(String),
    /// Block one tool (wins over allow).
    Disallow(String),
    /// Clear both sets.
    Reset,
    /// Usage text.
    Help,
}

/// Fail-closed command errors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PermissionCommandError {
    EmptyTool,
    Wildcard(String),
    Reserved(String),
    InvalidName(String),
    UnknownSubcommand(String),
    TooManyTools,
    Io(String),
}

impl fmt::Display for PermissionCommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTool => write!(f, "tool name is empty"),
            Self::Wildcard(name) => write!(f, "wildcards never grant: {name:?}"),
            Self::Reserved(name) => write!(f, "reserved namespace is never grantable: {name:?}"),
            Self::InvalidName(name) => write!(f, "invalid tool name: {name:?}"),
            Self::UnknownSubcommand(name) => write!(
                f,
                "unknown /permission subcommand {name:?}; use list, allow, disallow, or reset"
            ),
            Self::TooManyTools => write!(f, "too many tools (limit {MAX_TOOLS})"),
            Self::Io(error) => write!(f, "permission store error: {error}"),
        }
    }
}

impl std::error::Error for PermissionCommandError {}

/// Parse one `/permission ...` line. The bare command lists.
pub fn parse(input: &str) -> Result<PermissionAction, PermissionCommandError> {
    let rest = input.trim();
    let rest = rest
        .strip_prefix("/permission")
        .or_else(|| rest.strip_prefix("permission"))
        .ok_or_else(|| PermissionCommandError::UnknownSubcommand(rest.to_owned()))?
        .trim();
    if rest.is_empty() || rest == "list" {
        return Ok(PermissionAction::List);
    }
    if rest == "help" {
        return Ok(PermissionAction::Help);
    }
    if rest == "reset" {
        return Ok(PermissionAction::Reset);
    }
    let (verb, name) = rest
        .split_once(char::is_whitespace)
        .unwrap_or((rest, ""));
    let name = name.trim();
    match verb {
        "allow" => {
            validate_tool(name)?;
            Ok(PermissionAction::Allow(name.to_owned()))
        }
        "disallow" | "deny" => {
            validate_tool(name)?;
            Ok(PermissionAction::Disallow(name.to_owned()))
        }
        _ => Err(PermissionCommandError::UnknownSubcommand(verb.to_owned())),
    }
}

/// Reject names this command can never grant (and names the broker would
/// treat as a pattern or a namespace).
pub fn validate_tool(name: &str) -> Result<(), PermissionCommandError> {
    if name.is_empty() {
        return Err(PermissionCommandError::EmptyTool);
    }
    if name.contains('*') {
        return Err(PermissionCommandError::Wildcard(name.to_owned()));
    }
    if RESERVED
        .iter()
        .any(|prefix| name.starts_with(prefix) || name == prefix.trim_end_matches('.'))
    {
        return Err(PermissionCommandError::Reserved(name.to_owned()));
    }
    if name.chars().count() > MAX_TOOL_CHARS
        || name.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(PermissionCommandError::InvalidName(name.to_owned()));
    }
    Ok(())
}

/// Bounded allow/deny sets. Deny wins.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PermissionStore {
    allowed: BTreeSet<String>,
    denied: BTreeSet<String>,
}

impl PermissionStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// True only when explicitly allowed and not explicitly denied.
    #[must_use]
    pub fn is_enabled(&self, tool: &str) -> bool {
        self.allowed.contains(tool) && !self.denied.contains(tool)
    }

    /// Tools the turn policy may enable (allowed minus denied), stable order.
    #[must_use]
    pub fn enabled(&self) -> Vec<String> {
        self.allowed
            .difference(&self.denied)
            .cloned()
            .collect::<Vec<_>>()
    }

    /// Apply one parsed action and return the lines to render.
    pub fn apply(
        &mut self,
        action: &PermissionAction,
    ) -> Result<Vec<String>, PermissionCommandError> {
        match action {
            PermissionAction::List => {}
            PermissionAction::Allow(tool) => {
                if self.allowed.len() >= MAX_TOOLS && !self.allowed.contains(tool) {
                    return Err(PermissionCommandError::TooManyTools);
                }
                self.allowed.insert(tool.clone());
            }
            PermissionAction::Disallow(tool) => {
                if self.denied.len() >= MAX_TOOLS && !self.denied.contains(tool) {
                    return Err(PermissionCommandError::TooManyTools);
                }
                self.denied.insert(tool.clone());
            }
            PermissionAction::Reset => {
                self.allowed.clear();
                self.denied.clear();
            }
            PermissionAction::Help => return Ok(vec![usage().to_owned()]),
        }
        Ok(vec![self.render()])
    }

    /// Human-readable current state.
    #[must_use]
    pub fn render(&self) -> String {
        let allowed = join_or_none(&self.allowed);
        let denied = join_or_none(&self.denied);
        format!("allowed: {allowed} | denied: {denied}")
    }

    /// Load `<data_dir>/permissions.json`; a missing or malformed file is an
    /// empty store (fail closed: nothing enabled).
    #[must_use]
    pub fn load(data_dir: &Path) -> Self {
        let Ok(bytes) = std::fs::read_to_string(data_dir.join(STORE_FILE)) else {
            return Self::new();
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&bytes) else {
            return Self::new();
        };
        let mut store = Self::new();
        for (key, set) in [("allowed", &mut store.allowed), ("denied", &mut store.denied)] {
            if let Some(items) = value.get(key).and_then(serde_json::Value::as_array) {
                for item in items.iter().filter_map(serde_json::Value::as_str) {
                    if validate_tool(item).is_ok() && set.len() < MAX_TOOLS {
                        set.insert(item.to_owned());
                    }
                }
            }
        }
        store
    }

    /// Persist atomically: write a sibling temp file, then rename. Returns an
    /// error without touching the previous file when the write fails.
    pub fn save(&self, data_dir: &Path) -> Result<(), PermissionCommandError> {
        std::fs::create_dir_all(data_dir).map_err(io_error)?;
        let payload = serde_json::json!({
            "allowed": self.allowed.iter().collect::<Vec<_>>(),
            "denied": self.denied.iter().collect::<Vec<_>>(),
        })
        .to_string();
        let temp = data_dir.join(format!("{STORE_FILE}.tmp"));
        std::fs::write(&temp, payload).map_err(io_error)?;
        std::fs::rename(&temp, data_dir.join(STORE_FILE)).map_err(io_error)
    }
}

fn io_error(error: std::io::Error) -> PermissionCommandError {
    PermissionCommandError::Io(error.to_string())
}

fn join_or_none(set: &BTreeSet<String>) -> String {
    if set.is_empty() {
        "(none)".to_owned()
    } else {
        set.iter().cloned().collect::<Vec<_>>().join(", ")
    }
}

/// Usage text shown by `/permission help` and by the command palette.
#[must_use]
pub fn usage() -> &'static str {
    "/permission                 show allowed/denied tools\n\
     /permission allow <tool>     enable a tool\n\
     /permission disallow <tool>  block a tool (deny wins)\n\
     /permission reset            clear both lists"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oc2-permission-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parses_list_allow_disallow_reset_help() {
        assert_eq!(parse("/permission"), Ok(PermissionAction::List));
        assert_eq!(parse("/permission list"), Ok(PermissionAction::List));
        assert_eq!(
            parse("/permission allow bash"),
            Ok(PermissionAction::Allow("bash".into()))
        );
        assert_eq!(
            parse("/permission disallow write"),
            Ok(PermissionAction::Disallow("write".into()))
        );
        assert_eq!(
            parse("/permission deny write"),
            Ok(PermissionAction::Disallow("write".into()))
        );
        assert_eq!(parse("/permission reset"), Ok(PermissionAction::Reset));
        assert_eq!(parse("/permission help"), Ok(PermissionAction::Help));
    }

    #[test]
    fn rejects_wildcards_reserved_empty_and_bad_names() {
        assert_eq!(
            parse("/permission allow *"),
            Err(PermissionCommandError::Wildcard("*".into()))
        );
        assert_eq!(
            parse("/permission allow bash*"),
            Err(PermissionCommandError::Wildcard("bash*".into()))
        );
        assert_eq!(
            parse("/permission allow human.approve"),
            Err(PermissionCommandError::Reserved("human.approve".into()))
        );
        assert_eq!(
            parse("/permission disallow system.shutdown"),
            Err(PermissionCommandError::Reserved("system.shutdown".into()))
        );
        assert_eq!(
            parse("/permission allow"),
            Err(PermissionCommandError::EmptyTool)
        );
        assert_eq!(
            parse("/permission allow two words"),
            Err(PermissionCommandError::InvalidName("two words".into()))
        );
        assert!(matches!(
            parse("/permission explode bash"),
            Err(PermissionCommandError::UnknownSubcommand(_))
        ));
    }

    #[test]
    fn deny_wins_over_allow_and_enabled_is_sorted() {
        let mut store = PermissionStore::new();
        store
            .apply(&PermissionAction::Allow("write".into()))
            .unwrap();
        store
            .apply(&PermissionAction::Allow("bash".into()))
            .unwrap();
        assert!(store.is_enabled("bash"));
        assert_eq!(store.enabled(), vec!["bash".to_owned(), "write".to_owned()]);
        store
            .apply(&PermissionAction::Disallow("bash".into()))
            .unwrap();
        assert!(!store.is_enabled("bash"), "deny wins");
        assert_eq!(store.enabled(), vec!["write".to_owned()]);
        // A later allow never lifts an explicit disallow.
        store
            .apply(&PermissionAction::Allow("bash".into()))
            .unwrap();
        assert!(!store.is_enabled("bash"));
    }

    #[test]
    fn reset_clears_both_sets() {
        let mut store = PermissionStore::new();
        store
            .apply(&PermissionAction::Allow("bash".into()))
            .unwrap();
        store
            .apply(&PermissionAction::Disallow("write".into()))
            .unwrap();
        let text = store
            .apply(&PermissionAction::Reset)
            .unwrap()
            .join("\n");
        assert!(text.contains("(none)"));
        assert!(store.enabled().is_empty());
        assert!(!store.is_enabled("write"));
    }

    #[test]
    fn persistence_round_trips_and_never_partial() {
        let dir = temp_dir("roundtrip");
        let mut store = PermissionStore::new();
        store
            .apply(&PermissionAction::Allow("bash".into()))
            .unwrap();
        store
            .apply(&PermissionAction::Disallow("write".into()))
            .unwrap();
        store.save(&dir).unwrap();
        assert!(!dir.join(format!("{STORE_FILE}.tmp")).exists(), "temp renamed");
        let loaded = PermissionStore::load(&dir);
        assert!(loaded.is_enabled("bash"));
        assert!(!loaded.is_enabled("write"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn malformed_store_fails_closed_and_filters_bad_entries() {
        let dir = temp_dir("malformed");
        std::fs::write(dir.join(STORE_FILE), "{not json").unwrap();
        assert!(PermissionStore::load(&dir).enabled().is_empty());
        std::fs::write(
            dir.join(STORE_FILE),
            r#"{"allowed":["bash","human.approve","*","bad name"],"denied":[]}"#,
        )
        .unwrap();
        let loaded = PermissionStore::load(&dir);
        assert_eq!(loaded.enabled(), vec!["bash".to_owned()]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sets_are_bounded() {
        let mut store = PermissionStore::new();
        for i in 0..MAX_TOOLS {
            store
                .apply(&PermissionAction::Allow(format!("tool{i}")))
                .unwrap();
        }
        assert_eq!(
            store.apply(&PermissionAction::Allow("one-too-many".into())),
            Err(PermissionCommandError::TooManyTools)
        );
    }
}
