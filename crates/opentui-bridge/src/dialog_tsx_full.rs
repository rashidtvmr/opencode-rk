#![forbid(unsafe_code)]
//! Dialog shell helpers mirroring `packages/tui/src/ui/dialog.tsx:11` `Dialog`.
//! Width by size, text-selection dismiss flag, title trim/cap.

pub fn dialog_width(size: &str) -> u32 {
    match size {
        "xlarge" => 116,
        "large" => 88,
        _ => 60,
    }
}

/// Mirrors `dismiss = !!renderer.getSelection()`: true means swallow mouse-up.
pub fn is_dismissed(flag: bool) -> bool {
    flag
}

/// Trimmed title capped at 128 chars.
pub fn dialog_title(t: &str) -> String {
    t.trim().chars().take(128).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xlarge_width() {
        assert_eq!(dialog_width("xlarge"), 116);
    }

    #[test]
    fn large_width() {
        assert_eq!(dialog_width("large"), 88);
    }

    #[test]
    fn default_width() {
        assert_eq!(dialog_width("medium"), 60);
        assert_eq!(dialog_width(""), 60);
    }

    #[test]
    fn dismiss_passthrough() {
        assert!(is_dismissed(true));
        assert!(!is_dismissed(false));
    }

    #[test]
    fn title_trims_and_caps() {
        assert_eq!(dialog_title("  hi  "), "hi");
        assert_eq!(dialog_title(&"a".repeat(200)).len(), 128);
    }
}
