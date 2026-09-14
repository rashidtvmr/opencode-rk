//! Declarative path and command allowlist with longest-match prefix evaluation (SEC-019).
//! Rules are evaluated by longest matching prefix. An empty rule set defaults to Deny.

use serde::{Deserialize, Serialize};

/// Effect applied when a rule's prefix matches the evaluated input.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyEffect {
    Allow,
    Deny,
    Audit,
}

/// A single prefix rule: applies to any path_or_cmd starting with `pattern`.
/// Longer patterns take precedence, so a more specific rule can narrow a
/// broader parent rule (e.g. `/home` Allow, `/home/secret` Deny).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolicyRule {
    pub pattern: String,
    pub effect: PolicyEffect,
}

impl PolicyRule {
    #[must_use]
    pub fn new(pattern: impl Into<String>, effect: PolicyEffect) -> Self {
        Self {
            pattern: pattern.into(),
            effect,
        }
    }
}

/// Declarative exec policy with longest-match prefix evaluation.
/// Default (empty) policy denies all.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecPolicy {
    pub rules: Vec<PolicyRule>,
}

impl ExecPolicy {
    #[must_use]
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Evaluate a path or command string against the rule set.
    ///
    /// A rule matches when `path_or_cmd` starts with the rule's `pattern`.
    /// Among all matching rules, the one with the longest pattern wins.
    /// If no rule matches, returns [`PolicyEffect::Deny`].
    #[must_use]
    pub fn evaluate(&self, path_or_cmd: &str) -> PolicyEffect {
        let mut best: Option<&PolicyRule> = None;
        for rule in &self.rules {
            if path_or_cmd.starts_with(rule.pattern.as_str()) {
                match &best {
                    None => best = Some(rule),
                    Some(current) => {
                        if rule.pattern.len() > current.pattern.len() {
                            best = Some(rule);
                        }
                    }
                }
            }
        }
        best.map_or(PolicyEffect::Deny, |rule| rule.effect)
    }

    /// Add a rule to the policy.
    pub fn add_rule(&mut self, pattern: impl Into<String>, effect: PolicyEffect) {
        self.rules.push(PolicyRule::new(pattern, effect));
    }

    /// Remove the first rule whose `pattern` equals `pattern`.
    /// Returns `true` if a rule was removed, `false` otherwise.
    pub fn remove_rule(&mut self, pattern: &str) -> bool {
        let index = self.rules.iter().position(|r| r.pattern == pattern);
        match index {
            Some(i) => {
                self.rules.remove(i);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longest_match_wins() {
        let mut policy = ExecPolicy::new();
        policy.add_rule("/home", PolicyEffect::Allow);
        policy.add_rule("/home/secret", PolicyEffect::Deny);

        // /home/secret/file starts with /home/secret which is longer than /home
        let result = policy.evaluate("/home/secret/file");
        assert_eq!(result, PolicyEffect::Deny);

        // /home/other matches only /home
        let result = policy.evaluate("/home/other");
        assert_eq!(result, PolicyEffect::Allow);
    }

    #[test]
    fn deny_all_default() {
        let policy = ExecPolicy::new();
        // No rules - everything should deny
        assert_eq!(policy.evaluate("/some/path"), PolicyEffect::Deny);
        assert_eq!(policy.evaluate("rm"), PolicyEffect::Deny);
        assert_eq!(policy.evaluate("any_command"), PolicyEffect::Deny);
    }

    #[test]
    fn explicit_allow_overrides() {
        let mut policy = ExecPolicy::new();
        policy.add_rule("/allowed/path", PolicyEffect::Allow);

        let result = policy.evaluate("/allowed/path/file.txt");
        assert_eq!(result, PolicyEffect::Allow);

        // Non-matching path still denies
        let result = policy.evaluate("/denied/path");
        assert_eq!(result, PolicyEffect::Deny);
    }

    #[test]
    fn remove_rule_works() {
        let mut policy = ExecPolicy::new();
        policy.add_rule("/test/rule", PolicyEffect::Allow);

        // Verify rule exists
        assert!(policy.rules.iter().any(|r| r.pattern == "/test/rule"));

        // Remove the rule
        let removed = policy.remove_rule("/test/rule");
        assert!(removed);

        // Verify rule is gone
        assert!(!policy.rules.iter().any(|r| r.pattern == "/test/rule"));

        // After removal, evaluation denies
        assert_eq!(policy.evaluate("/test/rule"), PolicyEffect::Deny);

        // Removing non-existent pattern returns false
        assert!(!policy.remove_rule("/nonexistent"));
    }

    #[test]
    fn audit_logged() {
        let mut policy = ExecPolicy::new();
        policy.add_rule("/audit/path", PolicyEffect::Audit);

        let result = policy.evaluate("/audit/path/sensitive");
        assert_eq!(result, PolicyEffect::Audit);

        // Non-matching path denies
        let result = policy.evaluate("/other/path");
        assert_eq!(result, PolicyEffect::Deny);
    }
}
