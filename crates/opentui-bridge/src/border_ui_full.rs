#![forbid(unsafe_code)]
//! Full bordered box renderer (mirrors `packages/tui/src/ui/border.ts:1-21`;
//! `EmptyBorder`/`SplitBorder` presets live in `border.rs`). Single `─│┌┐└┘`
//! box: top rule embeds title, body lines padded/truncated, bottom rule.

/// Max box width (`box_width` ceiling) and max total rows (cap 64 rows).
pub const MAX_BOX_W: usize = 120;
/// Minimum box width (corners + at least 2 content cells).
pub const MIN_BOX_W: usize = 4;
/// Max rows returned by [`border_box`] (top + body + bottom).
pub const MAX_BOX_ROWS: usize = 64;

/// Clamp requested width into `[MIN_BOX_W, MAX_BOX_W]` (max 120).
#[must_use]
pub fn box_width(width: usize) -> usize {
    width.clamp(MIN_BOX_W, MAX_BOX_W)
}

/// Render titled box: top rule + title, padded body lines, bottom rule.
/// Body truncated so total rows never exceed [`MAX_BOX_ROWS`].
#[must_use]
pub fn border_box(title: &str, lines: &[String], width: usize) -> Vec<String> {
    let w = box_width(width);
    let inner = w - 2;
    let mut out = Vec::with_capacity(MAX_BOX_ROWS);
    out.push(top_rule(title, inner));
    for line in lines.iter().take(MAX_BOX_ROWS - 2) {
        let cut: String = line.chars().take(inner).collect();
        let pad = inner - cut.chars().count();
        out.push(format!("│{}{}│", cut, " ".repeat(pad)));
    }
    out.push(format!("└{}┘", "─".repeat(inner)));
    out
}

fn top_rule(title: &str, inner: usize) -> String {
    if title.is_empty() {
        return format!("┌{}┐", "─".repeat(inner));
    }
    let t: String = title.chars().take(inner.saturating_sub(3).max(1)).collect();
    let rest = inner - (t.chars().count() + 3);
    format!("┌─ {}{}┐", t, "─".repeat(rest + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_clamped_to_120() {
        assert_eq!(box_width(40), 40);
        assert_eq!(box_width(999), MAX_BOX_W);
        assert_eq!(box_width(0), MIN_BOX_W);
    }

    #[test]
    fn frame_corners_and_width() {
        let rows = border_box("", &["hi".to_string()], 10);
        assert_eq!(rows.len(), 3);
        assert!(rows[0].starts_with('┌') && rows[0].ends_with('┐'));
        assert!(rows[2].starts_with('└') && rows[2].ends_with('┘'));
        for r in &rows {
            assert_eq!(r.chars().count(), 10);
        }
    }

    #[test]
    fn title_embedded_in_top_rule() {
        let rows = border_box("hi", &[], 10);
        assert!(rows[0].contains("hi"), "top: {}", rows[0]);
        assert_eq!(rows[0].chars().count(), 10);
    }

    #[test]
    fn body_padded_and_truncated() {
        let rows = border_box("", &["ab".to_string(), "toolongline".to_string()], 6);
        assert_eq!(rows[1], "│ab  │");
        assert_eq!(rows[2], "│tool│");
    }

    #[test]
    fn rows_capped_at_64() {
        let lines = vec!["x".to_string(); 200];
        let rows = border_box("t", &lines, 20);
        assert_eq!(rows.len(), MAX_BOX_ROWS);
    }
}
