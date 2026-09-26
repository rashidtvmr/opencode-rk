#![forbid(unsafe_code)]
//! Tri-state permission reply: `permission` (once), `always`, `reject`.
//!
//! Mirrors `Stage` (`Ask` = pending `permission` prompt, `Always`, `Reject`)
//! and the `EditBody` reply shape in `run/permission.shared.ts:22`.

/// Reply to a permission prompt: allow once, allow always, or reject.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum PermDecision {
    Allow,
    Always,
    Reject,
}

/// Tool permission kinds covered (mirrors the 12 `PermissionKind`s).
pub const KIND_COUNT: usize = 12;

/// Parse a reply name; fail-closed: unknown maps to `Reject`.
pub fn decide_name(name: &str) -> PermDecision {
    match name.trim().to_ascii_lowercase().as_str() {
        "allow" | "once" | "permission" => PermDecision::Allow,
        "always" => PermDecision::Always,
        _ => PermDecision::Reject,
    }
}

/// Initial prompt state allows the action (answered, not persisted).
pub fn initial_is_allow() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_names_parse() {
        assert_eq!(decide_name("allow"), PermDecision::Allow);
        assert_eq!(decide_name("permission"), PermDecision::Allow);
        assert_eq!(decide_name(" Allow "), PermDecision::Allow);
    }

    #[test]
    fn always_parses() {
        assert_eq!(decide_name("always"), PermDecision::Always);
        assert_eq!(decide_name("ALWAYS"), PermDecision::Always);
    }

    #[test]
    fn unknown_rejects_fail_closed() {
        for n in ["", "reject", "bogus", "rm -rf", "yes"] {
            assert_eq!(decide_name(n), PermDecision::Reject, "{n:?}");
        }
    }

    #[test]
    fn initial_allows_and_count_twelve() {
        assert!(initial_is_allow());
        assert_eq!(KIND_COUNT, 12);
    }
}
