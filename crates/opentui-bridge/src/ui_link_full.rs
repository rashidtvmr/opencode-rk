#![forbid(unsafe_code)]
//! Link text helpers (mirrors `link.tsx` displayText fallback).

/// Trimmed url, capped at 256 chars.
#[must_use]
pub fn link_text(url: &str) -> String {
    url.trim().chars().take(256).collect()
}

/// True when url starts with http:// or https://.
#[must_use]
pub fn is_external(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

/// Trimmed text, or trimmed url fallback; capped at 256 chars.
#[must_use]
pub fn link_label(url: &str, text: &str) -> String {
    let t = text.trim();
    if t.is_empty() {
        link_text(url)
    } else {
        t.chars().take(256).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_trims_and_caps() {
        assert_eq!(link_text("  https://a.b  "), "https://a.b");
        assert_eq!(link_text(&"x".repeat(300)).len(), 256);
    }

    #[test]
    fn external_prefix_only() {
        assert!(is_external("https://a.b"));
        assert!(is_external("http://a.b"));
        assert!(!is_external("ftp://a.b"));
        assert!(!is_external("example.com"));
    }

    #[test]
    fn label_prefers_text() {
        assert_eq!(link_label("https://a.b", "hi"), "hi");
        assert_eq!(link_label("https://a.b", "  hi  "), "hi");
    }

    #[test]
    fn label_falls_back_and_caps() {
        assert_eq!(link_label("https://a.b", ""), "https://a.b");
        assert_eq!(link_label("https://a.b", "   "), "https://a.b");
        assert_eq!(link_label("u", &"y".repeat(300)).len(), 256);
    }
}
