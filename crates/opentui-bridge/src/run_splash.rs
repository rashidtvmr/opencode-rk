#![forbid(unsafe_code)]
//! Splash + turn-summary writers (TS `run/splash.ts` read-only ref).
//!
//! [`splash_lines`] renders the 3-row startup banner (name, version,
//! hint), clipped to `width` columns. [`turn_separator`] is the turn
//! boundary marker. [`summary_commit`] joins the last 5 lines of a
//! turn summary, capped at [`SUMMARY_CAP`] bytes.

/// Turn boundary marker between run turns.
pub const TURN_SEPARATOR: &str = "---";

/// Byte cap for [`summary_commit`] output.
pub const SUMMARY_CAP: usize = 1024;

/// Clip a row to at most `width` chars (fail-closed: zero clips all).
fn clip(row: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut out = String::with_capacity(row.len().min(width));
    for (i, c) in row.chars().enumerate() {
        if i >= width {
            break;
        }
        out.push(c);
    }
    out
}

/// 3-row banner: app name, version line, hint. Each row clipped to `width`.
#[must_use]
pub fn splash_lines(app: &str, version: &str, width: usize) -> Vec<String> {
    let rows = [
        app.to_string(),
        format!("v{version}"),
        "type ? for help".to_string(),
    ];
    rows.iter().map(|r| clip(r, width)).collect()
}

/// Turn boundary marker.
#[must_use]
pub const fn turn_separator() -> &'static str {
    TURN_SEPARATOR
}

/// Join the last 5 lines, capped at [`SUMMARY_CAP`] bytes.
#[must_use]
pub fn summary_commit(lines: &[String]) -> String {
    let start = lines.len().saturating_sub(5);
    let joined = lines[start..].join("\n");
    if joined.len() <= SUMMARY_CAP {
        return joined;
    }
    let mut end = SUMMARY_CAP;
    while !joined.is_char_boundary(end) {
        end -= 1;
    }
    joined[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_has_three_rows() {
        let out = splash_lines("opencode", "1.2.3", 80);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0], "opencode");
        assert_eq!(out[1], "v1.2.3");
        assert_eq!(out[2], "type ? for help");
    }

    #[test]
    fn banner_clips_to_width() {
        let out = splash_lines("opencode-long-name", "9.9.9", 4);
        assert_eq!(out.len(), 3);
        for row in &out {
            assert!(row.chars().count() <= 4, "row {row:?} exceeds width");
        }
        assert_eq!(out[0], "open");
    }

    #[test]
    fn zero_width_clips_all() {
        let out = splash_lines("opencode", "1.0", 0);
        assert!(out.iter().all(|r| r.is_empty()));
    }

    #[test]
    fn separator_is_const_dashes() {
        assert_eq!(turn_separator(), "---");
        assert_eq!(TURN_SEPARATOR, "---");
    }

    #[test]
    fn summary_joins_last_five() {
        let lines: Vec<String> = (0..8).map(|i| format!("l{i}")).collect();
        assert_eq!(summary_commit(&lines), "l3\nl4\nl5\nl6\nl7");
    }

    #[test]
    fn summary_caps_at_1kib() {
        let lines = vec!["x".repeat(2000)];
        let out = summary_commit(&lines);
        assert!(out.len() <= SUMMARY_CAP, "len {}", out.len());
        assert_eq!(out.len(), SUMMARY_CAP);
    }

    #[test]
    fn summary_short_passthrough() {
        let lines = vec!["a".to_string(), "b".to_string()];
        assert_eq!(summary_commit(&lines), "a\nb");
        let empty: Vec<String> = vec![];
        assert_eq!(summary_commit(&empty), "");
    }
}
