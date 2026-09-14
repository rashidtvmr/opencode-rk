//! Star-proof validator (SEC-017): wildcard `*` cannot bypass mandatory controls.
#![forbid(unsafe_code)]
use super::{PermissionSet, RuleEffect};
use std::fmt;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum MandatoryControl {
    SecretFileProtection,
    DestructiveCommandBlock,
    SystemFileWriteBlock,
    SsrfBlock,
    SqlInjectionBlock,
}
impl fmt::Display for MandatoryControl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SecretFileProtection => write!(f, "secret-file protection"),
            Self::DestructiveCommandBlock => write!(f, "destructive-command block"),
            Self::SystemFileWriteBlock => write!(f, "system-file write block"),
            Self::SsrfBlock => write!(f, "SSRF block"),
            Self::SqlInjectionBlock => write!(f, "SQL-injection block"),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Violation {
    pub control: MandatoryControl,
    pub description: String,
}
#[derive(Clone, Debug, Default)]
pub struct StarProofValidator;
impl StarProofValidator {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
    #[must_use]
    pub fn validate(&self, permissions: &PermissionSet) -> Vec<Violation> {
        let mut hit = [false; 5];
        for rule in &permissions.rules {
            if rule.effect != RuleEffect::Allow {
                continue;
            }
            for (i, control) in ALL.iter().enumerate() {
                if pattern_covers(&rule.pattern, control) {
                    hit[i] = true;
                }
            }
        }
        ALL.iter()
            .zip(hit)
            .filter(|(_, h)| *h)
            .map(|(c, _)| Violation {
                control: *c,
                description: format!(
                    "allow rule with wildcard scope would bypass mandatory {c}; mandatory control holds regardless of permission grants"
                ),
            })
            .collect()
    }
}
const ALL: [MandatoryControl; 5] = [
    MandatoryControl::SecretFileProtection,
    MandatoryControl::DestructiveCommandBlock,
    MandatoryControl::SystemFileWriteBlock,
    MandatoryControl::SsrfBlock,
    MandatoryControl::SqlInjectionBlock,
];
fn is_global(pattern: &str) -> bool {
    let t = pattern.trim();
    !t.is_empty() && t.contains('*') && t.chars().all(|c| c == '*' || c == '/' || c == '?')
}
fn tokens(lower: &str) -> Vec<&str> {
    lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect()
}
fn has_token(lower: &str, words: &[&str]) -> bool {
    let t = tokens(lower);
    words.iter().any(|w| t.contains(w))
}
fn contains_any(lower: &str, subs: &[&str]) -> bool {
    subs.iter().any(|s| lower.contains(s))
}
fn pattern_covers(pattern: &str, control: &MandatoryControl) -> bool {
    if is_global(pattern) {
        return true;
    }
    let lower = pattern.to_ascii_lowercase();
    match control {
        MandatoryControl::SecretFileProtection => contains_any(
            &lower,
            &[
                ".env",
                ".ssh",
                ".aws",
                "credential",
                "token",
                "secret",
                "gcloud",
                ".gnupg",
                "id_rsa",
                "id_ed25519",
                ".npmrc",
                ".kube",
                ".docker",
                "git-credential",
                ".p12",
                ".pem",
            ],
        ),
        MandatoryControl::DestructiveCommandBlock => has_token(
            &lower,
            &[
                "rm",
                "rmdir",
                "mkfs",
                "dd",
                "chmod",
                "chown",
                "kill",
                "docker",
                "kubectl",
                "git",
                "find",
                "format",
                "fdisk",
                "systemctl",
                "pip",
            ],
        ),
        MandatoryControl::SystemFileWriteBlock => contains_any(
            &lower,
            &[
                "/etc",
                "/usr",
                "/bin",
                "/sbin",
                "/boot",
                "/system",
                "/proc",
                "/sys",
                "c:/windows",
                "c:/program files",
            ],
        ),
        MandatoryControl::SsrfBlock => contains_any(
            &lower,
            &[
                "http",
                "://",
                "url",
                "fetch",
                "curl",
                "wget",
                "ssrf",
                "localhost",
                "169.254",
                "192.168",
                "metadata.google",
            ],
        ),
        MandatoryControl::SqlInjectionBlock => {
            lower.contains("sql")
                || lower.contains("database")
                || has_token(
                    &lower,
                    &[
                        "drop", "select", "insert", "update", "delete", "grant", "revoke",
                        "truncate", "alter", "attach",
                    ],
                )
        }
    }
}
#[cfg(test)]
mod tests {
    use super::super::{Decision, FileAction, OperationIntent, PermissionBroker, SecurityPolicy};
    use super::*;
    use std::path::PathBuf;
    fn file(action: FileAction, path: &str) -> OperationIntent {
        OperationIntent::File {
            action,
            path: PathBuf::from(path),
        }
    }
    fn proc(program: &str, args: &[&str]) -> OperationIntent {
        OperationIntent::Process {
            program: program.to_owned(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            cwd: PathBuf::from("/work/project"),
        }
    }
    fn sql(statement: &str) -> OperationIntent {
        OperationIntent::Sql {
            statement: statement.to_owned(),
            database: "workspace".to_owned(),
        }
    }
    fn broker_star() -> PermissionBroker {
        PermissionBroker::new(SecurityPolicy::lean_default("/work/project"))
            .with_permissions(PermissionSet::star())
    }
    fn has(v: &[Violation], c: MandatoryControl) -> bool {
        v.iter().any(|x| x.control == c)
    }
    #[test]
    fn star_cannot_bypass_secrets() {
        let v = StarProofValidator::new().validate(&PermissionSet::star());
        assert!(has(&v, MandatoryControl::SecretFileProtection));
        assert!(matches!(
            broker_star().authorize(&file(FileAction::Read, "/work/project/.env")),
            Decision::Deny { .. }
        ));
    }
    #[test]
    fn star_cannot_bypass_destructive() {
        let v = StarProofValidator::new().validate(&PermissionSet::star());
        assert!(has(&v, MandatoryControl::DestructiveCommandBlock));
        assert!(matches!(
            broker_star().authorize(&proc("rm", &["-rf", "*"])),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            broker_star().authorize(&sql("DROP TABLE users")),
            Decision::Deny { .. }
        ));
    }
    #[test]
    fn star_cannot_bypass_syswrite() {
        let v = StarProofValidator::new().validate(&PermissionSet::star());
        assert!(has(&v, MandatoryControl::SystemFileWriteBlock));
        assert!(matches!(
            broker_star().authorize(&file(FileAction::Write, "/etc/hosts")),
            Decision::Deny { .. }
        ));
    }
    #[test]
    fn clean_permissions_pass() {
        let scoped = PermissionSet::new(vec![super::super::PermissionRule::new(
            "/work/project/src/**",
            RuleEffect::Allow,
        )]);
        assert!(StarProofValidator::new().validate(&scoped).is_empty());
        assert!(StarProofValidator::new()
            .validate(&PermissionSet::default())
            .is_empty());
    }
    #[test]
    fn mixed_violations_detected() {
        let v = StarProofValidator::new().validate(&PermissionSet::star());
        assert_eq!(v.len(), 5);
        for c in ALL {
            assert!(has(&v, c), "missing {c}");
        }
        let deny_star = PermissionSet::new(vec![super::super::PermissionRule::new(
            "*",
            RuleEffect::Deny,
        )]);
        assert!(StarProofValidator::new().validate(&deny_star).is_empty());
    }
}
