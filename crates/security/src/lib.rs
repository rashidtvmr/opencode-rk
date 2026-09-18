//! Deterministic authorization and destructive-operation classification.
#![forbid(unsafe_code)]
pub mod app_policy;
pub mod platform_matrix;
pub mod clarity_guard;
pub mod cmd_patterns;
pub mod credentials;
pub mod env_restrict;
pub mod exec_policy;
pub mod hook_bus_v2;
pub mod hooks;
pub mod manual_only;
pub mod project_boundary;
pub mod sandbox;
pub mod sensitive;
pub mod shell_denylist;
pub mod spawn;
pub mod sql_classify;
pub mod ssrf;
pub mod star_proof;
pub mod sysfiles;
pub mod trusted;
use opencode_rk_contracts::ApprovalId;
use serde::{Deserialize, Serialize};
use std::{
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileAction {
    Read,
    Write,
    Delete,
    CreateDirectory,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OperationIntent {
    File {
        action: FileAction,
        path: PathBuf,
    },
    Process {
        program: String,
        args: Vec<String>,
        cwd: PathBuf,
    },
    Sql {
        statement: String,
        database: String,
    },
    Tool {
        name: String,
        description: String,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum Decision {
    Allow,
    Deny {
        reason: String,
    },
    RequireHuman {
        approval_id: ApprovalId,
        reason: String,
    },
}
impl Decision {
    #[must_use]
    pub fn is_mandatory(&self) -> bool {
        matches!(self, Decision::Deny { .. } | Decision::RequireHuman { .. })
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub protected_mode: bool,
    pub destructive_command_protection: bool,
    pub allow_secret_reads: bool,
    pub project_roots: Vec<PathBuf>,
    pub explicitly_allowed_roots: Vec<PathBuf>,
    pub system_readable: bool,
}
impl SecurityPolicy {
    #[must_use]
    pub fn lean_default(project_root: impl Into<PathBuf>) -> Self {
        Self {
            protected_mode: true,
            destructive_command_protection: true,
            allow_secret_reads: false,
            project_roots: vec![project_root.into()],
            explicitly_allowed_roots: Vec::new(),
            system_readable: true,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleEffect {
    Allow,
    Deny,
    Ask,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PermissionRule {
    pub pattern: String,
    pub effect: RuleEffect,
}
impl PermissionRule {
    #[must_use]
    pub fn new(pattern: impl Into<String>, effect: RuleEffect) -> Self {
        Self {
            pattern: pattern.into(),
            effect,
        }
    }
    #[must_use]
    pub fn matches(&self, intent: &OperationIntent) -> bool {
        self.pattern == "*" || glob_match(&self.pattern, &rule_scope(intent))
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PermissionSet {
    pub rules: Vec<PermissionRule>,
}
impl PermissionSet {
    #[must_use]
    pub fn new(rules: Vec<PermissionRule>) -> Self {
        Self { rules }
    }
    #[must_use]
    pub fn star() -> Self {
        Self {
            rules: vec![PermissionRule::new("*", RuleEffect::Allow)],
        }
    }
    #[must_use]
    pub fn evaluate(&self, intent: &OperationIntent) -> Option<&PermissionRule> {
        self.rules.iter().find(|rule| rule.matches(intent))
    }
}
#[derive(Clone, Debug)]
pub struct AuditEntry {
    pub seq: u64,
    pub at: SystemTime,
    pub intent: OperationIntent,
    pub decision: Decision,
}
pub const MAX_AUDIT_ENTRIES: usize = 1024;
#[derive(Clone)]
pub struct PermissionBroker {
    generation: u64,
    policy: Arc<SecurityPolicy>,
    permissions: PermissionSet,
    audit: Arc<Mutex<Vec<AuditEntry>>>,
}
impl PermissionBroker {
    #[must_use]
    pub fn new(policy: SecurityPolicy) -> Self {
        Self {
            generation: 1,
            policy: Arc::new(policy),
            permissions: PermissionSet::default(),
            audit: Arc::new(Mutex::new(Vec::new())),
        }
    }
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }
    #[must_use]
    pub fn permissions(&self) -> &PermissionSet {
        &self.permissions
    }
    #[must_use]
    pub fn with_policy(&self, policy: SecurityPolicy) -> Self {
        Self {
            generation: self.generation.saturating_add(1),
            policy: Arc::new(policy),
            permissions: self.permissions.clone(),
            audit: Arc::clone(&self.audit),
        }
    }
    #[must_use]
    pub fn with_permissions(&self, permissions: PermissionSet) -> Self {
        Self {
            generation: self.generation.saturating_add(1),
            policy: Arc::clone(&self.policy),
            permissions,
            audit: Arc::clone(&self.audit),
        }
    }
    #[must_use]
    pub fn audit_len(&self) -> usize {
        self.audit.lock().map(|log| log.len()).unwrap_or(0)
    }
    #[must_use]
    pub fn audit(&self) -> Vec<AuditEntry> {
        self.audit.lock().map(|log| log.clone()).unwrap_or_default()
    }
    #[must_use]
    pub fn authorize(&self, intent: &OperationIntent) -> Decision {
        let baseline = self.decide(intent);
        let decision = match baseline {
            Decision::Allow => match self.permissions.evaluate(intent) {
                None
                | Some(PermissionRule {
                    effect: RuleEffect::Allow,
                    ..
                }) => Decision::Allow,
                Some(rule) if rule.effect == RuleEffect::Deny => Decision::Deny {
                    reason: format!("denied by permission rule {:?}", rule.pattern),
                },
                Some(rule) => human_required(&format!(
                    "permission rule {:?} requires human approval",
                    rule.pattern
                )),
            },
            other => other,
        };
        self.record(intent, &decision);
        decision
    }
    fn record(&self, intent: &OperationIntent, decision: &Decision) {
        if let Ok(mut log) = self.audit.lock() {
            let seq = log.last().map_or(0, |entry| entry.seq.saturating_add(1));
            log.push(AuditEntry {
                seq,
                at: SystemTime::now(),
                intent: intent.clone(),
                decision: decision.clone(),
            });
            if log.len() > MAX_AUDIT_ENTRIES {
                let overflow = log.len() - MAX_AUDIT_ENTRIES;
                log.drain(..overflow);
            }
        }
    }
    fn decide(&self, intent: &OperationIntent) -> Decision {
        match intent {
            OperationIntent::File { action, path } => self.authorize_file(*action, path),
            OperationIntent::Process { program, args, cwd } => {
                self.authorize_process(program, args, cwd)
            }
            OperationIntent::Sql {
                statement,
                database,
            } => self.authorize_sql(statement, database),
            OperationIntent::Tool { .. } => Decision::Allow,
        }
    }
    fn authorize_file(&self, action: FileAction, path: &Path) -> Decision {
        let normalized = lexical_normalize(path);
        if is_secret_path(&normalized) && !self.policy.allow_secret_reads {
            return Decision::Deny {
                reason: "secret-bearing paths are not accessible to agents".to_owned(),
            };
        }
        if is_system_path(&normalized) {
            return match action {
                FileAction::Read if self.policy.system_readable => Decision::Allow,
                FileAction::Read => Decision::Deny {
                    reason: "system-file reads are disabled by policy".to_owned(),
                },
                _ => Decision::Deny {
                    reason: "system paths are read-only to agents".to_owned(),
                },
            };
        }
        if matches!(
            action,
            FileAction::Write | FileAction::Delete | FileAction::CreateDirectory
        ) && !self.is_within_writable_root(&normalized)
        {
            return human_required(
                "write access outside approved project roots requires explicit human approval",
            );
        }
        if action == FileAction::Delete && self.policy.destructive_command_protection {
            return human_required(
                "file deletion requires an explicit human approval in protected mode",
            );
        }
        Decision::Allow
    }
    fn authorize_process(&self, program: &str, args: &[String], cwd: &Path) -> Decision {
        let executable = basename(program).to_ascii_lowercase();
        if is_shell_interpreter(&executable) && args.iter().any(|arg| arg == "-c" || arg == "/c") {
            return human_required("opaque shell command strings require human approval; direct argv execution is preferred");
        }
        if self.policy.destructive_command_protection {
            if let Some(reason) = classify_destructive_argv(&executable, args, cwd) {
                return Decision::Deny { reason };
            }
        }
        Decision::Allow
    }
    fn authorize_sql(&self, statement: &str, database: &str) -> Decision {
        if !self.policy.destructive_command_protection {
            return Decision::Allow;
        }
        let upper = statement.to_ascii_uppercase();
        let words: Vec<&str> = upper
            .split(|c: char| !(c.is_ascii_alphabetic()))
            .filter(|w| !w.is_empty())
            .collect();
        let has_where = upper.contains(" WHERE ");
        let obj = |i: usize| {
            matches!(
                words.get(i..i + 2),
                Some([
                    "INDEX"
                        | "VIEW"
                        | "SCHEMA"
                        | "USER"
                        | "DATABASE"
                        | "TABLE"
                        | "ROLE"
                        | "TRIGGER"
                        | "SEQUENCE",
                    ..
                ])
            )
        };
        match words.as_slice(){["DROP",..]if obj(1)=>Decision::Deny{reason:format!("DROP is blocked in protected mode (database: {database})")},["TRUNCATE",..]=>Decision::Deny{reason:format!("TRUNCATE is blocked in protected mode (database: {database})")},["GRANT",..]|["REVOKE",..]=>Decision::Deny{reason:format!("GRANT/REVOKE privilege changes are never executed by an agent (database: {database})")},["CREATE","UNIQUE",..]|["CREATE","INDEX",..]|["DROP","CLUSTER",..]|["ALTER","INDEX",..]|["REINDEX",..]=>human_required("index or cluster structural changes require human approval"),["DELETE",..]if !has_where=>human_required("DELETE without an explicit WHERE clause requires human approval"),["UPDATE",..]if !has_where=>human_required("UPDATE without an explicit WHERE clause requires human approval"),["ALTER","TABLE",..]=>human_required("ALTER TABLE requires human approval"),_=>Decision::Allow}
    }
    fn is_within_writable_root(&self, path: &Path) -> bool {
        self.policy
            .project_roots
            .iter()
            .chain(&self.policy.explicitly_allowed_roots)
            .map(|root| lexical_normalize(root))
            .any(|root| path.starts_with(root))
    }
}
fn rule_scope(intent: &OperationIntent) -> String {
    match intent {
        OperationIntent::File { path, .. } => {
            lexical_normalize(path).to_string_lossy().replace('\\', "/")
        }
        OperationIntent::Process { program, .. } => basename(program).to_owned(),
        OperationIntent::Sql { statement, .. } => statement.clone(),
        OperationIntent::Tool { name, .. } => name.clone(),
    }
}
#[must_use]
pub fn glob_match(pattern: &str, text: &str) -> bool {
    glob_bytes(pattern.as_bytes(), text.as_bytes())
}
fn glob_bytes(pattern: &[u8], text: &[u8]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some((b'*', rest)) => {
            if rest.first() == Some(&b'*') {
                let mut tail = &rest[1..];
                while tail.first() == Some(&b'*') {
                    tail = &tail[1..];
                }
                (0..=text.len()).any(|i| glob_bytes(tail, &text[i..]))
            } else {
                for i in 0..=text.len() {
                    if glob_bytes(rest, &text[i..]) {
                        return true;
                    }
                    if i == text.len() || text[i] == b'/' {
                        break;
                    }
                }
                false
            }
        }
        Some((b'?', rest)) => !text.is_empty() && text[0] != b'/' && glob_bytes(rest, &text[1..]),
        Some((byte, rest)) => !text.is_empty() && text[0] == *byte && glob_bytes(rest, &text[1..]),
    }
}
fn human_required(reason: &str) -> Decision {
    Decision::RequireHuman {
        approval_id: ApprovalId::new(),
        reason: reason.to_owned(),
    }
}
fn basename(program: &str) -> &str {
    program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(program)
        .trim_end_matches(".exe")
}
fn is_shell_interpreter(program: &str) -> bool {
    matches!(
        program,
        "sh" | "bash" | "zsh" | "fish" | "dash" | "cmd" | "powershell" | "pwsh"
    )
}
#[must_use]
pub fn classify_destructive_argv(program: &str, args: &[String], cwd: &Path) -> Option<String> {
    match program {
        "rm" => classify_rm(args, cwd),
        "rmdir" => args
            .iter()
            .any(|arg| is_system_delete_target(arg))
            .then(|| "deleting system-level paths is never permitted for agents".to_owned()),
        "mkfs" | "mkfs.ext4" | "mkfs.xfs" | "format" => {
            Some("filesystem formatting is never executed by an agent".to_owned())
        }
        "dd" if args.iter().any(|arg| arg.starts_with("of=/dev/")) => {
            Some("raw-device writes are never executed by an agent".to_owned())
        }
        "find" if args.iter().any(|arg| arg == "-delete") => {
            Some("recursive find -delete requires a human-operated command".to_owned())
        }
        "git" => classify_git(args),
        "chmod" if args.iter().any(|arg| is_world_writable_mode(arg)) => {
            Some("world-writable file modes are never executed by an agent".to_owned())
        }
        "chown"
            if args
                .iter()
                .any(|arg| arg == "root" || arg.starts_with("root:")) =>
        {
            Some("ownership changes to root are never executed by an agent".to_owned())
        }
        "kill"
            if args
                .iter()
                .any(|arg| matches!(arg.as_str(), "-9" | "-KILL" | "-SIGKILL"))
                || args.windows(2).any(|pair| {
                    pair[0] == "-s" && matches!(pair[1].as_str(), "KILL" | "SIGKILL" | "9")
                }) =>
        {
            Some("SIGKILL process termination requires a human-operated command".to_owned())
        }
        "docker"
            if matches!(args.first().map(String::as_str), Some("rm" | "rmi"))
                || (args.first().map(String::as_str) == Some("volume")
                    && args.iter().any(|arg| arg == "rm")) =>
        {
            Some(
                "docker object removal is blocked; present the command to the user instead"
                    .to_owned(),
            )
        }
        "kubectl"
            if matches!(
                args.first().map(String::as_str),
                Some("delete" | "drain" | "cordon")
            ) =>
        {
            Some("kubectl cluster-object mutation requires a human-operated command".to_owned())
        }
        "pip" | "pip3" if args.iter().any(|arg| arg == "--break-system-packages") => {
            Some("breaking system-package protection is never executed by an agent".to_owned())
        }
        _ => None,
    }
}
fn classify_git(args: &[String]) -> Option<String> {
    let rest = &args[1.min(args.len())..];
    let force = rest.iter().any(|arg| {
        arg == "-f"
            || arg == "--force"
            || arg == "--force-with-lease"
            || (arg.starts_with('-') && !arg.starts_with("--") && arg[1..].contains('f'))
    });
    match args.first().map(String::as_str).unwrap_or("") {
        "push" if force => Some(
            "forced git push rewrites remote history; present the command to the user instead"
                .to_owned(),
        ),
        "reset" if rest.iter().any(|arg| arg == "--hard") => Some(
            "git reset --hard discards local work; present the command to the user instead"
                .to_owned(),
        ),
        "clean" if force => Some(
            "forced git clean deletes untracked files; present the command to the user instead"
                .to_owned(),
        ),
        _ => None,
    }
}
fn is_world_writable_mode(arg: &str) -> bool {
    let mode = arg.rsplit([':', '(']).next().unwrap_or(arg).trim();
    if mode.len() == 3 && mode.chars().all(|c| c.is_ascii_digit()) {
        matches!(mode.as_bytes()[2], b'2' | b'3' | b'6' | b'7')
    } else {
        matches!(mode, "a+rwx" | "o+w" | "a+w" | "go+w" | "uog+w")
    }
}
fn classify_rm(args: &[String], cwd: &Path) -> Option<String> {
    let recursive = args.iter().any(|arg| {
        arg == "-r"
            || arg == "-R"
            || arg == "--recursive"
            || (arg.starts_with('-') && arg.len() > 2 && arg[1..].contains('r'))
    });
    let force = args.iter().any(|arg| {
        arg == "-f"
            || arg == "--force"
            || (arg.starts_with('-') && arg.len() > 2 && arg[1..].contains('f'))
    });
    let targets: Vec<&str> = args
        .iter()
        .filter(|arg| !arg.starts_with('-'))
        .map(String::as_str)
        .collect();
    if recursive
        && force
        && targets
            .iter()
            .any(|target| is_broad_delete_target(target, cwd))
    {
        return Some(
            "broad recursive forced deletion is blocked; present the command to the user instead"
                .to_owned(),
        );
    }
    if targets.iter().any(|target| is_system_delete_target(target)) {
        return Some("deleting system-level paths is never permitted for agents".to_owned());
    }
    None
}
fn is_broad_delete_target(target: &str, cwd: &Path) -> bool {
    matches!(target.trim(), "*" | "./*" | "." | ".." | "../" | "/")
        || (cwd == Path::new("/") && matches!(target.trim(), "*" | "./*"))
}
fn is_system_delete_target(target: &str) -> bool {
    let n = target.replace('\\', "/").to_ascii_lowercase();
    n == "/"
        || n.starts_with("/etc")
        || n.starts_with("/usr")
        || n.starts_with("/bin")
        || n.starts_with("/sbin")
        || n.starts_with("/boot")
        || n.starts_with("/system")
        || n.starts_with("c:/windows")
        || n.starts_with("c:/program files")
}
fn lexical_normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}
fn is_secret_path(path: &Path) -> bool {
    let lower = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == ".env" || name.starts_with(".env."))
        || lower.contains("/.ssh/")
        || lower.ends_with("/.ssh")
        || lower.contains("/.aws/")
        || lower.contains("/.config/gcloud/")
        || lower.contains("/credentials")
}
fn is_system_path(path: &Path) -> bool {
    let lower = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    ["/etc/", "/usr/", "/bin/", "/sbin/", "/boot/", "/system/"]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
        || matches!(
            lower.as_str(),
            "/etc" | "/usr" | "/bin" | "/sbin" | "/boot" | "/system"
        )
        || lower.starts_with("c:/windows/")
        || lower.starts_with("c:/program files/")
}
#[cfg(test)]
mod tests {
    use super::*;
    fn broker() -> PermissionBroker {
        PermissionBroker::new(SecurityPolicy::lean_default("/work/project"))
    }
    fn star() -> PermissionBroker {
        broker().with_permissions(PermissionSet::star())
    }
    fn file(action: FileAction, path: &str) -> OperationIntent {
        OperationIntent::File {
            action,
            path: PathBuf::from(path),
        }
    }
    fn proc(program: &str, args: &[&str]) -> OperationIntent {
        OperationIntent::Process {
            program: program.to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            cwd: PathBuf::from("/work/project"),
        }
    }
    fn sql(statement: &str) -> OperationIntent {
        OperationIntent::Sql {
            statement: statement.to_owned(),
            database: "workspace".to_owned(),
        }
    }
    #[test]
    fn system_write_denied() {
        assert!(matches!(
            broker().authorize(&OperationIntent::File {
                action: FileAction::Write,
                path: PathBuf::from("/etc/hosts")
            }),
            Decision::Deny { .. }
        ));
    }
    #[test]
    fn env_denied() {
        assert!(matches!(
            broker().authorize(&OperationIntent::File {
                action: FileAction::Read,
                path: PathBuf::from("/work/project/.env")
            }),
            Decision::Deny { .. }
        ));
    }
    #[test]
    fn ordinary_project_write_allowed() {
        assert_eq!(
            broker().authorize(&OperationIntent::File {
                action: FileAction::Write,
                path: PathBuf::from("/work/project/src/lib.rs")
            }),
            Decision::Allow
        );
    }
    #[test]
    fn project_delete_human() {
        assert!(matches!(
            broker().authorize(&OperationIntent::File {
                action: FileAction::Delete,
                path: PathBuf::from("/work/project/generated.tmp")
            }),
            Decision::RequireHuman { .. }
        ));
    }
    #[test]
    fn rm_rf_star_blocked() {
        assert!(matches!(
            broker().authorize(&OperationIntent::Process {
                program: "rm".to_owned(),
                args: vec!["-rf".to_owned(), "*".to_owned()],
                cwd: PathBuf::from("/work/project")
            }),
            Decision::Deny { .. }
        ));
    }
    #[test]
    fn shell_c_human() {
        assert!(matches!(
            broker().authorize(&OperationIntent::Process {
                program: "/bin/bash".to_owned(),
                args: vec!["-c".to_owned(), "echo hi".to_owned()],
                cwd: PathBuf::from("/work/project")
            }),
            Decision::RequireHuman { .. }
        ));
    }
    #[test]
    fn destructive_sql_gated() {
        assert!(matches!(
            broker().authorize(&OperationIntent::Sql {
                statement: "DROP TABLE users".to_owned(),
                database: "workspace".to_owned()
            }),
            Decision::Deny { .. }
        ));
    }
    #[test]
    fn star_allows_ordinary_write() {
        assert_eq!(
            star().authorize(&file(FileAction::Write, "/work/project/src/lib.rs")),
            Decision::Allow
        );
    }
    #[test]
    fn star_cannot_read_secrets() {
        assert!(star()
            .authorize(&file(FileAction::Read, "/work/project/.env"))
            .is_mandatory());
        assert!(matches!(
            star().authorize(&file(FileAction::Read, "/work/project/.env")),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            star().authorize(&file(FileAction::Read, "/home/user/.ssh/id_rsa")),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            star().authorize(&file(FileAction::Read, "/work/project/.aws/credentials")),
            Decision::Deny { .. }
        ));
    }
    #[test]
    fn star_cannot_write_system() {
        assert!(matches!(
            star().authorize(&file(FileAction::Write, "/etc/hosts")),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            star().authorize(&file(FileAction::Delete, "/usr/bin/tool")),
            Decision::Deny { .. }
        ));
    }
    #[test]
    fn targeted_allow_cannot_lift_mandatory() {
        let targeted = broker().with_permissions(PermissionSet::new(vec![
            PermissionRule::new("/etc/**", RuleEffect::Allow),
            PermissionRule::new("**/.env", RuleEffect::Allow),
        ]));
        assert!(matches!(
            targeted.authorize(&file(FileAction::Write, "/etc/hosts")),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            targeted.authorize(&file(FileAction::Read, "/work/project/.env")),
            Decision::Deny { .. }
        ));
    }
    #[test]
    fn star_cannot_bypass_human_gates() {
        assert!(matches!(
            star().authorize(&file(FileAction::Delete, "/work/project/generated.tmp")),
            Decision::RequireHuman { .. }
        ));
        assert!(matches!(
            star().authorize(&file(FileAction::Write, "/tmp/outside.txt")),
            Decision::RequireHuman { .. }
        ));
        assert!(matches!(
            star().authorize(&proc("/bin/bash", &["-c", "echo hi"])),
            Decision::RequireHuman { .. }
        ));
        assert!(matches!(
            star().authorize(&sql("DELETE FROM users")),
            Decision::RequireHuman { .. }
        ));
        assert!(matches!(
            star().authorize(&sql("ALTER TABLE users ADD COLUMN x TEXT")),
            Decision::RequireHuman { .. }
        ));
    }
    #[test]
    fn star_cannot_run_destructive() {
        assert!(matches!(
            star().authorize(&proc("rm", &["-rf", "*"])),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            star().authorize(&proc("mkfs", &["/dev/sda1"])),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            star().authorize(&sql("DROP TABLE users")),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            star().authorize(&sql("TRUNCATE users")),
            Decision::Deny { .. }
        ));
    }
    #[test]
    fn deny_rule_narrows_ordinary() {
        let scoped = broker().with_permissions(PermissionSet::new(vec![PermissionRule::new(
            "/work/project/scratch/**",
            RuleEffect::Deny,
        )]));
        assert!(matches!(
            scoped.authorize(&file(FileAction::Write, "/work/project/scratch/note.txt")),
            Decision::Deny { .. }
        ));
        assert_eq!(
            scoped.authorize(&file(FileAction::Write, "/work/project/src/lib.rs")),
            Decision::Allow
        );
    }
    #[test]
    fn ask_rule_requires_human_for_ordinary() {
        let gated = broker().with_permissions(PermissionSet::new(vec![PermissionRule::new(
            "/work/project/release/**",
            RuleEffect::Ask,
        )]));
        assert!(matches!(
            gated.authorize(&file(FileAction::Write, "/work/project/release/bin")),
            Decision::RequireHuman { .. }
        ));
        assert_eq!(
            gated.authorize(&file(FileAction::Write, "/work/project/src/lib.rs")),
            Decision::Allow
        );
    }
    #[test]
    fn first_match_wins() {
        let allow_first = broker().with_permissions(PermissionSet::new(vec![
            PermissionRule::new("/work/project/**", RuleEffect::Allow),
            PermissionRule::new("/work/project/scratch/**", RuleEffect::Deny),
        ]));
        assert_eq!(
            allow_first.authorize(&file(FileAction::Write, "/work/project/scratch/n.txt")),
            Decision::Allow
        );
        let deny_first = broker().with_permissions(PermissionSet::new(vec![
            PermissionRule::new("/work/project/scratch/**", RuleEffect::Deny),
            PermissionRule::new("/work/project/**", RuleEffect::Allow),
        ]));
        assert!(matches!(
            deny_first.authorize(&file(FileAction::Write, "/work/project/scratch/n.txt")),
            Decision::Deny { .. }
        ));
    }
    #[test]
    fn glob_semantics() {
        assert!(glob_match("**", "/a/b/c"));
        assert!(glob_match("/work/project/**", "/work/project/src/lib.rs"));
        assert!(!glob_match("/work/project/*", "/work/project/src/lib.rs"));
        assert!(glob_match("/work/project/*", "/work/project/Cargo.toml"));
        assert!(glob_match("rm", "rm"));
        assert!(!glob_match("r?", "rmdir"));
        assert!(glob_match("r?", "rm"));
        assert!(PermissionRule::new("*", RuleEffect::Allow).matches(&sql("DROP TABLE users")));
        assert!(PermissionRule::new("*", RuleEffect::Allow).matches(&proc("rm", &["-rf"])));
    }
    #[test]
    fn audit_records_seq_and_bounds() {
        let b = star();
        assert_eq!(b.audit_len(), 0);
        let _ = b.authorize(&file(FileAction::Write, "/work/project/a.rs"));
        let _ = b.authorize(&file(FileAction::Read, "/work/project/.env"));
        assert_eq!(b.audit_len(), 2);
        let trail = b.audit();
        assert_eq!(trail.len(), 2);
        assert!(trail[0].seq < trail[1].seq);
        assert_eq!(trail[0].decision, Decision::Allow);
        assert!(trail[1].decision.is_mandatory());
        for i in 0..(MAX_AUDIT_ENTRIES + 5) {
            let _ = b.authorize(&file(
                FileAction::Write,
                &format!("/work/project/gen{i}.rs"),
            ));
        }
        assert_eq!(b.audit_len(), MAX_AUDIT_ENTRIES);
        let capped = b.audit();
        assert_eq!(capped.first().map(|entry| entry.seq), Some(7));
        assert!(capped.windows(2).all(|pair| pair[0].seq + 1 == pair[1].seq));
    }
    #[test]
    fn generation_bumps_and_audit_shared() {
        let base = broker();
        let forked = base.with_permissions(PermissionSet::star());
        assert_eq!(forked.generation(), base.generation() + 1);
        assert_eq!(forked.permissions(), &PermissionSet::star());
        let _ = forked.authorize(&file(FileAction::Write, "/work/project/a.rs"));
        assert_eq!(base.audit_len(), 1);
        let repoliced = forked.with_policy(SecurityPolicy::lean_default("/work/other"));
        assert_eq!(repoliced.generation(), forked.generation() + 1);
        assert_eq!(repoliced.permissions(), &PermissionSet::star());
        assert_eq!(repoliced.audit_len(), 1);
    }
    #[test]
    fn git_destructive_blocked() {
        assert!(matches!(
            broker().authorize(&proc("git", &["push", "--force", "origin", "main"])),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            broker().authorize(&proc("git", &["push", "-f"])),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            broker().authorize(&proc("git", &["push", "--force-with-lease"])),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            broker().authorize(&proc("git", &["reset", "--hard", "HEAD~3"])),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            broker().authorize(&proc("git", &["clean", "-fd"])),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            broker().authorize(&proc("git", &["clean", "-fx"])),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            broker().authorize(&proc("git", &["clean", "--force", "-d"])),
            Decision::Deny { .. }
        ));
        assert_eq!(
            broker().authorize(&proc("git", &["push", "origin", "main"])),
            Decision::Allow
        );
        assert_eq!(
            broker().authorize(&proc("git", &["reset", "--soft", "HEAD~1"])),
            Decision::Allow
        );
        assert_eq!(
            broker().authorize(&proc("git", &["status"])),
            Decision::Allow
        );
    }
    #[test]
    fn chmod_chown_kill_blocked() {
        assert!(matches!(
            broker().authorize(&proc("chmod", &["777", "script.sh"])),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            broker().authorize(&proc("chmod", &["a+rwx", "dir"])),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            broker().authorize(&proc("chmod", &["go+w", "file"])),
            Decision::Deny { .. }
        ));
        assert_eq!(
            broker().authorize(&proc("chmod", &["644", "file"])),
            Decision::Allow
        );
        assert_eq!(
            broker().authorize(&proc("chmod", &["u+x", "file"])),
            Decision::Allow
        );
        assert!(matches!(
            broker().authorize(&proc("chown", &["root", "/work/project/file"])),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            broker().authorize(&proc("chown", &["-R", "root:root", "dir"])),
            Decision::Deny { .. }
        ));
        assert_eq!(
            broker().authorize(&proc("chown", &["dev:dev", "file"])),
            Decision::Allow
        );
        assert!(matches!(
            broker().authorize(&proc("kill", &["-9", "1234"])),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            broker().authorize(&proc("kill", &["-KILL", "1234"])),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            broker().authorize(&proc("kill", &["-s", "KILL", "1234"])),
            Decision::Deny { .. }
        ));
        assert_eq!(
            broker().authorize(&proc("kill", &["-TERM", "1234"])),
            Decision::Allow
        );
        assert_eq!(
            broker().authorize(&proc("kill", &["1234"])),
            Decision::Allow
        );
    }
    #[test]
    fn docker_kubectl_pip_blocked() {
        assert!(matches!(
            broker().authorize(&proc("docker", &["rm", "-f", "container"])),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            broker().authorize(&proc("docker", &["rmi", "image"])),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            broker().authorize(&proc("docker", &["volume", "rm", "data"])),
            Decision::Deny { .. }
        ));
        assert_eq!(
            broker().authorize(&proc("docker", &["ps"])),
            Decision::Allow
        );
        assert_eq!(
            broker().authorize(&proc("docker", &["volume", "ls"])),
            Decision::Allow
        );
        assert!(matches!(
            broker().authorize(&proc("kubectl", &["delete", "pod", "web-1"])),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            broker().authorize(&proc("kubectl", &["drain", "node-2"])),
            Decision::Deny { .. }
        ));
        assert_eq!(
            broker().authorize(&proc("kubectl", &["get", "pods"])),
            Decision::Allow
        );
        assert!(matches!(
            broker().authorize(&proc("pip", &["install", "--break-system-packages", "pkg"])),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            broker().authorize(&proc(
                "pip3",
                &["install", "pkg", "--break-system-packages"]
            )),
            Decision::Deny { .. }
        ));
        assert_eq!(
            broker().authorize(&proc("pip", &["install", "pkg"])),
            Decision::Allow
        );
    }
    #[test]
    fn star_cannot_bypass_new_destructives() {
        for intent in [
            proc("git", &["push", "--force"]),
            proc("git", &["reset", "--hard", "HEAD"]),
            proc("git", &["clean", "-fd"]),
            proc("chmod", &["777", "x"]),
            proc("chown", &["root", "x"]),
            proc("kill", &["-9", "1"]),
            proc("docker", &["rm", "c"]),
            proc("kubectl", &["delete", "pod", "p"]),
            proc("pip", &["install", "--break-system-packages", "p"]),
        ] {
            assert!(
                matches!(star().authorize(&intent), Decision::Deny { .. }),
                "* lifted destructive: {intent:?}"
            );
        }
    }
    #[test]
    fn sql_grant_revoke_gated() {
        assert!(matches!(
            broker().authorize(&sql("GRANT ALL ON db.* TO 'app'@'%'")),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            broker().authorize(&sql("REVOKE DELETE ON users FROM role")),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            broker().authorize(&sql("drop index ix_users_email on users")),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            broker().authorize(&sql("CREATE INDEX ix_tmp ON users (email)")),
            Decision::RequireHuman { .. }
        ));
        assert!(matches!(
            broker().authorize(&sql("CREATE UNIQUE INDEX ix_u ON users (email)")),
            Decision::RequireHuman { .. }
        ));
        assert!(matches!(
            broker().authorize(&sql("UPDATE users SET active = 0")),
            Decision::RequireHuman { .. }
        ));
        assert!(matches!(
            broker().authorize(&sql("update users set active=0 where id=7")),
            Decision::Allow
        ));
        assert!(matches!(
            broker().authorize(&sql("SELECT * FROM users")),
            Decision::Allow
        ));
        assert!(matches!(
            broker().authorize(&sql("DELETE FROM users WHERE id = 7")),
            Decision::Allow
        ));
    }
}
