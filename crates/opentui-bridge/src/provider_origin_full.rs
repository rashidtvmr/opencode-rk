#![forbid(unsafe_code)]
//! Provider origin label helpers.
//! Mirrors `packages/tui/src/util/provider-origin.ts` display intent:
//! empty origin renders as local; remote origins show host.

/// Max display chars for origin labels.
pub const MAX_ORIGIN_DISPLAY: usize = 64;

fn truncate(s: &str) -> String {
    if s.chars().count() <= MAX_ORIGIN_DISPLAY {
        s.to_string()
    } else {
        s.chars().take(MAX_ORIGIN_DISPLAY).collect()
    }
}

/// Display label; empty/blank -> "local", else capped at 64 chars.
#[must_use]
pub fn origin_label(origin: &str) -> String {
    let t = origin.trim();
    if t.is_empty() {
        return "local".to_string();
    }
    truncate(t)
}

/// True when origin looks remote (`scheme://...`).
#[must_use]
pub fn is_remote(origin: &str) -> bool {
    let t = origin.trim();
    match t.find("://") {
        Some(i) => i > 0,
        None => false,
    }
}

/// Host part after `://` (cut at `/` `?` `#`), or whole trimmed origin; capped.
#[must_use]
pub fn short_origin(origin: &str) -> String {
    let t = origin.trim();
    if t.is_empty() {
        return "local".to_string();
    }
    let host = match t.find("://") {
        Some(i) => &t[i + 3..],
        None => t,
    };
    let end = host.find(['/', '?', '#']).unwrap_or(host.len());
    truncate(&host[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_maps_to_local() {
        assert_eq!(origin_label(""), "local");
        assert_eq!(origin_label("   "), "local");
        assert_eq!(short_origin(""), "local");
    }

    #[test]
    fn remote_requires_scheme() {
        assert!(is_remote("https://example.com"));
        assert!(is_remote("http://localhost:8080/x"));
        assert!(!is_remote(""));
        assert!(!is_remote("local"));
        assert!(!is_remote("example.com"));
    }

    #[test]
    fn short_strips_scheme_path() {
        assert_eq!(short_origin("https://example.com/a?b=1"), "example.com");
        assert_eq!(short_origin("http://localhost:8080/x"), "localhost:8080");
        assert_eq!(short_origin("example.com"), "example.com");
    }

    #[test]
    fn labels_cap_at_64() {
        let long = "x".repeat(100);
        assert_eq!(origin_label(&long).chars().count(), 64);
        let url = format!("https://{}.com/y", "z".repeat(100));
        assert!(short_origin(&url).chars().count() <= 64);
    }
}
