//! OS-level environment variable restrictions for child processes (SEC-014).
//! Controls which environment variables are inherited by spawned processes.
use std::collections::HashMap;

/// Policy for inheriting environment variables in child processes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InheritPolicy {
    /// Inherit all environment variables unchanged.
    InheritAll,
    /// Inherit no environment variables (empty environment).
    InheritNone,
    /// Inherit only variables in the allowlist.
    InheritAllowlist(Vec<String>),
}

impl Default for InheritPolicy {
    fn default() -> Self {
        InheritPolicy::InheritAll
    }
}

/// Default blocklist patterns for sensitive environment variables.
pub static DEFAULT_BLOCKLIST: &[&str] = &[
    "*_KEY",
    "*_SECRET",
    "*_TOKEN",
    "*_PASSWORD",
    "AWS_*",
    "OPENAI_*",
    "ANTHROPIC_*",
];

/// Environment restriction configuration for controlling child process env inheritance.
#[derive(Clone, Debug, Default)]
pub struct EnvRestriction {
    pub policy: InheritPolicy,
    pub blocklist: Vec<String>,
}

impl EnvRestriction {
    /// Creates a new EnvRestriction with the given policy and default blocklist.
    #[must_use]
    pub fn new(policy: InheritPolicy) -> Self {
        Self {
            policy,
            blocklist: DEFAULT_BLOCKLIST.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Creates a restriction that inherits all environment variables.
    #[must_use]
    pub fn inherit_all() -> Self {
        Self::new(InheritPolicy::InheritAll)
    }

    /// Creates a restriction that inherits no environment variables.
    #[must_use]
    pub fn inherit_none() -> Self {
        Self::new(InheritPolicy::InheritNone)
    }

    /// Creates a restriction that only allows specific variables.
    #[must_use]
    pub fn allowlist(allowlist: Vec<String>) -> Self {
        Self::new(InheritPolicy::InheritAllowlist(allowlist))
    }

    /// Adds a custom blocklist pattern.
    pub fn add_blocklist(&mut self, pattern: impl Into<String>) {
        self.blocklist.push(pattern.into());
    }

    /// Removes a blocklist pattern. Returns true if the pattern was present.
    pub fn remove_blocklist(&mut self, pattern: &str) -> bool {
        let len = self.blocklist.len();
        self.blocklist.retain(|p| p != pattern);
        self.blocklist.len() != len
    }

    /// Applies the restriction policy to the current environment, returning a filtered copy.
    #[must_use]
    pub fn apply(&self, current_env: &HashMap<String, String>) -> HashMap<String, String> {
        match &self.policy {
            InheritPolicy::InheritAll => current_env
                .iter()
                .filter(|(k, _)| !self.is_blocked(k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            InheritPolicy::InheritNone => HashMap::new(),
            InheritPolicy::InheritAllowlist(allowlist) => current_env
                .iter()
                .filter(|(k, _)| allowlist.contains(k))
                .filter(|(k, _)| !self.is_blocked(k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        }
    }

    /// Checks if a variable name matches any blocklist pattern.
    #[must_use]
    pub fn is_blocked(&self, name: &str) -> bool {
        let upper = name.to_ascii_uppercase();
        self.blocklist
            .iter()
            .any(|pattern| glob_match(pattern.as_str(), &upper))
    }
}

/// Applies the default env filtering (blocks secrets).
#[must_use]
pub fn filter_env(env: &HashMap<String, String>) -> HashMap<String, String> {
    env.iter()
        .filter(|(k, _)| !is_secret_key(k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Checks if an environment variable key should be blocked.
#[must_use]
pub fn is_secret_key(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    let patterns = [
        "*_KEY",
        "*_SECRET",
        "*_TOKEN",
        "*_PASSWORD",
        "AWS_*",
        "OPENAI_*",
        "ANTHROPIC_*",
    ];
    patterns.iter().any(|p| glob_match(p, &upper))
}

/// Simple glob pattern matching (supports * wildcard).
#[must_use]
pub fn glob_match(pattern: &str, text: &str) -> bool {
    glob(pattern.as_bytes(), text.as_bytes())
}

fn glob(pat: &[u8], txt: &[u8]) -> bool {
    match pat.split_first() {
        None => txt.is_empty(),
        Some((b'*', rest)) => {
            let mut r = rest;
            while r.first() == Some(&b'*') {
                r = &r[1..];
            }
            (0..=txt.len()).any(|i| glob(r, &txt[i..]))
        }
        Some((b'?', rest)) => !txt.is_empty() && txt[0] != b'*' && glob(rest, &txt[1..]),
        Some((c, rest)) => !txt.is_empty() && txt[0] == *c && glob(rest, &txt[1..]),
    }
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
            ("AWS_ACCESS_KEY", "aws-key"),
            ("ANTHROPIC_API_KEY", "anthropic-key"),
            ("CARGO_HOME", "/home/dev/.cargo"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
    }

    #[test]
    fn inherit_none_empty() {
        let restriction = EnvRestriction::inherit_none();
        let result = restriction.apply(&env_pair());
        assert!(result.is_empty(), "InheritNone should produce empty map");
    }

    #[test]
    fn inherit_all_passes() {
        let restriction = EnvRestriction::inherit_all();
        let result = restriction.apply(&env_pair());
        assert!(
            !result.contains_key("OPENAI_API_KEY"),
            "OPENAI_API_KEY should be blocked even with InheritAll"
        );
        assert!(
            !result.contains_key("GITHUB_TOKEN"),
            "GITHUB_TOKEN should be blocked even with InheritAll"
        );
        assert!(
            !result.contains_key("DB_PASSWORD"),
            "DB_PASSWORD should be blocked even with InheritAll"
        );
        assert!(
            !result.contains_key("AWS_ACCESS_KEY"),
            "AWS_ACCESS_KEY should be blocked even with InheritAll"
        );
        assert_eq!(result.get("HOME"), Some(&"/home/dev".to_string()));
        assert_eq!(result.get("PATH"), Some(&"/usr/bin:/bin".to_string()));
        assert_eq!(
            result.get("CARGO_HOME"),
            Some(&"/home/dev/.cargo".to_string())
        );
    }

    #[test]
    fn allowlist_filters() {
        let allowlist = vec![
            "PATH".to_string(),
            "HOME".to_string(),
            "CARGO_HOME".to_string(),
        ];
        let restriction = EnvRestriction::allowlist(allowlist);
        let result = restriction.apply(&env_pair());
        assert!(
            result.contains_key("PATH"),
            "PATH should be in allowlist result"
        );
        assert!(
            result.contains_key("HOME"),
            "HOME should be in allowlist result"
        );
        assert!(
            result.contains_key("CARGO_HOME"),
            "CARGO_HOME should be in allowlist result"
        );
        assert!(
            !result.contains_key("OPENAI_API_KEY"),
            "OPENAI_API_KEY should not be in result"
        );
        assert!(
            !result.contains_key("GITHUB_TOKEN"),
            "GITHUB_TOKEN should not be in result"
        );
    }

    #[test]
    fn default_blocks_secrets() {
        let restriction = EnvRestriction::inherit_all();
        let result = restriction.apply(&env_pair());
        assert!(
            !result.contains_key("OPENAI_API_KEY"),
            "OPENAI_API_KEY should be blocked by default blocklist"
        );
        assert!(
            !result.contains_key("GITHUB_TOKEN"),
            "GITHUB_TOKEN should be blocked by default blocklist"
        );
        assert!(
            !result.contains_key("DB_PASSWORD"),
            "DB_PASSWORD should be blocked by default blocklist"
        );
        assert!(
            !result.contains_key("AWS_ACCESS_KEY"),
            "AWS_ACCESS_KEY should be blocked by default blocklist"
        );
        assert!(
            !result.contains_key("ANTHROPIC_API_KEY"),
            "ANTHROPIC_API_KEY should be blocked by default blocklist"
        );
        assert!(result.contains_key("HOME"), "HOME should pass through");
        assert!(result.contains_key("PATH"), "PATH should pass through");
    }

    #[test]
    fn custom_blocklist() {
        let mut restriction = EnvRestriction::inherit_all();
        restriction.add_blocklist("CUSTOM_*".to_string());
        restriction.add_blocklist("SECRET_VAR".to_string());

        let mut custom_env = env_pair();
        custom_env.insert("CUSTOM_API_KEY".to_string(), "custom-secret".to_string());
        custom_env.insert("SECRET_VAR".to_string(), "secret-value".to_string());

        let result = restriction.apply(&custom_env);

        assert!(
            !result.contains_key("OPENAI_API_KEY"),
            "Standard keys still blocked"
        );
        assert!(
            !result.contains_key("CUSTOM_API_KEY"),
            "CUSTOM_API_KEY should be blocked by custom pattern"
        );
        assert!(
            !result.contains_key("SECRET_VAR"),
            "SECRET_VAR should be blocked by explicit entry"
        );
        assert!(result.contains_key("HOME"), "HOME should pass through");
    }
}
