#![forbid(unsafe_code)]
//! Path normalization (mirrors `util/filesystem.ts:115-123` @ a0d9b6c).
//!
//! Spec cited `util/path.ts` (12 lines); actual source is `normalizePath`
//! in `filesystem.ts`: non-win32 passthrough, win32 `win32.resolve` +
//! `win32.normalize`. Fail-closed: empty -> empty; overlong -> truncated.

/// Cap on path chars kept.
pub const MAX_PATH_LEN: usize = 4096;

/// TS `normalizePath`: passthrough off Windows.
#[must_use]
pub fn normalize_path(p: &str) -> String {
    if p.is_empty() {
        return String::new();
    }
    #[cfg(windows)]
    {
        return normalize_path_win32(p);
    }
    #[cfg(not(windows))]
    {
        p.chars().take(MAX_PATH_LEN).collect()
    }
}

/// Win32 branch: backslash resolve note (slash -> `\`, collapse runs).
#[must_use]
pub fn normalize_path_win32(p: &str) -> String {
    let mut out = String::with_capacity(p.len());
    let mut prev_sep = false;
    for c in p.chars().take(MAX_PATH_LEN) {
        if c == '/' || c == '\\' {
            if !prev_sep {
                out.push('\\');
            }
            prev_sep = true;
        } else {
            out.push(c);
            prev_sep = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_off_windows() {
        assert_eq!(normalize_path("/a/b/c"), "/a/b/c");
    }

    #[test]
    fn empty_gives_empty() {
        assert_eq!(normalize_path(""), "");
    }

    #[test]
    fn win32_helper_slashes() {
        assert_eq!(normalize_path_win32("a/b\\\\c"), "a\\b\\c");
        assert_eq!(normalize_path_win32("C:/x//y"), "C:\\x\\y");
    }
}
