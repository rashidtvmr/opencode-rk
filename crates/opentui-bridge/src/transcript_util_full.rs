#![forbid(unsafe_code)]
//! Transcript line role/body helpers.
//!
//! TS truth: `packages/tui/src/util/transcript.ts` (message roles
//! `user`/`assistant`, text/reasoning/tool parts). No `you:`/`system:` line
//! protocol exists in TS; this parses bridge lines from `crate::reply_lines`
//! (`you:`/`assistant:`) and `crate::err_line` (`error:`), else `system:`.
//! ponytail: no `offline:` role; add when transcript stores it.

/// Role prefix of `line`: `you:` | `assistant:` | `error:` | `system:`.
#[must_use]
pub fn role_of(line: &str) -> &'static str {
    if line.starts_with("you:") {
        "you:"
    } else if line.starts_with("assistant:") {
        "assistant:"
    } else if line.starts_with("error:") {
        "error:"
    } else {
        "system:"
    }
}

/// Body after first space, else whole line (`"you: hi"` -> `"hi"`).
#[must_use]
pub fn body_of(line: &str) -> &str {
    match line.find(' ') {
        Some(i) => &line[i + 1..],
        None => line,
    }
}

/// True when `line` is a user line (`you:` prefix).
#[must_use]
pub fn is_user(line: &str) -> bool {
    role_of(line) == "you:"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles() {
        assert_eq!(role_of("you: hi"), "you:");
        assert_eq!(role_of("assistant: ok"), "assistant:");
        assert_eq!(role_of("error: boom"), "error:");
        assert_eq!(role_of("system: hi"), "system:");
        assert_eq!(role_of("other"), "system:");
    }

    #[test]
    fn body_after_space() {
        assert_eq!(body_of("you: hi there"), "hi there");
        assert_eq!(body_of("error: boom"), "boom");
    }

    #[test]
    fn body_no_space_is_whole() {
        assert_eq!(body_of("you:"), "you:");
        assert_eq!(body_of(""), "");
    }

    #[test]
    fn detects_user() {
        assert!(is_user("you: hi"));
        assert!(!is_user("assistant: ok"));
        assert!(!is_user("error: x"));
        assert!(!is_user("system: x"));
    }
}
