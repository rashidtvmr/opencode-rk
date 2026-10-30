//! Bounded word wrapping shared by prompt text and insertion cursors.
use crate::native_view_layout::visit_unpadded_cells;
const MAX_BYTES: usize = crate::native_view_chat::MAX_DRAFT_BYTES;

pub(crate) fn bounded_source(draft: &str) -> &str {
    let mut end = draft.len().min(MAX_BYTES);
    while !draft.is_char_boundary(end) { end -= 1; }
    &draft[..end]
}

fn normalize(draft: &str) -> String {
    // OpenTUI UnifiedTextBuffer defaults to a fixed two-cell tab width.
    crate::native_transcript::strip_ansi(draft).replace('\t', "  ")
}

pub(crate) struct PromptRows {
    pub rows: Vec<String>,
    pub cursor: Option<(usize, usize)>,
}

impl PromptRows {
    pub fn window_start(&self, height: usize) -> usize {
        if height == 0 || self.rows.is_empty() {
            return 0;
        }
        let row = self.cursor.map_or(self.rows.len().saturating_sub(1), |(_, row)| row);
        row.saturating_add(1).saturating_sub(height)
            .min(self.rows.len().saturating_sub(height))
    }
}

pub(crate) fn wrap_prompt(draft: &str, width: usize, cursor: usize) -> PromptRows {
    let draft = bounded_source(draft);
    // Rendering is deliberately bounded, but callers commonly pass the live
    // buffer's end offset.  Once the source is capped, that offset is still
    // the insertion point at the visible end (not an absent cursor).  Keep
    // non-boundary offsets invalid: an editor offset must never split UTF-8.
    let cursor = cursor.min(draft.len());
    let valid_cursor = draft.is_char_boundary(cursor);
    let before = valid_cursor.then(|| normalize(&draft[..cursor]));
    let cursor_line = before.as_ref().map(|value| value.bytes().filter(|byte| *byte == b'\n').count());
    let cursor_col = before.as_ref().map(|value| visit_unpadded_cells(
        value.rsplit('\n').next().unwrap_or(""), MAX_BYTES * 2, |_, _| {}));
    let cursor_byte = before.as_ref().map(|value| value.rsplit('\n').next().unwrap_or("").len());
    let clean = normalize(draft);
    let width = width.clamp(1, u16::MAX as usize);
    let mut result = PromptRows { rows: Vec::new(), cursor: None };
    for (logical_line, line) in clean.split('\n').enumerate() {
        let mut clusters = Vec::new();
        // The general painter attaches zero-width continuations to the previous
        // glyph. U+200B is instead a standalone editor break opportunity.
        for (index, part) in line.split('\u{200b}').enumerate() {
            if index > 0 { clusters.push(("\u{200b}".to_owned(), 0)); }
            visit_unpadded_cells(part, MAX_BYTES * 2, |glyph, cells| clusters.push((glyph.to_owned(), cells)));
        }
        let mut columns = Vec::with_capacity(clusters.len() + 1);
        let mut source_bytes = Vec::with_capacity(clusters.len() + 1);
        source_bytes.push(0usize);
        columns.push(0usize);
        for (glyph, cells) in &clusters {
            columns.push(columns.last().copied().unwrap_or(0) + cells);
            source_bytes.push(source_bytes.last().copied().unwrap_or(0) + glyph.len());
        }
        // Source starts are independent of rendered text: dropped separators
        // belong to the preceding row until the next row's source start.
        // (normalized source byte start, rendered text, cells, source cell start).
        // Byte boundaries distinguish either side of zero-cell separators.
        let mut rows: Vec<(usize, String, usize, usize)> = Vec::new();
        let mut current: Option<(usize, String, usize, usize)> = None;
        let mut seen_word = false;
        let mut index = 0;
        while index < clusters.len() {
            let ch = clusters[index].0.chars().next().unwrap_or(' ');
            let whitespace = is_wrap_whitespace(ch);
            if whitespace {
                if clusters[index].1 == 0 { index += 1; continue; }
                if current.as_ref().is_some_and(|row| row.2 + clusters[index].1 > width) {
                    rows.push(current.take().unwrap());
                }
                if !seen_word || current.is_some() {
                    let row = current.get_or_insert_with(|| (source_bytes[index], String::new(), 0, columns[index]));
                    row.1.push_str(&clusters[index].0);
                    row.2 += clusters[index].1;
                }
                index += 1;
                continue;
            }
            seen_word = true;
            // Queue a complete word, including its closing punctuation. CJK
            // intercharacter/script transitions are separate break positions.
            let start = index;
            index += 1;
            while index < clusters.len() {
                let previous = clusters[index - 1].0.chars().next().unwrap_or(' ');
                let next = clusters[index].0.chars().next().unwrap_or(' ');
                if is_wrap_whitespace(next) || "-/\\.,;:!?()[]{}".contains(previous)
                    || is_cjk(previous) || is_cjk(next) { break; }
                index += 1;
            }
            let word_width = columns[index] - columns[start];
            if current.as_ref().is_some_and(|row| row.2 + word_width > width) {
                rows.push(current.take().unwrap());
            }
            for part in start..index {
                let (glyph, cells) = &clusters[part];
                if current.as_ref().is_some_and(|row| row.2 + cells > width) {
                    rows.push(current.take().unwrap());
                }
                let row = current.get_or_insert_with(|| (source_bytes[part], String::new(), 0, columns[part]));
                // A double-width cluster in a one-cell pane becomes a blank.
                row.1.push_str(if *cells > width { " " } else { glyph });
                row.2 += (*cells).min(width);
            }
        }
        if let Some(row) = current { rows.push(row); }
        if rows.is_empty() { rows.push((0, String::new(), 0, 0)); }
        if cursor_line == Some(logical_line) {
            let column = cursor_col.unwrap_or(0);
            let row = rows.iter().rposition(|row| row.0 <= cursor_byte.unwrap_or(0)).unwrap_or(0);
            result.cursor = Some((column.saturating_sub(rows[row].3).min(rows[row].2), result.rows.len() + row));
        }
        result.rows.extend(rows.into_iter().map(|row| row.1));
    }
    result
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x2e80..=0x9fff | 0xac00..=0xd7af | 0xf900..=0xfaff | 0x20000..=0x3134f)
}

// OpenTUI utf8.zig::unicodeLayoutWrapBreakKind, in addition to ASCII
// space/tab. Do not use char::is_whitespace: its set differs (notably U+200B).
fn is_wrap_whitespace(c: char) -> bool {
    matches!(c as u32, 0x20 | 0x09 | 0x00a0 | 0x1680 | 0x2000..=0x200a
        | 0x202f | 0x205f | 0x3000 | 0x200b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_unicode_separators_share_hidden_boundary_affinity() {
        for separator in ["\u{3000}", "\u{00a0}", "\u{1680}", "\u{2003}", "\u{202f}", "\u{205f}", "\u{200b}"] {
            let draft = format!("hello{separator}world");
            assert_eq!(wrap_prompt(&draft, 5, draft.len()).rows, ["hello", "world"], "{separator:?}");
            assert_eq!(wrap_prompt(&draft, 5, 5).cursor, Some((5, 0)), "{separator:?}");
            assert_eq!(wrap_prompt(&draft, 5, 5 + separator.len()).cursor, Some((0, 1)), "{separator:?}");
        }
    }

    #[test]
    fn overflowing_separators_are_hidden_with_preceding_row_cursor_affinity() {
        for (draft, continuation) in [("hello world", 6), ("hello  world", 7)] {
            assert_eq!(wrap_prompt(draft, 5, draft.len()).rows, ["hello", "world"]);
            for cursor in 5..continuation {
                assert_eq!(wrap_prompt(draft, 5, cursor).cursor, Some((5, 0)));
            }
            assert_eq!(wrap_prompt(draft, 5, continuation).cursor, Some((0, 1)));
            assert_eq!(wrap_prompt(draft, 5, draft.len()).cursor, Some((5, 1)));
        }
    }

    #[test]
    fn leading_and_fitting_whitespace_remain_visible_before_an_overflowing_word() {
        assert_eq!(wrap_prompt(" hello", 5, 6).rows, [" ", "hello"]);
        assert_eq!(wrap_prompt("hi world", 5, 8).rows, ["hi ", "world"]);
        assert_eq!(wrap_prompt("hi world", 5, 3).cursor, Some((0, 1)));
        assert_eq!(wrap_prompt("hi world", 5, 4).cursor, Some((1, 1)));
    }

    #[test]
    fn tabs_have_the_same_two_cells_in_text_and_cursor_projection() {
        let rows = wrap_prompt("a\tb", 12, 2);
        assert_eq!(rows.rows, ["a  b"]);
        assert_eq!(rows.cursor, Some((3, 0)));
    }

    #[test]
    fn wrap_boundary_and_newline_keep_canonical_insertion_affinity() {
        assert_eq!(wrap_prompt("abcd", 4, 4).cursor, Some((4, 0)));
        assert_eq!(wrap_prompt("abcde", 4, 4).cursor, Some((0, 1)));
        assert_eq!(wrap_prompt("ab\ncd", 4, 2).cursor, Some((2, 0)));
        assert_eq!(wrap_prompt("ab\ncd", 4, 3).cursor, Some((0, 1)));
    }

    #[test]
    fn unicode_clusters_keep_text_and_end_caret_in_the_same_display_space() {
        for cluster in ["界", "e\u{301}", "🇺🇸", "👩\u{200d}💻"] {
            let draft = format!("{cluster}x");
            let rows = wrap_prompt(&draft, 2, draft.len());
            assert_eq!(rows.rows.concat(), draft);
            let (column, row) = rows.cursor.unwrap();
            let expected = visit_unpadded_cells(&rows.rows[row], 32, |_, _| {});
            assert_eq!(column, expected);
        }
        let rows = wrap_prompt("界", 1, "界".len());
        assert_eq!(rows.rows, [" "]);
        assert_eq!(rows.cursor, Some((1, 0)));
    }

    #[test]
    fn ansi_is_not_counted_in_rows_or_insertion_columns() {
        let draft = "\u{1b}[31mred\u{1b}[0m blue";
        let rows = wrap_prompt(draft, 7, draft.len());
        assert_eq!(rows.rows, ["red ", "blue"]);
        assert_eq!(rows.cursor, Some((4, 1)));
    }

    #[test]
    fn width_43_and_44_keep_the_same_cursor_projection_at_the_boundary() {
        for width in [43, 44] {
            let draft = format!("{}界", "x".repeat(width - 2));
            let rows = wrap_prompt(&draft, width, draft.len());
            assert_eq!(rows.rows.len(), 1, "width={width}");
            assert_eq!(rows.cursor, Some((width, 0)), "width={width}");
        }
    }

    #[test]
    fn capped_live_cursor_projects_to_the_end_of_the_visible_source() {
        let draft = "x".repeat(MAX_BYTES + 17);
        let rows = wrap_prompt(&draft, 43, draft.len());
        assert!(rows.cursor.is_some());
        let (column, row) = rows.cursor.unwrap();
        assert_eq!(row, rows.rows.len() - 1);
        assert_eq!(column, visit_unpadded_cells(&rows.rows[row], usize::MAX, |_, _| {}));
    }

    #[test]
    fn zero_height_window_is_empty_and_does_not_scroll_past_the_rows() {
        let rows = wrap_prompt("hello", 5, 5);
        assert_eq!(rows.window_start(0), 0);
        assert_eq!(rows.window_start(1), 0);
    }
}
