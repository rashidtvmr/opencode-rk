//! Secure process spawning with env filtering and path restrictions (SEC-008). Policy only; argv exec via Command, never shell strings.
use std::{
    collections::HashMap,
    fmt,
    path::{Component, PathBuf},
    process::Command,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpawnDenied {
    pub program: String,
    pub reason: String,
}
impl SpawnDenied {
    fn new(program: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            reason: reason.into(),
        }
    }
}
impl fmt::Display for SpawnDenied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "spawn denied ({}): {}", self.program, self.reason)
    }
}
impl std::error::Error for SpawnDenied {}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpawnPolicy {
    pub allowed_programs: Vec<String>,
    pub denied_programs: Vec<String>,
    pub env_filter: bool,
}
impl SpawnPolicy {
    #[must_use]
    pub fn new(
        allowed_programs: Vec<String>,
        denied_programs: Vec<String>,
        env_filter: bool,
    ) -> Self {
        Self {
            allowed_programs,
            denied_programs,
            env_filter,
        }
    }
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            allowed_programs: Vec::new(),
            denied_programs: Vec::new(),
            env_filter: true,
        }
    }
    #[must_use]
    pub fn restrictive(allowed: Vec<String>) -> Self {
        Self {
            allowed_programs: allowed,
            denied_programs: Vec::new(),
            env_filter: true,
        }
    }
}
impl Default for SpawnPolicy {
    fn default() -> Self {
        Self::permissive()
    }
}
#[derive(Clone, Debug)]
pub struct SecureSpawner {
    policy: SpawnPolicy,
}
impl SecureSpawner {
    #[must_use]
    pub fn new(policy: SpawnPolicy) -> Self {
        Self { policy }
    }
    #[must_use]
    pub fn policy(&self) -> &SpawnPolicy {
        &self.policy
    }
    #[must_use]
    pub fn filter_env(&self, env: &HashMap<String, String>) -> HashMap<String, String> {
        if self.policy.env_filter {
            filter_env(env)
        } else {
            env.clone()
        }
    }
    pub fn validate_program(&self, program: &str) -> Result<(), SpawnDenied> {
        validate_program(program)?;
        let base = basename(program);
        for d in &self.policy.denied_programs {
            if d.as_str() == program || d.as_str() == base.as_str() {
                return Err(SpawnDenied::new(
                    program,
                    format!("program is denied by policy: {d}"),
                ));
            }
        }
        if !self.policy.allowed_programs.is_empty()
            && !self
                .policy
                .allowed_programs
                .iter()
                .any(|a| a.as_str() == program || a.as_str() == base.as_str())
        {
            return Err(SpawnDenied::new(
                program,
                "program is not in the allowed list",
            ));
        }
        Ok(())
    }
    pub fn checked_command(
        &self,
        program: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Command, SpawnDenied> {
        self.validate_program(program)?;
        let mut cmd = Command::new(program);
        cmd.args(args);
        cmd.env_clear();
        cmd.envs(self.filter_env(env));
        Ok(cmd)
    }
}
#[must_use]
pub fn filter_env(env: &HashMap<String, String>) -> HashMap<String, String> {
    env.iter()
        .filter(|(k, _)| !is_secret_key(k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}
pub fn validate_program(program: &str) -> Result<(), SpawnDenied> {
    if program.is_empty() {
        return Err(SpawnDenied::new(program, "empty program path"));
    }
    if program.contains('\0') {
        return Err(SpawnDenied::new(program, "program contains null byte"));
    }
    let norm = lexical_normalize_str(program);
    let lower = norm.to_ascii_lowercase();
    for prefix in ["/sbin", "/usr/sbin"] {
        if lower == prefix || lower.starts_with(&format!("{prefix}/")) {
            return Err(SpawnDenied::new(
                program,
                "system binaries under /sbin are never executed by an agent",
            ));
        }
    }
    Ok(())
}
fn is_secret_key(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    [
        "API_KEY",
        "SECRET",
        "TOKEN",
        "PASSWORD",
        "PASSWD",
        "PRIVATE_KEY",
        "CREDENTIAL",
        "AWS_",
        "AUTH",
        "ACCESS_KEY",
        "SESSION_KEY",
        "CLIENT_SECRET",
    ]
    .iter()
    .any(|pat| upper.contains(pat))
}
fn basename(program: &str) -> String {
    program
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(program)
        .trim_end_matches(".exe")
        .to_ascii_lowercase()
}
fn lexical_normalize_str(raw: &str) -> String {
    let tmp = raw.replace('\\', "/");
    let mut out = PathBuf::new();
    for component in std::path::Path::new(&tmp).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out.to_string_lossy().replace('\\', "/")
}
#[cfg(test)]
mod tests {
    use super::*;
    fn env_pair() -> HashMap<String, String> {
        [
            ("HOME", "/home/dev"),
            ("PATH", "/usr/bin:/bin"),
            ("OPENAI_API_KEY", "sk-secret"),
            ("GITHUB_TOKEN", "gh-secret"),
            ("DB_PASSWORD", "pw-secret"),
            ("CARGO_HOME", "/home/dev/.cargo"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
    }
    #[test]
    fn env_filtered() {
        let out = filter_env(&env_pair());
        assert!(out.contains_key("HOME"));
        assert!(out.contains_key("PATH"));
        assert!(out.contains_key("CARGO_HOME"));
        assert!(!out.contains_key("OPENAI_API_KEY"));
        assert!(!out.contains_key("GITHUB_TOKEN"));
        assert!(!out.contains_key("DB_PASSWORD"));
    }
    #[test]
    fn denied_program_blocked() {
        let s = SecureSpawner::new(SpawnPolicy::new(
            Vec::new(),
            vec!["evil-tool".to_owned()],
            true,
        ));
        assert!(s.validate_program("evil-tool").is_err());
        assert!(s.validate_program("/usr/bin/evil-tool").is_err());
        assert!(s.checked_command("evil-tool", &[], &env_pair()).is_err());
    }
    #[test]
    fn allowed_program_passes() {
        let s = SecureSpawner::new(SpawnPolicy::restrictive(vec![
            "git".to_owned(),
            "cargo".to_owned(),
        ]));
        assert!(s.validate_program("git").is_ok());
        assert!(s.validate_program("cargo").is_ok());
        assert!(s.validate_program("/usr/bin/git").is_ok());
        assert!(s.validate_program("evil").is_err());
    }
    #[test]
    fn system_binary_blocked() {
        assert!(validate_program("/sbin/iptables").is_err());
        assert!(validate_program("/usr/sbin/sshd").is_err());
        assert!(validate_program("/sbin").is_err());
        let s = SecureSpawner::new(SpawnPolicy::default());
        assert!(s.validate_program("/sbin/iptables").is_err());
        assert!(s.validate_program("/usr/sbin/sshd").is_err());
        assert!(s.validate_program("/usr/bin/git").is_ok());
    }
    #[test]
    fn custom_policy_works() {
        let open = SecureSpawner::new(SpawnPolicy::new(Vec::new(), Vec::new(), false));
        assert_eq!(open.filter_env(&env_pair()).len(), env_pair().len());
        let strict = SecureSpawner::new(SpawnPolicy::new(Vec::new(), Vec::new(), true));
        assert!(strict.filter_env(&env_pair()).len() < env_pair().len());
        let conflict = SecureSpawner::new(SpawnPolicy::new(
            vec!["tool".to_owned()],
            vec!["tool".to_owned()],
            true,
        ));
        assert!(conflict.validate_program("tool").is_err());
    }
}
