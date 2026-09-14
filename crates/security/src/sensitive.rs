//! Extended sensitive-path detection and environment-variable filtering.
use std::{collections::HashMap, path::Path};
#[derive(Clone, Debug)]
pub struct SensitivePathChecker {
    patterns: Vec<String>,
}
impl SensitivePathChecker {
    #[must_use]
    pub fn new() -> Self {
        Self {
            patterns: vec![
                ".env*".into(),
                ".ssh/".into(),
                ".aws/".into(),
                ".config/gcloud".into(),
                ".gnupg/".into(),
                "id_rsa".into(),
                "id_ed25519".into(),
                ".netrc".into(),
                ".npmrc".into(),
                ".docker/config.json".into(),
                ".kube/config".into(),
                ".git-credentials".into(),
                "*token*".into(),
                "*credentials*".into(),
            ],
        }
    }
    #[must_use]
    pub fn with_patterns(patterns: Vec<String>) -> Self {
        Self { patterns }
    }
    #[must_use]
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }
    #[must_use]
    pub fn is_sensitive(&self, path: impl AsRef<Path>) -> bool {
        let n = path
            .as_ref()
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();
        let base = n.rsplit('/').next().unwrap_or(&n).to_owned();
        self.patterns.iter().any(|raw| {
            let p = raw.to_ascii_lowercase();
            glob_match(&p, &n)
                || glob_match(&p, &base)
                || (!p.contains('*') && n.contains(p.as_str()))
        })
    }
    pub fn add_pattern(&mut self, pattern: impl Into<String>) {
        self.patterns.push(pattern.into());
    }
    pub fn remove_pattern(&mut self, pattern: &str) -> bool {
        let n = self.patterns.len();
        self.patterns.retain(|p| p != pattern);
        self.patterns.len() != n
    }
}
impl Default for SensitivePathChecker {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Clone, Debug)]
pub struct EnvFilter {
    deny: Vec<String>,
}
impl EnvFilter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            deny: vec![
                "*_KEY".into(),
                "*_SECRET".into(),
                "*_TOKEN".into(),
                "*_PASSWORD".into(),
                "*_CREDENTIAL".into(),
                "AWS_*".into(),
                "OPENAI_*".into(),
                "ANTHROPIC_*".into(),
                "GITHUB_TOKEN".into(),
                "DATABASE_URL".into(),
            ],
        }
    }
    #[must_use]
    pub fn denies(&self) -> &[String] {
        &self.deny
    }
    #[must_use]
    pub fn is_denied(&self, name: &str) -> bool {
        let u = name.to_ascii_uppercase();
        self.deny
            .iter()
            .any(|raw| glob_match(&raw.to_ascii_uppercase(), &u))
    }
    #[must_use]
    pub fn filter(&self, env: &HashMap<String, String>) -> HashMap<String, String> {
        env.iter()
            .filter(|(k, _)| !self.is_denied(k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
    pub fn add_deny(&mut self, pattern: impl Into<String>) {
        self.deny.push(pattern.into());
    }
    pub fn remove_deny(&mut self, pattern: &str) -> bool {
        let n = self.deny.len();
        self.deny.retain(|p| p != pattern);
        self.deny.len() != n
    }
}
impl Default for EnvFilter {
    fn default() -> Self {
        Self::new()
    }
}
#[must_use]
pub fn glob_match(pattern: &str, text: &str) -> bool {
    glob(pattern.as_bytes(), text.as_bytes())
}
fn glob(p: &[u8], t: &[u8]) -> bool {
    match p.split_first() {
        None => t.is_empty(),
        Some((b'*', rest)) => {
            let mut r = rest;
            while r.first() == Some(&b'*') {
                r = &r[1..];
            }
            (0..=t.len()).any(|i| glob(r, &t[i..]))
        }
        Some((c, rest)) => !t.is_empty() && t[0] == *c && glob(rest, &t[1..]),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn checker() -> SensitivePathChecker {
        SensitivePathChecker::new()
    }
    fn envmap(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }
    #[test]
    fn blocks_dotenv() {
        let c = checker();
        assert!(c.is_sensitive("/work/project/.env"));
        assert!(c.is_sensitive("/work/project/.env.local"));
        assert!(c.is_sensitive("/work/project/.env.production"));
        assert!(!c.is_sensitive("/work/project/src/lib.rs"));
    }
    #[test]
    fn blocks_ssh_keys() {
        let c = checker();
        assert!(c.is_sensitive("/home/user/.ssh/id_rsa"));
        assert!(c.is_sensitive("/home/user/.ssh/id_ed25519"));
        assert!(c.is_sensitive("/home/user/.ssh/known_hosts"));
        let mut m = checker();
        m.add_pattern("*.pem");
        assert!(m.is_sensitive("/home/user/key.pem"));
        assert!(m.remove_pattern("*.pem"));
        assert!(!m.is_sensitive("/home/user/key.pem"));
    }
    #[test]
    fn blocks_cloud_creds() {
        let c = checker();
        assert!(c.is_sensitive("/home/user/.aws/credentials"));
        assert!(c.is_sensitive("/home/user/.aws/config"));
        assert!(c.is_sensitive("/home/user/.config/gcloud/credentials.db"));
        assert!(c.is_sensitive("/home/user/.gnupg/secring.gpg"));
        assert!(c.is_sensitive("/work/project/.docker/config.json"));
        assert!(c.is_sensitive("/home/user/.kube/config"));
        assert!(c.is_sensitive("/home/user/.git-credentials"));
        assert!(c.is_sensitive("/work/svc/api_token.txt"));
        assert!(!c.is_sensitive("/work/project/README.md"));
    }
    #[test]
    fn env_filter_removes_secrets() {
        let f = EnvFilter::new();
        let env = envmap(&[
            ("API_KEY", "x"),
            ("AWS_SECRET_ACCESS_KEY", "x"),
            ("AWS_REGION", "x"),
            ("OPENAI_API_KEY", "x"),
            ("ANTHROPIC_API_KEY", "x"),
            ("GITHUB_TOKEN", "x"),
            ("DATABASE_URL", "postgres://u:p@h/db"),
            ("DB_PASSWORD", "x"),
            ("SVC_CREDENTIAL", "x"),
            ("AUTH_TOKEN", "x"),
        ]);
        let out = f.filter(&env);
        for k in [
            "API_KEY",
            "AWS_SECRET_ACCESS_KEY",
            "AWS_REGION",
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
            "GITHUB_TOKEN",
            "DATABASE_URL",
            "DB_PASSWORD",
            "SVC_CREDENTIAL",
            "AUTH_TOKEN",
        ] {
            assert!(!out.contains_key(k), "leaked {k}");
        }
        assert!(out.is_empty());
    }
    #[test]
    fn env_filter_keeps_safe() {
        let f = EnvFilter::new();
        let env = envmap(&[
            ("PATH", "/usr/bin"),
            ("HOME", "/home/user"),
            ("TERM", "xterm-256color"),
            ("EDITOR", "vim"),
        ]);
        let out = f.filter(&env);
        assert_eq!(out.len(), 4);
        assert_eq!(out.get("PATH").map(String::as_str), Some("/usr/bin"));
        assert_eq!(out.get("HOME").map(String::as_str), Some("/home/user"));
        assert_eq!(out.get("TERM").map(String::as_str), Some("xterm-256color"));
        let mut m = EnvFilter::new();
        m.add_deny("EDITOR");
        assert!(m.filter(&env).get("EDITOR").is_none());
        assert!(m.remove_deny("EDITOR"));
        assert!(m.filter(&env).get("EDITOR").is_some());
    }
}
