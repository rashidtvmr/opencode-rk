#![forbid(unsafe_code)]
//! Native frame assembler for `tui_entry paint_native`.
//!
//! Evidence: `crates/cli/src/tui_entry.rs:452-542` (`native_page_lines` Chat
//! branch) + `tui_entry.rs:662-666` (`MAX_NATIVE_TRANSCRIPT` drain cap).
//! Extracts the inline layout so the renderer only paints returned lines.
//!
//! Layout (Chat page): title, status bar, divider, transcript tail, padding,
//! divider, draft echo (`"> {draft}"`), hint line. Title `None` renders the
//! offline banner (`OFFLINE_TITLE`). Char-count clipping only; display-width
//! tables belong to GAP-03.
//!
//! Bounds: `cols == 0 || rows == 0` returns empty (fail-closed). Otherwise
//! mirrors the inline clamps (`width.max(20)`, `height.max(8)`).

/// Cap on retained transcript lines (tui_entry.rs:662).
pub const MAX_NATIVE_TRANSCRIPT: usize = 500;
/// Banner title when no live snapshot is bound (tui_entry.rs:466).
pub const OFFLINE_TITLE: &str = "OpenCode RK — offline";
/// Body placeholder for an empty transcript (tui_entry.rs:481).
pub const EMPTY_HINT: &str = "Start typing to send a turn.";
/// Footer hint under the draft echo (tui_entry.rs:491).
pub const DRAFT_HINT: &str = "Enter send • Backspace edit • Ctrl+P commands • Ctrl+T context";

/// Unicode-safe clip to `width` chars (tui_entry.rs:537-541).
#[must_use]
pub fn clip_line(line: &str, width: usize) -> String {
    if line.chars().count() <= width {
        return line.to_string();
    }
    line.chars().take(width).collect()
}

/// Tail of at most `MAX_NATIVE_TRANSCRIPT` lines (tui_entry.rs:662-666).
#[must_use]
pub fn capped_transcript(transcript: &[String]) -> &[String] {
    let start = transcript.len().saturating_sub(MAX_NATIVE_TRANSCRIPT);
    &transcript[start..]
}

/// Compose screen lines: transcript tail + draft echo + status bar.
/// `title: None` renders the offline banner. Output is `<= rows` lines,
//  each `<= width` chars.
#[must_use]
pub fn assemble_frame(
    transcript: &[String],
    draft: &str,
    title: Option<&str>,
    status_bar: &str,
    cols: usize,
    rows: usize,
) -> Vec<String> {
    if cols == 0 || rows == 0 {
        return Vec::new();
    }
    let width = cols.max(20);
    let height = rows.max(8);
    let mut lines = Vec::with_capacity(height);
    lines.push(title.unwrap_or(OFFLINE_TITLE).to_string());
    lines.push(status_bar.to_string());
    lines.push("─".repeat(width.min(120)));
    let body_rows = height.saturating_sub(7);
    let capped = capped_transcript(transcript);
    let start = capped.len().saturating_sub(body_rows);
    if capped.is_empty() {
        lines.push(EMPTY_HINT.to_string());
    } else {
        lines.extend(capped[start..].iter().cloned());
    }
    while lines.len() < height.saturating_sub(3) {
        lines.push(String::new());
    }
    lines.push("─".repeat(width.min(120)));
    lines.push(format!("> {draft}"));
    lines.push(DRAFT_HINT.to_string());
    lines.truncate(height);
    for line in &mut lines {
        if line.chars().count() > width {
            *line = clip_line(line, width);
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbered(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("line-{i}")).collect()
    }

    #[test]
    fn cap_enforced_at_500() {
        let t = numbered(600);
        let capped = capped_transcript(&t);
        assert_eq!(capped.len(), MAX_NATIVE_TRANSCRIPT);
        assert_eq!(capped[0], "line-100");
        assert_eq!(capped[499], "line-599");
    }

    #[test]
    fn cap_visible_in_frame() {
        let t = numbered(600);
        let lines = assemble_frame(&t, "", Some("T"), "s", 80, 520);
        assert_eq!(lines[3], "line-100");
        assert_eq!(lines[502], "line-599");
    }

    #[test]
    fn empty_transcript_shows_hint() {
        let lines = assemble_frame(&[], "", Some("T"), "s", 80, 24);
        assert!(lines.contains(&EMPTY_HINT.to_string()));
    }

    #[test]
    fn zero_size_fail_closed() {
        let t = numbered(3);
        assert!(assemble_frame(&t, "d", Some("T"), "s", 0, 24).is_empty());
        assert!(assemble_frame(&t, "d", Some("T"), "s", 80, 0).is_empty());
        assert!(assemble_frame(&t, "d", Some("T"), "s", 0, 0).is_empty());
    }

    #[test]
    fn draft_echo_included() {
        let lines = assemble_frame(&numbered(2), "hello", Some("T"), "s", 80, 24);
        assert!(lines.contains(&"> hello".to_string()));
    }

    #[test]
    fn offline_banner_without_title() {
        let lines = assemble_frame(&[], "", None, "s", 80, 24);
        assert_eq!(lines[0], OFFLINE_TITLE);
        let online = assemble_frame(&[], "", Some("OpenCode RK — live"), "s", 80, 24);
        assert_eq!(online[0], "OpenCode RK — live");
    }

    #[test]
    fn unicode_clip_is_char_safe() {
        let clipped = clip_line("héllo🍕world", 6);
        assert_eq!(clipped.chars().count(), 6);
        assert_eq!(clipped, "héllo🍕");
        let lines = assemble_frame(&["héllo🍕world".to_string()], "", Some("T"), "s", 20, 24);
        assert!(lines.iter().all(|l| l.chars().count() <= 20));
    }
}
