#![forbid(unsafe_code)]
//! Revert-diff display helpers (port of `packages/tui/src/util/revert-diff.ts`).
//! Caps fail closed: stat counts saturate at 64, paths truncate at 256 chars.

/// Max count shown by [`diff_stat`].
pub const MAX_STAT: usize = 64;
/// Max path chars kept by [`revert_line`].
pub const MAX_REVERT_PATH: usize = 256;

/// Format `"+A -R"`, each count capped at 64.
#[must_use]
pub fn diff_stat(added: usize, removed: usize) -> String {
    format!("+{} -{}", added.min(MAX_STAT), removed.min(MAX_STAT))
}

/// True when there is anything to revert.
#[must_use]
pub fn is_revertable(added: usize, removed: usize) -> bool {
    added > 0 || removed > 0
}

/// Format `"revert {path}"`, path truncated to 256 chars.
#[must_use]
pub fn revert_line(path: &str) -> String {
    format!(
        "revert {}",
        path.chars().take(MAX_REVERT_PATH).collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_formats() {
        assert_eq!(diff_stat(2, 1), "+2 -1");
    }

    #[test]
    fn stat_caps() {
        assert_eq!(diff_stat(99, 1000), "+64 -64");
    }

    #[test]
    fn revertable_iff_nonzero() {
        assert!(!is_revertable(0, 0));
        assert!(is_revertable(1, 0));
        assert!(is_revertable(0, 1));
    }

    #[test]
    fn line_prefix_and_cap() {
        assert_eq!(revert_line("a.ts"), "revert a.ts");
        let s = revert_line(&"x".repeat(300));
        assert_eq!(s.len(), 7 + MAX_REVERT_PATH);
        assert!(s.starts_with("revert "));
    }
}
