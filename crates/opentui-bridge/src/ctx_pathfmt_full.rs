#![forbid(unsafe_code)]
//! Tilde path + basename (mirrors `path-format.tsx` + `runtime:abbreviateHome`).
pub const MAX_FMT_LEN: usize = 128;
pub const MAX_BASE_LEN: usize = 64;
/// `~` for home, `~/rest` below it, else as-is; cap 128 chars.
#[must_use]
pub fn fmt_path(path: &str, home: &str) -> String {
    let home = trim(home);
    let shown = if path == home && !home.is_empty() {
        "~".to_string()
    } else if let Some(rest) = strip(path, home) {
        format!("~/{rest}")
    } else {
        path.to_string()
    };
    shown.chars().take(MAX_FMT_LEN).collect()
}
/// Final component after last `/`; cap 64 chars.
#[must_use]
pub fn base_of(path: &str) -> String {
    let t = path.trim_end_matches('/');
    let b = if t.is_empty() {
        if path.is_empty() {
            ""
        } else {
            "/"
        }
    } else {
        t.rsplit('/').next().unwrap_or(t)
    };
    b.chars().take(MAX_BASE_LEN).collect()
}
fn trim(s: &str) -> &str {
    if s.len() > 1 {
        s.trim_end_matches('/')
    } else {
        s
    }
}
fn strip<'a>(path: &'a str, home: &str) -> Option<&'a str> {
    if home == "/" && path.starts_with('/') {
        return Some(path.trim_start_matches('/'));
    }
    if home.is_empty() || !path.starts_with(home) {
        return None;
    }
    path.strip_prefix(home)
        .filter(|r| r.starts_with('/'))
        .map(|r| &r[1..])
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tilde_home() {
        assert_eq!(fmt_path("/home/u/a", "/home/u"), "~/a");
        assert_eq!(fmt_path("/home/u", "/home/u"), "~");
    }
    #[test]
    fn no_false_prefix() {
        assert_eq!(fmt_path("/home/u2/x", "/home/u"), "/home/u2/x");
        assert_eq!(fmt_path("/etc/hosts", "/home/u"), "/etc/hosts");
    }
    #[test]
    fn base_and_caps() {
        assert_eq!(base_of("/a/b/c.rs"), "c.rs");
        assert_eq!(base_of("/"), "/");
        assert!(fmt_path(&"a".repeat(200), "/h").chars().count() <= 128);
        assert!(base_of(&"a".repeat(100)).chars().count() <= 64);
    }
}
