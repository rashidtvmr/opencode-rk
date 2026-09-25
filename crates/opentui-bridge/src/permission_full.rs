#![forbid(unsafe_code)]
//! Single permission decision (mirrors `permission.shared.ts` reply side).
//! Capped tool/args, tri-state decision via allow/deny, pending check.

/// Max chars for [`PermissionFull::tool`].
pub const TOOL_CAP: usize = 64;
/// Max args held.
pub const ARG_CAP: usize = 8;
/// Max chars per arg.
pub const ARG_LEN_CAP: usize = 256;
/// Max chars for [`PermissionFull::summary`].
pub const SUMMARY_CAP: usize = 128;

fn trunc(s: &str, cap: usize) -> String {
    s.chars().take(cap).collect()
}

/// One tool permission with pending/allow/deny decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionFull {
    tool: String,
    args: Vec<String>,
    decision: Option<bool>,
}

impl PermissionFull {
    /// Build with tool cap 64, args truncated to 8, each arg cap 256.
    pub fn new(tool: String, args: Vec<String>) -> Self {
        let mut a: Vec<String> = args.into_iter().map(|s| trunc(&s, ARG_LEN_CAP)).collect();
        a.truncate(ARG_CAP);
        Self {
            tool: trunc(&tool, TOOL_CAP),
            args: a,
            decision: None,
        }
    }

    /// Decide allow.
    pub fn allow(&mut self) {
        self.decision = Some(true);
    }

    /// Decide deny.
    pub fn deny(&mut self) {
        self.decision = Some(false);
    }

    /// True while undecided.
    pub fn pending(&self) -> bool {
        self.decision.is_none()
    }

    /// `"tool allow|deny|pending"`, cap 128 chars.
    pub fn summary(&self) -> String {
        let word = match self.decision {
            Some(true) => "allow",
            Some(false) => "deny",
            None => "pending",
        };
        trunc(&format!("{} {word}", self.tool), SUMMARY_CAP)
    }

    /// Tool name.
    pub fn tool(&self) -> &str {
        &self.tool
    }

    /// Capped args.
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Current decision.
    pub fn decision(&self) -> Option<bool> {
        self.decision
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> PermissionFull {
        PermissionFull::new("bash".to_string(), vec!["ls".to_string()])
    }

    #[test]
    fn default_pending() {
        let x = p();
        assert!(x.pending());
        assert_eq!(x.decision(), None);
        assert_eq!(x.summary(), "bash pending");
    }

    #[test]
    fn allow_sets_true() {
        let mut x = p();
        x.allow();
        assert!(!x.pending());
        assert_eq!(x.decision(), Some(true));
        assert_eq!(x.summary(), "bash allow");
    }

    #[test]
    fn deny_sets_false() {
        let mut x = p();
        x.deny();
        assert!(!x.pending());
        assert_eq!(x.decision(), Some(false));
        assert_eq!(x.summary(), "bash deny");
    }

    #[test]
    fn overwrite_last_wins() {
        let mut x = p();
        x.allow();
        x.deny();
        assert_eq!(x.decision(), Some(false));
        x.allow();
        assert_eq!(x.decision(), Some(true));
    }

    #[test]
    fn trunc_caps() {
        let x = PermissionFull::new(
            "t".repeat(100),
            (0..12)
                .map(|i| format!("a{i}-{}", "y".repeat(300)))
                .collect(),
        );
        assert_eq!(x.tool().len(), TOOL_CAP);
        assert_eq!(x.args().len(), ARG_CAP);
        assert!(x.args().iter().all(|a| a.chars().count() <= ARG_LEN_CAP));
        assert!(x.summary().chars().count() <= SUMMARY_CAP);
    }
}
