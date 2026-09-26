#![forbid(unsafe_code)]
//! Home-tilde short display, basename, parent (extends `path_format_ctx` idiom).

/// Cap on `short_path` chars.
pub const MAX_SHORT_LEN: usize = 256;
/// Replace `home` prefix with `~` (boundary-safe), cap 256 chars.
#[must_use]
pub fn short_path(path: &str, home: &str) -> String {
    let home = trim_end(home);
    if path == home && !home.is_empty() {
        return "~".into();
    }
    let shown = match strip_home(path, home) {
        Some(rest) => format!("~/{rest}"),
        None => path.to_string(),
    };
    shown.chars().take(MAX_SHORT_LEN).collect()
}
/// Final component after last `/`; `/`->`/`, trailing `/` cut.
#[must_use]
pub fn base_name(path: &str) -> &str {
    if path.is_empty() || path == "/" {
        return path;
    }
    let t = path.trim_end_matches('/');
    if t.is_empty() {
        return "/";
    }
    t.rsplit('/').next().unwrap_or(t)
}
/// Directory above; `/` stays `/`, bare name -> `""`.
#[must_use]
pub fn parent_of(path: &str) -> &str {
    if path.is_empty() || path == "/" {
        return path;
    }
    let t = path.trim_end_matches('/');
    if t.is_empty() {
        return "/";
    }
    match t.rfind('/') {
        None => "",
        Some(0) => "/",
        Some(i) => &t[..i],
    }
}
fn trim_end(s: &str) -> &str {
    if s.len() > 1 {
        s.trim_end_matches('/')
    } else {
        s
    }
}

fn strip_home<'a>(path: &'a str, home: &str) -> Option<&'a str> {
    if home.is_empty() || home == "/" || !path.starts_with(home) {
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
    fn tilde_hit_and_cap() {
        assert_eq!(short_path("/home/u/p/f.rs", "/home/u"), "~/p/f.rs");
        assert_eq!(short_path("/home/u", "/home/u"), "~");
        let long = format!("/home/u/{}", "a".repeat(400));
        assert_eq!(short_path(&long, "/home/u").chars().count(), 256);
    }
    #[test]
    fn boundary_reject() {
        assert_eq!(short_path("/home/u2/x", "/home/u"), "/home/u2/x");
        assert_eq!(short_path("/etc/hosts", "/home/u"), "/etc/hosts");
        assert_eq!(short_path("", "/home/u"), "");
    }

    #[test]
    fn basename() {
        assert_eq!(base_name("/a/b/c.rs"), "c.rs");
        assert_eq!(base_name("/a/b/"), "b");
        assert_eq!(base_name("file.txt"), "file.txt");
        assert_eq!(base_name("/"), "/");
        assert_eq!(base_name(""), "");
    }

    #[test]
    fn parent() {
        assert_eq!(parent_of("/a/b/c.rs"), "/a/b");
        assert_eq!(parent_of("/a"), "/");
        assert_eq!(parent_of("/"), "/");
        assert_eq!(parent_of("file.txt"), "");
        assert_eq!(parent_of(""), "");
    }
}
