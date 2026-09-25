#![forbid(unsafe_code)]
//! Full workspace label (workspace-label.tsx:5-18 `{name} ({type})`).
//! Tilde-shorten + char caps; std-only.

/// Full label bound.
pub const MAX_LABEL: usize = 128;
/// Short (basename) bound.
pub const MAX_SHORT: usize = 64;

fn cap(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect()
    }
}

fn norm(p: &str) -> &str {
    let t = p.trim_end_matches(['/', '\\']);
    if t.is_empty() {
        "/"
    } else {
        t
    }
}

/// Tilde-shorten `path` under `home`, cap 128 chars.
#[must_use]
pub fn workspace_label(path: &str, home: &str) -> String {
    let t = norm(path);
    let h = norm(home);
    let s = if t == h {
        "~".to_string()
    } else if !h.is_empty() && t.starts_with(&format!("{h}/")) {
        format!("~{}", &t[h.len()..])
    } else {
        t.to_string()
    };
    cap(&s, MAX_LABEL)
}

/// Basename of `path`, cap 64 chars.
#[must_use]
pub fn workspace_short(path: &str) -> String {
    let n = norm(path);
    if n == "/" {
        return "/".to_string();
    }
    let b = n.rsplit(['/', '\\']).next().unwrap_or(n);
    cap(b, MAX_SHORT)
}

/// True iff `path` and `cwd` match modulo trailing slashes.
#[must_use]
pub fn is_current(path: &str, cwd: &str) -> bool {
    norm(path) == norm(cwd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_home_itself() {
        assert_eq!(workspace_label("/home/u", "/home/u"), "~");
    }

    #[test]
    fn label_subpath_tilde() {
        assert_eq!(workspace_label("/home/u/proj", "/home/u"), "~/proj");
    }

    #[test]
    fn label_outside_and_cap() {
        assert_eq!(workspace_label("/x/y", "/home/u"), "/x/y");
        assert_eq!(workspace_label(&"a".repeat(200), "/home/u").len(), 128);
    }

    #[test]
    fn short_basename_and_cap() {
        assert_eq!(workspace_short("/a/b/c"), "c");
        assert_eq!(workspace_short("/a/b/"), "b");
        assert_eq!(workspace_short("/"), "/");
        assert_eq!(
            workspace_short(&format!("/a/{}", "b".repeat(100))).len(),
            64
        );
    }

    #[test]
    fn current_matches_slashes() {
        assert!(is_current("/a/b", "/a/b/"));
        assert!(!is_current("/a/b", "/a/c"));
    }
}
