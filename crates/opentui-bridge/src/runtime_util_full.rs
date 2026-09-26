#![forbid(unsafe_code)]
//! Home-tilde abbreviate + under-root check (mirrors `runtime.tsx:3-9`).
pub const MAX_HOME_LEN: usize = 256;
#[must_use]
pub fn abbreviate_home(path: &str, home: &str) -> String {
    let home = trim(home);
    if home.is_empty() {
        return path.to_string();
    }
    let out = if path == home {
        "~".to_string()
    } else {
        match strip(path, home) {
            Some(rest) => format!("~/{rest}"),
            None => path.to_string(),
        }
    };
    out.chars().take(MAX_HOME_LEN).collect()
}
#[must_use]
pub fn is_under(path: &str, root: &str) -> bool {
    let root = trim(root);
    if root.is_empty() {
        return false;
    }
    if root == "/" {
        return path.starts_with('/');
    }
    path == root || strip(path, root).is_some()
}
fn trim(s: &str) -> &str {
    if s.len() > 1 {
        s.trim_end_matches('/')
    } else {
        s
    }
}
fn strip<'a>(path: &'a str, base: &str) -> Option<&'a str> {
    path.strip_prefix(base)
        .filter(|r| r.starts_with('/'))
        .map(|r| &r[1..])
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tilde_exact_and_nested() {
        assert_eq!(abbreviate_home("/home/u", "/home/u"), "~");
        assert_eq!(abbreviate_home("/home/u/p/f.rs", "/home/u"), "~/p/f.rs");
        assert_eq!(abbreviate_home("/home/u/", "/home/u/"), "~/");
    }
    #[test]
    fn tilde_passthrough_and_cap() {
        assert_eq!(abbreviate_home("/etc/hosts", "/home/u"), "/etc/hosts");
        assert_eq!(abbreviate_home("/home/u2/x", "/home/u"), "/home/u2/x");
        assert_eq!(abbreviate_home("/a", ""), "/a");
        let long = format!("/home/u/{}", "a".repeat(400));
        assert_eq!(abbreviate_home(&long, "/home/u").chars().count(), 256);
    }
    #[test]
    fn under_root() {
        assert!(is_under("/a/b", "/a"));
        assert!(is_under("/a", "/a"));
        assert!(!is_under("/a2/x", "/a"));
        assert!(!is_under("/etc/hosts", "/home/u"));
        assert!(!is_under("/a", ""));
        assert!(is_under("/a", "/"));
    }
}
