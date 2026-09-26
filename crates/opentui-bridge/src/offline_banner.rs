#![forbid(unsafe_code)]
//! Offline stdin banner titles.
//!
//! Evidence: `run_runtime_stdin.rs:40-46` (`probe_label` tty/piped/closed)
//! + `native_frame.rs:19` (`OFFLINE_TITLE` fallback when title is None).
//! Title carries the probe mode so headless runs show why stdin is offline.

/// Max chars for [`offline_title`].
pub const MAX_OFFLINE_TITLE: usize = 256;

/// Offline window title for a probe mode label (e.g. "tty"/"piped"/"closed").
#[must_use]
pub fn offline_title(mode: &str) -> String {
    let full = format!("OpenCode RK -- offline stdin:{mode}");
    if full.chars().count() <= MAX_OFFLINE_TITLE {
        full
    } else {
        full.chars().take(MAX_OFFLINE_TITLE).collect()
    }
}

/// Stable seed string for an offline probe mode label.
#[must_use]
pub fn offline_seed(mode: &str) -> String {
    format!("offline stdin:{mode}")
}

/// True when no live snapshot is bound.
#[must_use]
pub fn is_offline(live: bool) -> bool {
    !live
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_carries_mode() {
        assert_eq!(
            offline_title("closed"),
            "OpenCode RK -- offline stdin:closed"
        );
    }

    #[test]
    fn title_matches_probe_labels() {
        for mode in ["tty", "piped", "closed"] {
            assert!(offline_title(mode).ends_with(mode));
        }
    }

    #[test]
    fn title_capped_at_256_chars() {
        let long = "x".repeat(300);
        let t = offline_title(&long);
        assert_eq!(t.chars().count(), MAX_OFFLINE_TITLE);
        assert!(t.starts_with("OpenCode RK -- offline stdin:"));
    }

    #[test]
    fn seed_format() {
        assert_eq!(offline_seed("piped"), "offline stdin:piped");
    }

    #[test]
    fn live_flag() {
        assert!(is_offline(false));
        assert!(!is_offline(true));
    }
}
