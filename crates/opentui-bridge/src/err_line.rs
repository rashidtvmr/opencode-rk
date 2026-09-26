//! Error transcript line (`"error: <msg>"`, capped at 512 chars).
//!
//! TS truth: `crate::error_format::error_message` for message text;
//! `tui_entry` pushes `"error:"`-prefixed lines onto the transcript.

#![forbid(unsafe_code)]

/// Max chars in the rendered line (prefix included).
pub const MAX_ERR_LINE: usize = 512;

/// Prefix marking error transcript lines.
pub const ERR_PREFIX: &str = "error: ";

/// Render `msg` as an `"error: <msg>"` transcript line, capped at 512 chars.
#[must_use]
pub fn err_line(msg: &str) -> String {
    let msg = msg.trim();
    let mut line = String::with_capacity(ERR_PREFIX.len() + msg.len());
    line.push_str(ERR_PREFIX);
    line.push_str(msg);
    if line.len() > MAX_ERR_LINE {
        line.truncate(MAX_ERR_LINE);
    }
    line
}

/// True when `line` is an error transcript line.
#[must_use]
pub fn is_err_line(line: &str) -> bool {
    line.starts_with("error:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_plain_message() {
        assert_eq!(err_line("boom"), "error: boom");
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert_eq!(err_line("  boom  "), "error: boom");
    }

    #[test]
    fn caps_at_512_chars() {
        let long = "x".repeat(600);
        let line = err_line(&long);
        assert_eq!(line.len(), MAX_ERR_LINE);
        assert!(line.starts_with("error: "));
    }

    #[test]
    fn detects_error_lines() {
        assert!(is_err_line("error: boom"));
        assert!(is_err_line("error:"));
        assert!(!is_err_line("info: boom"));
        assert!(!is_err_line(""));
    }
}
