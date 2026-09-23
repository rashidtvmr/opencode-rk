#![forbid(unsafe_code)]
//! Default session-title matcher (std-only, no regex crate).
//!
//! Evidence (TS checkout a0d9b6c; task notes divergence from 95daf90):
//! - `packages/tui/src/util/session.ts:1-3`
//!   `/^(New session - |Child session - )\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/`
//! - title origin `packages/core/src/session.ts:228`
//!   `` `New session - ${new Date(now).toISOString()}` ``
//! - prefix consts `packages/opencode/src/session/session.ts:48-49`
//!   `"New session - "`, `"Child session - "`
//! - test `packages/tui/test/util/session.test.ts:6-8`: both prefixes valid,
//!   `"New session - custom"` false.
//!
//! Port: prefix strip + exact 24-char `toISOString` shape check
//! (`YYYY-MM-DDTHH:MM:SS.sssZ` with fixed separators, digits elsewhere).

/// Prefix for generated parent session titles.
pub const NEW_PREFIX: &str = "New session - ";
/// Prefix for generated child session titles.
pub const CHILD_PREFIX: &str = "Child session - ";

/// True when `title` is a generated default title (prefix + ISO datetime).
#[must_use]
pub fn is_default_title(title: &str) -> bool {
    let rest = title
        .strip_prefix(NEW_PREFIX)
        .or_else(|| title.strip_prefix(CHILD_PREFIX));
    let Some(dt) = rest else { return false };
    is_iso_millis_z(dt)
}

/// `YYYY-MM-DDTHH:MM:SS.sssZ` shape: len 24, fixed separators, digits else.
fn is_iso_millis_z(s: &str) -> bool {
    const SEPS: [(usize, u8); 7] = [
        (4, b'-'),
        (7, b'-'),
        (10, b'T'),
        (13, b':'),
        (16, b':'),
        (19, b'.'),
        (23, b'Z'),
    ];
    let b = s.as_bytes();
    if b.len() != 24 {
        return false;
    }
    for (i, want) in SEPS {
        if b[i] != want {
            return false;
        }
    }
    for (i, c) in b.iter().enumerate() {
        if SEPS.iter().any(|(j, _)| *j == i) {
            continue;
        }
        if !c.is_ascii_digit() {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_prefix_valid() {
        assert!(is_default_title("New session - 2026-06-06T12:34:56.789Z"));
    }

    #[test]
    fn child_prefix_valid() {
        assert!(is_default_title("Child session - 2026-06-06T12:34:56.789Z"));
    }

    #[test]
    fn custom_false() {
        assert!(!is_default_title("New session - custom"));
    }

    #[test]
    fn bad_date_false() {
        assert!(!is_default_title("New session - 2026-06-06T12:34:56Z"));
        assert!(!is_default_title("New session - 2026/06/06T12:34:56.789Z"));
        assert!(!is_default_title("New session - 2026-06-06T12:34:56.78XZ"));
    }

    #[test]
    fn empty_and_prefix_only_false() {
        assert!(!is_default_title(""));
        assert!(!is_default_title("New session - "));
        assert!(!is_default_title("Child session - "));
    }
}
