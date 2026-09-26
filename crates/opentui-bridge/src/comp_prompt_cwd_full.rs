#![forbid(unsafe_code)]
//! Prompt cwd label (`component/prompt/cwd.ts` empty;
//! logic `runtime.tsx:3` + `context/directory.ts:13` + `sidebar/footer.tsx:22-30`).

/// Tilde label bound.
pub const MAX_LABEL: usize = 128;
/// Basename bound.
pub const MAX_BASE: usize = 64;

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

/// Tilde-shorten `cwd` under `home`, cap 128 chars.
#[must_use]
pub fn cwd_label(cwd: &str, home: &str) -> String {
    let t = norm(cwd);
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

/// Basename of `cwd`, cap 64 chars.
#[must_use]
pub fn cwd_base(cwd: &str) -> String {
    let n = norm(cwd);
    if n == "/" {
        return "/".to_string();
    }
    let b = n.rsplit(['/', '\\']).next().unwrap_or(n);
    cap(b, MAX_BASE)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn label_home_itself() {
        assert_eq!(cwd_label("/home/u", "/home/u"), "~");
    }
    #[test]
    fn label_sub_outside_cap() {
        assert_eq!(cwd_label("/home/u/p", "/home/u"), "~/p");
        assert_eq!(cwd_label("/x/y", "/home/u"), "/x/y");
        assert_eq!(cwd_label(&"a".repeat(200), "/home/u").len(), 128);
    }
    #[test]
    fn base_split_cap() {
        assert_eq!(cwd_base("/a/b/c"), "c");
        assert_eq!(cwd_base("/a/b/"), "b");
        assert_eq!(cwd_base("/"), "/");
        assert_eq!(cwd_base(&format!("/a/{}", "b".repeat(100))).len(), 64);
    }
}
