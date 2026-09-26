#![forbid(unsafe_code)]
//! Home detail tabs + provider label (`packages/tui/src/routes/home.tsx:1-95`).
//! TS `Home:22` prompt-first; `session-destination` `Provider:23` label;
//! `NativePage` vs `Page` unmapped (Rust keeps tab index only).

/// Home tabs in display order.
pub const HOME_TABS: [&str; 2] = ["recent", "new"];
/// Max chars in a provider label.
pub const PROVIDER_LABEL_MAX: usize = 64;

/// Clip provider label to 64 chars (char-safe).
#[must_use]
pub fn trunc_provider(s: &str) -> String {
    s.chars().take(PROVIDER_LABEL_MAX).collect()
}

/// Tab at `i`; fail-closed to `"recent"` when out of range.
#[must_use]
pub fn tab_at(i: usize) -> &'static str {
    HOME_TABS.get(i).copied().unwrap_or(HOME_TABS[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_order() {
        assert_eq!(HOME_TABS, ["recent", "new"]);
    }

    #[test]
    fn tab_at_valid() {
        assert_eq!(tab_at(0), "recent");
        assert_eq!(tab_at(1), "new");
    }

    #[test]
    fn tab_at_oob_fail_closed() {
        assert_eq!(tab_at(2), "recent");
        assert_eq!(tab_at(usize::MAX), "recent");
    }

    #[test]
    fn trunc_clips_ascii() {
        assert_eq!(trunc_provider("a"), "a");
        assert_eq!(trunc_provider(&"x".repeat(65)).len(), 64);
    }

    #[test]
    fn trunc_char_safe() {
        let s = format!("{}{}", "e".repeat(63), "☃ab");
        assert_eq!(trunc_provider(&s).chars().count(), PROVIDER_LABEL_MAX);
    }
}
