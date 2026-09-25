#![forbid(unsafe_code)]
//! Path display (mirrors `path-format.tsx:formatPath` + `runtime.tsx:abbreviateHome`).
//! `format_relative` = TS `formatPath` minus IO: cwd-relative, then `~`, else as-is.
//! `shorten` = TS `locale.ts:truncateMiddle` on chars, not UTF-16 units.

/// Cap on displayed path chars.
pub const MAX_DISPLAY_LEN: usize = 1024;

/// TS `formatPath`: `.` when `path == cwd`, cwd-relative strip, `~` home prefix, else as-is.
#[must_use]
pub fn format_relative(path: &str, home: &str, cwd: &str) -> String {
    format_relative_uncapped(path, home, cwd)
        .chars()
        .take(MAX_DISPLAY_LEN)
        .collect()
}

/// TS `truncateMiddle`: passthrough when short, else `start + … + end`, char-safe.
#[must_use]
pub fn shorten(path: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let n = path.chars().count();
    if n <= max {
        return path.to_string();
    }
    if max == 1 {
        return "…".to_string();
    }
    let (a, b) = (max / 2, (max - 1) / 2);
    let s: String = path.chars().take(a).collect();
    let e: String = path.chars().skip(n - b).collect();
    format!("{s}…{e}")
}

fn format_relative_uncapped(path: &str, home: &str, cwd: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    if let Some(rest) = strip_dir(path, trim_end(cwd)) {
        return if rest.is_empty() {
            ".".into()
        } else {
            rest.into()
        };
    }
    let home = trim_end(home);
    if path == home {
        return "~".into();
    }
    match strip_dir(path, home) {
        Some(rest) if !home.is_empty() => format!("~/{rest}"),
        _ => path.into(),
    }
}

fn trim_end(s: &str) -> &str {
    if s.len() > 1 {
        s.trim_end_matches('/')
    } else {
        s
    }
}

/// `Some(rest)` when `path == dir` (`""`) or under it (boundary-safe); `/` owns all absolutes.
fn strip_dir<'a>(path: &'a str, dir: &str) -> Option<&'a str> {
    if dir.is_empty() || !path.starts_with(dir) || path == dir {
        return if path == dir && !dir.is_empty() {
            Some("")
        } else {
            None
        };
    }
    if dir == "/" {
        return Some(path[1..].trim_start_matches('/'));
    }
    path.strip_prefix(dir)
        .filter(|r| r.starts_with('/'))
        .map(|r| &r[1..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilde() {
        assert_eq!(
            format_relative("/home/u/p/f.rs", "/home/u", "/tmp"),
            "~/p/f.rs"
        );
        assert_eq!(format_relative("/home/u", "/home/u", "/tmp"), "~");
    }

    #[test]
    fn relative() {
        assert_eq!(format_relative("/r/a/b/c.rs", "/home/u", "/r/a"), "b/c.rs");
        assert_eq!(format_relative("/r/a", "/home/u", "/r/a"), ".");
    }

    #[test]
    fn passthrough() {
        assert_eq!(
            format_relative("/etc/hosts", "/home/u", "/r/a"),
            "/etc/hosts"
        );
        assert_eq!(format_relative("/home/u2/x", "/home/u", "/r"), "/home/u2/x");
        assert_eq!(format_relative("/home/u/p", "/home/u", "/home/u"), "p");
    }

    #[test]
    fn ellipsis_middle() {
        assert_eq!(shorten("abcdefghij", 5), "ab…ij");
        assert_eq!(shorten("/a/b", 35), "/a/b");
    }

    #[test]
    fn unicode_safe() {
        let out = shorten("héllo wörld ✓✓✓✓✓", 8);
        assert_eq!(out.chars().count(), 8);
        assert!(out.contains('…'));
    }
}
