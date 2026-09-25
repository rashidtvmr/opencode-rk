#![forbid(unsafe_code)]
//! Unified stream commit.
//!
//! Reconciles duplicate `StreamCommit`: `run_types.rs` (id/text, 1024-byte
//! validated) vs `run_stream.rs` (c-<n>/text, 64 KiB fail-closed buffer).
//! `UnifiedCommit` is the single role/body/final shape callers convert to.

/// Max role chars.
pub const MAX_ROLE_CHARS: usize = 32;
/// Max body chars (64 KiB).
pub const MAX_BODY_CHARS: usize = 64 * 1024;
/// Max rendered line chars.
pub const MAX_LINE_CHARS: usize = 512;

/// Single unified commit over both `StreamCommit` dups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnifiedCommit {
    pub role: String,
    pub body: String,
    final_: bool,
}

fn cap(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        s.chars().take(max).collect()
    }
}

impl UnifiedCommit {
    #[must_use]
    pub fn from_either(role: &str, body: &str, final_: bool) -> Self {
        Self {
            role: cap(role, MAX_ROLE_CHARS),
            body: cap(body, MAX_BODY_CHARS),
            final_,
        }
    }

    #[must_use]
    pub fn line(&self) -> String {
        cap(&format!("{}: {}", self.role, self.body), MAX_LINE_CHARS)
    }

    #[must_use]
    pub fn is_final(&self) -> bool {
        self.final_
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_role_at_32_chars() {
        let c = UnifiedCommit::from_either(&"r".repeat(40), "b", false);
        assert_eq!(c.role.chars().count(), MAX_ROLE_CHARS);
    }

    #[test]
    fn caps_body_at_64k_chars() {
        let c = UnifiedCommit::from_either("r", &"b".repeat(MAX_BODY_CHARS + 1), false);
        assert_eq!(c.body.chars().count(), MAX_BODY_CHARS);
    }

    #[test]
    fn line_caps_at_512_chars() {
        let c = UnifiedCommit::from_either("role", &"x".repeat(600), false);
        assert!(c.line().chars().count() <= MAX_LINE_CHARS);
        assert!(c.line().starts_with("role: "));
    }

    #[test]
    fn final_flag_roundtrips() {
        assert!(UnifiedCommit::from_either("r", "b", true).is_final());
        assert!(!UnifiedCommit::from_either("r", "b", false).is_final());
    }

    #[test]
    fn short_values_kept_verbatim() {
        let c = UnifiedCommit::from_either("a", "b", false);
        assert_eq!(c.role, "a");
        assert_eq!(c.body, "b");
        assert_eq!(c.line(), "a: b");
    }
}
