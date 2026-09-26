#![forbid(unsafe_code)]
//! Link label + open helpers (mirrors `link.tsx`).

/// Display label for url: strip scheme, cap at 256 chars.
#[must_use]
pub fn link_label(url: &str) -> String {
    let s = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    s.chars().take(256).collect()
}

/// True when s starts with http:// or https://.
#[must_use]
pub fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// Open command argv: xdg-open + url, capped at 2 entries.
#[must_use]
pub fn open_cmd(url: &str) -> Vec<String> {
    vec!["xdg-open".to_string(), url.to_string()]
        .into_iter()
        .take(2)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_https_scheme() {
        assert_eq!(link_label("https://example.com/a"), "example.com/a");
    }

    #[test]
    fn strips_http_and_caps_256() {
        let long = format!("http://{}", "x".repeat(300));
        assert_eq!(link_label(&long).len(), 256);
    }

    #[test]
    fn is_url_prefix_only() {
        assert!(is_url("https://a.b"));
        assert!(is_url("http://a.b"));
        assert!(!is_url("ftp://a.b"));
        assert!(!is_url("example.com"));
    }

    #[test]
    fn open_cmd_shape() {
        let c = open_cmd("https://example.com");
        assert_eq!(
            c,
            vec!["xdg-open".to_string(), "https://example.com".to_string()]
        );
        assert!(c.len() <= 2);
    }

    #[test]
    fn label_passthrough_no_scheme() {
        assert_eq!(link_label("example.com"), "example.com");
    }
}
