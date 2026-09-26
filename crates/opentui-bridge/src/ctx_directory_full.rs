#![forbid(unsafe_code)]
//! Directory label: `~` abbreviate + `:branch` suffix (directory.ts:7-17).
pub const MAX_LABEL: usize = 128;
pub const MAX_WITH_BRANCH: usize = 192;
#[must_use]
pub fn dir_label(dir: &str, home: &str) -> String {
    take(&abbrev(dir, home), MAX_LABEL)
}
#[must_use]
pub fn dir_with_branch(dir: &str, branch: &str) -> String {
    if dir.is_empty() {
        return String::new();
    }
    let s = if branch.is_empty() {
        dir.to_string()
    } else {
        format!("{dir}:{branch}")
    };
    take(&s, MAX_WITH_BRANCH)
}
fn abbrev(dir: &str, home: &str) -> String {
    if dir.is_empty() || home.is_empty() {
        return dir.to_string();
    }
    let h = if home.len() > 1 {
        home.trim_end_matches('/')
    } else {
        home
    };
    let d = if dir.len() > 1 {
        dir.trim_end_matches('/')
    } else {
        dir
    };
    if d == h {
        return "~".into();
    }
    if h == "/" {
        return if d.starts_with('/') {
            format!("~{d}")
        } else {
            dir.to_string()
        };
    }
    match d.strip_prefix(h).filter(|r| r.starts_with('/')) {
        Some(r) => format!("~{r}"),
        None => dir.to_string(),
    }
}
fn take(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tilde() {
        assert_eq!(dir_label("/home/u/p", "/home/u"), "~/p");
        assert_eq!(dir_label("/home/u", "/home/u"), "~");
    }
    #[test]
    fn passthrough() {
        assert_eq!(dir_label("/etc/h", "/home/u"), "/etc/h");
        assert_eq!(dir_label("/home/u2/x", "/home/u"), "/home/u2/x");
    }
    #[test]
    fn branch() {
        assert_eq!(dir_with_branch("~/p", "main"), "~/p:main");
        assert_eq!(dir_with_branch("~/p", ""), "~/p");
    }
    #[test]
    fn caps() {
        assert_eq!(dir_label(&"a".repeat(200), "/h").chars().count(), 128);
        assert_eq!(dir_with_branch(&"a".repeat(200), "b").chars().count(), 192);
    }
}
