//! Bounded word wrapping shared by prompt text and insertion cursors, plus the
//! prompt surface geometry pinned by upstream
//! `450e7b92412c780a7b5a75226bb8d49aaca24e22`:
//! `component/prompt/index.tsx:1674` (editor max rows), `:1681-1698`
//! (left-only border, responsive horizontal inset, one-row top padding) and
//! `:1855-1898` (metadata block and the separate half-block lip).
use crate::native_view_layout::visit_unpadded_cells;
const MAX_BYTES: usize = crate::native_view_chat::MAX_DRAFT_BYTES;

pub(crate) fn bounded_source(draft: &str) -> &str {
    let mut end = draft.len().min(MAX_BYTES);
    while !draft.is_char_boundary(end) { end -= 1; }
    &draft[..end]
}

fn normalize(draft: &str) -> String {
    // Pinned `packages/tui/test/prompt-paste.test.tsx` pastes
    // `a\fb\u001b[31mc\u0000d` and expects the prompt row `xa^Lbc^@d`: the
    // editor keeps pasted C0 controls and paints each in caret notation, so a
    // control costs two cells (`^` plus `c ^ 0x40`) and the insertion caret
    // advances across those same two cells.
    //
    // `\n` and `\t` keep their editor meaning (row break, fixed two-cell tab)
    // and ESC stays literal, so genuine ANSI/OSC sequences are still recognized
    // by the shared sanitizer instead of being painted as `^[`. DEL (0x7f) is
    // not a C0 control and is not covered by the pinned oracle, so the shared
    // sanitizer keeps dropping it.
    let expanded = caret_expand_controls(draft);
    // OpenTUI UnifiedTextBuffer defaults to a fixed two-cell tab width.
    crate::native_transcript::strip_ansi(&expanded).replace('\t', "  ")
}

/// Rewrite bare C0 controls (`0x00..=0x1f` other than `\n`, `\t` and ESC) as
/// the caret pair `^` + `c ^ 0x40`, and consume C1 controls (see below).
///
/// ESC-prefixed terminal control sequences are copied byte-for-byte rather than
/// expanded inside: a caret pair inside CSI parameters ends the sequence at its
/// own `^` (a legal `0x40..=0x7e` final byte), and `^G` would destroy an
/// OSC/DCS `BEL` terminator. Either would change which bytes the shared
/// sanitizer strips afterwards, so ESC sequences keep exactly the grammar
/// `strip_ansi` already implements, and only C0 controls outside them gain
/// cells.
///
/// C1 controls are *not* C0 controls and have no caret notation: a `&str`
/// always encodes them as `0xc2 0x80..=0x9f`, and a terminal treats them as
/// controls rather than glyphs. They are consumed (never copied and never
/// expanded) so neither the control scalar nor a DCS/SOS/PM/APC/OSC payload
/// can survive into the display projection or a painted row.
fn caret_expand_controls(draft: &str) -> String {
    let bytes = draft.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(draft.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == 0x1b {
            // ESC-prefixed sequence: copy it, do not touch its interior.
            let end = escape_sequence_end(bytes, index);
            out.extend_from_slice(&bytes[index..end]);
            index = end;
            continue;
        }
        // C1 control (`U+0080..=U+009F`). Consume the whole control and drop
        // it: a real terminal would, and the display projection must not paint
        // a spurious blank cell or print a buried payload as visible text.
        if byte == 0xc2 {
            if let Some(&second) = bytes.get(index + 1) {
                if (0x80..=0x9f).contains(&second) {
                    index = c1_control_end(bytes, index, second);
                    continue;
                }
            }
        }
        if byte < 0x20 && byte != b'\n' && byte != b'\t' {
            out.push(b'^');
            out.push(byte + 0x40);
            index += 1;
            continue;
        }
        let end = (index + utf8_width(byte)).min(bytes.len());
        out.extend_from_slice(&bytes[index..end]);
        index = end;
    }
    // Expansion only inserts ASCII, so valid input stays valid UTF-8.
    String::from_utf8(out).unwrap_or_default()
}

/// Byte offset just past the ESC sequence starting at `start`, mirroring the
/// ESC grammar in `native_transcript::strip_ansi`.
fn escape_sequence_end(bytes: &[u8], start: usize) -> usize {
    let len = bytes.len();
    if start + 1 >= len {
        return len;
    }
    match bytes[start + 1] {
        b'[' => csi_end(bytes, start + 2),
        b']' => osc_end(bytes, start + 2),
        b'P' | b'X' | b'^' | b'_' => osc_end(bytes, start + 2),
        b'(' | b')' | b'#' => (start + 3).min(len),
        _ => (start + 2).min(len),
    }
}

/// CSI parameters/intermediates, ending on the first `0x40..=0x7e` byte.
fn csi_end(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() {
        let byte = bytes[index];
        index += 1;
        if (0x40..=0x7e).contains(&byte) {
            break;
        }
    }
    index
}

/// OSC/DCS/SOS/PM/APC payload, ending at `BEL`, `ST` (`ESC \`) or end of input.
fn osc_end(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() {
        if bytes[index] == 0x07 {
            return index + 1;
        }
        if bytes[index] == 0x1b && bytes.get(index + 1) == Some(&b'\\') {
            return index + 2;
        }
        index += 1;
    }
    bytes.len()
}

/// UTF-8 sequence width from a lead byte; the input is `&str`, so bytes agree.
fn utf8_width(lead: u8) -> usize {
    match lead {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        _ => 1,
    }
}

/// Byte offset just past the C1 control at `start` (lead `0xc2`, `second` in
/// `0x80..=0x9f`), following that control's own terminator.
///
/// `CSI` (U+009B) ends at its first `0x40..=0x7e` final byte; the DCS/SOS/PM/APC
/// string families (U+0090, U+0098, U+009E, U+009F) and `OSC` (U+009D) run to
/// `BEL`, `ST` or end-of-input; every other C1 scalar — including `ST` itself
/// (U+009C) — is a single zero-width no-op.
fn c1_control_end(bytes: &[u8], start: usize, second: u8) -> usize {
    match second {
        0x9b => csi_end(bytes, start + 2),
        0x90 | 0x98 | 0x9d | 0x9e | 0x9f => c1_string_end(bytes, start + 2),
        _ => start + 2,
    }
}

/// C1 string payload (`DCS`/`SOS`/`PM`/`APC`/`OSC`), ending at `BEL`, `ESC \`
/// (7-bit `ST`), UTF-8 `ST` (`0xc2 0x9c`) or end of input (fail closed).
///
/// This mirrors `native_transcript::strip_ansi`'s OSC/DCS grammar and adds the
/// UTF-8 `ST` terminator that only exists for C1-encoded controls. ESC-prefixed
/// sequences keep using [`osc_end`], so that grammar is unchanged. Every probe
/// byte is ASCII, so a multi-byte scalar inside the payload is never split.
fn c1_string_end(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() {
        if bytes[index] == 0x07 {
            return index + 1;
        }
        if bytes[index] == 0x1b && bytes.get(index + 1) == Some(&b'\\') {
            return index + 2;
        }
        if bytes[index] == 0xc2 && bytes.get(index + 1) == Some(&0x9c) {
            return index + 2;
        }
        index += 1;
    }
    bytes.len()
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

/// Horizontal inner padding of the raised prompt surface.
///
/// Upstream `component/prompt/index.tsx:1691-1692`:
/// `paddingLeft/Right = dimensions().width < 44 ? 1 : 2`. The threshold uses
/// the **terminal** width, not the prompt's own width.
pub(crate) fn prompt_inset(terminal_width: u16) -> u16 {
    if terminal_width < 44 { 1 } else { 2 }
}

/// Text column inside the prompt surface.
///
/// `pane_width` covers the whole prompt box, including the one-cell left
/// border (`index.tsx:1681-1689`). The inner box is `width="100%"`
/// (`:1690-1698`), so the text column is
/// `pane - 1 left border - 2 * inset`. This is deliberately non-monotonic at
/// the 44-column threshold: a 43-column prompt has 40 text cells, a 44-column
/// prompt only 39, because the inset doubles there.
pub(crate) fn prompt_text_width(pane_width: u16, terminal_width: u16) -> usize {
    let inset = usize::from(prompt_inset(terminal_width));
    usize::from(pane_width).saturating_sub(1 + 2 * inset)
}

/// Textarea row cap: `max(6, floor(terminalHeight / 3))`
/// (`component/prompt/index.tsx:1674` closure at the `maxHeight` prop,
/// `:1774`).
pub(crate) fn editor_max_rows(terminal_height: u16) -> usize {
    usize::from(terminal_height / 3).max(6)
}

/// Visible editor rows: minimum 1 (`index.tsx:1773`), grows until the cap and
/// then scrolls inside the textarea.
pub(crate) fn editor_rows(input_rows: usize, terminal_height: u16) -> usize {
    input_rows.clamp(1, editor_max_rows(terminal_height))
}

/// Rows owned by the raised surface itself: one top padding row, the editor
/// rows, and the metadata block (its own one-row top padding plus one row).
///
/// `index.tsx:1690-1698` (`paddingTop={1}`) and `:1855-1870`
/// (`<box ... paddingTop={1}>` around `PromptMetadataRow`).
pub(crate) fn surface_rows(input_rows: usize) -> usize {
    input_rows + 3
}

/// Rows of the whole prompt block: surface plus the separate one-row
/// half-block lip (`index.tsx:1873-1898`). The status rail
/// (`:1899-1983`) is a sibling, outside this block.
pub(crate) fn prompt_rows(input_rows: usize) -> usize {
    surface_rows(input_rows) + 1
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
    fn tamil_cursor_projection_matches_native_editor_prefix_oracle() {
        let draft = "தமிழ்";
        for (cursor, expected) in [(0, (0, 0)), (3, (1, 0)), (6, (2, 0)), (9, (2, 0)), (12, (3, 0)), (15, (3, 0))] {
            assert_eq!(wrap_prompt(draft, 3, cursor).cursor, Some(expected), "cursor={cursor}");
        }
        for (cursor, expected) in [(0, (0, 0)), (3, (1, 0)), (6, (1, 0)), (7, (2, 0))] {
            assert_eq!(wrap_prompt("மிX", 3, cursor).cursor, Some(expected), "cursor={cursor}");
        }
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

    // Pinned upstream `packages/tui/test/prompt-paste.test.tsx`
    // (`await setup.mockInput.pasteBracketedText("a\fb\u001b[31mc\u0000d")`):
    // the prompt keeps pasted control characters and renders each in caret
    // notation, so `xa\fbc\0d` occupies nine cells and the caret follows them.
    // The editor plaintext stays the seven literal characters.
    #[test]
    fn pasted_c0_controls_paint_caret_notation_cells_and_cursor() {
        let draft = "xa\u{0c}bc\u{0}d";
        let rows = wrap_prompt(draft, 43, draft.len());
        assert_eq!(rows.rows, ["xa^Lbc^@d"]);
        assert_eq!(visit_unpadded_cells(&rows.rows[0], 32, |_, _| {}), 9);
        assert_eq!(rows.cursor, Some((9, 0)));
        assert_eq!(draft.len(), 7, "the source draft bytes are unchanged");
    }

    #[test]
    fn control_caret_expansion_keeps_cursor_affinity_at_every_source_offset() {
        let draft = "xa\u{0c}bc\u{0}d";
        for (offset, column, caret) in [
            (0usize, 0usize, ""),
            (1, 1, "x"),
            (2, 2, "xa"),
            (3, 4, "xa^L"),
            (4, 5, "xa^Lb"),
            (5, 6, "xa^Lbc"),
            (6, 8, "xa^Lbc^@"),
            (7, 9, "xa^Lbc^@d"),
        ] {
            assert!(draft.is_char_boundary(offset), "offset={offset}");
            assert_eq!(wrap_prompt(draft, 43, offset).cursor, Some((column, 0)), "offset={offset}");
            assert_eq!(visit_unpadded_cells(caret, 32, |_, _| {}), column, "offset={offset}");
        }
    }

    #[test]
    fn ansi_sequences_are_stripped_and_never_displayed_while_controls_remain() {
        // The escape sequence disappears; the control character beside it does
        // not. No escape or control scalar reaches a painted row.
        let draft = "a\u{1b}[31m\u{0c}b";
        let rows = wrap_prompt(draft, 43, draft.len());
        assert_eq!(rows.rows, ["a^Lb"]);
        assert!(rows.rows.iter().all(|row| !row.chars().any(char::is_control)));
        assert!(!rows.rows.iter().any(|row| row.contains('\u{1b}')));
        // A dangling escape is still dropped, never rendered as `^[`.
        let dangling = "a\u{1b}";
        assert_eq!(wrap_prompt(dangling, 43, dangling.len()).rows, ["a"]);
        assert_eq!(wrap_prompt(dangling, 43, dangling.len()).cursor, Some((1, 0)));
        // Controls inside a sequence belong to the sequence: they pass through
        // untouched so the shared sanitizer still strips the whole sequence. A
        // caret pair expanded inside the parameters would terminate the
        // sequence early (`^` is a legal CSI final byte) and leak the rest.
        let csi_control = "a\u{1b}[1\u{0}mz";
        assert_eq!(wrap_prompt(csi_control, 43, csi_control.len()).rows, ["az"]);
        // BEL still terminates an OSC, so the text after it stays visible.
        let osc_bel = "a\u{1b}]0;t\u{7}b";
        assert_eq!(wrap_prompt(osc_bel, 43, osc_bel.len()).rows, ["ab"]);
    }

    #[test]
    fn tabs_newlines_del_and_utf8_keep_documented_cell_semantics() {
        // Tabs keep the two-cell OpenTUI width and newlines split rows. DEL is
        // still dropped exactly as `native_transcript::strip_ansi` documents:
        // the pinned paste oracle proves C0 caret notation only, so widening
        // DEL to `^?` would be an unproven control-cell policy change.
        let rows = wrap_prompt("a\tb\nc\u{7f}d", 43, 9);
        assert_eq!(rows.rows, ["a  b", "cd"]);
        assert_eq!(rows.cursor, Some((2, 1)));
        let utf8 = "a\u{0c}\u{754c}\nb";
        let rows = wrap_prompt(utf8, 43, utf8.len());
        assert_eq!(rows.rows, ["a^L\u{754c}", "b"]);
        assert_eq!(rows.cursor, Some((1, 1)));
    }

    #[test]
    fn wrapping_counts_expanded_control_cells_at_the_row_edge() {
        // `a b ^ A c d e` is seven cells, so a five-cell row must wrap.
        let draft = "ab\u{1}cde";
        let rows = wrap_prompt(draft, 5, draft.len());
        assert_eq!(rows.rows.len(), 2, "rows={:?}", rows.rows);
        assert_eq!(rows.rows.concat(), "ab^Acde");
        let (column, row) = rows.cursor.unwrap();
        assert_eq!(row, 1);
        assert_eq!(column, visit_unpadded_cells(&rows.rows[row], usize::MAX, |_, _| {}));
    }

    #[test]
    fn bounded_draft_with_controls_stays_bounded_and_projects() {
        let draft = "\u{1}".repeat(MAX_BYTES);
        let rows = wrap_prompt(&draft, 43, draft.len());
        assert!(!rows.rows.is_empty());
        assert!(rows.rows.iter().all(|row| visit_unpadded_cells(row, usize::MAX, |_, _| {}) <= 43));
        assert!(rows.rows.iter().all(|row| !row.chars().any(char::is_control)));
        assert!(rows.rows[0].starts_with("^A"));
        let (column, row) = rows.cursor.expect("visible cursor at the capped end");
        assert_eq!(row, rows.rows.len() - 1);
        assert_eq!(column, visit_unpadded_cells(&rows.rows[row], usize::MAX, |_, _| {}));
    }

    // -- C1 (UTF-8 `0xc2 0x80..=0x9f`) display projection -------------------
    //
    // A `&str` can only carry C1 controls as `U+0080..=U+009F`. A real
    // terminal does not paint them as glyphs: the DCS/SOS/PM/APC string
    // families swallow their payload until `BEL`, `ST` or end-of-input, `CSI`
    // ends at its first final byte, and every other C1 scalar is a zero-width
    // no-op. The display projection must consume them instead of turning them
    // into a space cell and printing the hostile payload as visible text.

    /// No control scalar (C0/C1/DEL) may reach a painted row.
    fn assert_no_control_scalars(rows: &PromptRows) {
        for row in &rows.rows {
            assert!(
                !row.chars().any(char::is_control),
                "row carries a control scalar: {row:?}"
            );
        }
    }

    #[test]
    fn c1_dcs_sos_pm_apc_payloads_are_consumed_for_every_terminator() {
        // DCS U+0090, SOS U+0098, PM U+009E, APC U+009F, each closed by the
        // three terminators the C1 grammar recognizes: BEL, 7-bit ST
        // (`ESC \`) and UTF-8 ST (U+009C).
        for introducer in ['\u{90}', '\u{98}', '\u{9e}', '\u{9f}'] {
            for terminator in ["\u{7}", "\u{1b}\\", "\u{9c}"] {
                let draft = format!("a{introducer}0;evil{terminator}b");
                let rows = wrap_prompt(&draft, 43, draft.len());
                assert_eq!(
                    rows.rows,
                    ["ab"],
                    "introducer={introducer:?} terminator={terminator:?}"
                );
                assert_eq!(
                    rows.cursor,
                    Some((2, 0)),
                    "introducer={introducer:?} terminator={terminator:?}"
                );
                assert_no_control_scalars(&rows);
                assert!(
                    !rows.rows.iter().any(|row| row.contains("evil")),
                    "payload leaked: {:?}",
                    rows.rows
                );
            }
        }
        // The UTF-8 ST scalar on its own is a no-op, not a payload start.
        assert_eq!(wrap_prompt("a\u{9c}0;evilb", 43, 12).rows, ["a0;evilb"]);
    }

    #[test]
    fn unterminated_c1_string_families_are_consumed_to_end_of_input() {
        // Fail closed: a string family (or OSC) with no terminator swallows
        // the remainder of the bounded source.
        for introducer in ['\u{90}', '\u{98}', '\u{9d}', '\u{9e}', '\u{9f}'] {
            let draft = format!("a{introducer}0;evil");
            let rows = wrap_prompt(&draft, 43, draft.len());
            assert_eq!(rows.rows, ["a"], "introducer={introducer:?}");
            assert_eq!(rows.cursor, Some((1, 0)), "introducer={introducer:?}");
            assert_no_control_scalars(&rows);
            assert!(!rows.rows.iter().any(|row| row.contains("evil")));
        }
        // A CSI introducer with no final byte is swallowed too.
        let csi = "a\u{9b}1;2";
        let rows = wrap_prompt(csi, 43, csi.len());
        assert_eq!(rows.rows, ["a"]);
        assert_eq!(rows.cursor, Some((1, 0)));
    }

    #[test]
    fn standalone_c1_controls_and_string_terminators_are_dropped_inertly() {
        for code in 0x80u32..=0x9f {
            let ch = char::from_u32(code).unwrap();
            let draft = format!("a{ch}b");
            let rows = wrap_prompt(&draft, 43, draft.len());
            assert_no_control_scalars(&rows);
            match code {
                // CSI consumes the following byte as its final byte.
                0x9b => {
                    assert_eq!(rows.rows, ["a"], "U+{code:04X}");
                    assert_eq!(rows.cursor, Some((1, 0)), "U+{code:04X}");
                }
                // String families are unterminated here, so they eat `b`.
                0x90 | 0x98 | 0x9d | 0x9e | 0x9f => {
                    assert_eq!(rows.rows, ["a"], "U+{code:04X}");
                    assert_eq!(rows.cursor, Some((1, 0)), "U+{code:04X}");
                }
                // Every remaining C1 scalar, including ST (U+009C), is a
                // zero-width no-op: the following `b` stays visible.
                _ => {
                    assert_eq!(rows.rows, ["ab"], "U+{code:04X}");
                    assert_eq!(rows.cursor, Some((2, 0)), "U+{code:04X}");
                }
            }
        }
    }

    #[test]
    fn malformed_c1_prefixes_never_leak_payload_or_control_scalars() {
        // Every one of these leaves only the leading `a`: the C1 introducer
        // consumes to end-of-input and no payload or control scalar escapes.
        for draft in [
            "a\u{90}",           // DCS introducer at end of input
            "a\u{90}\u{91}b",    // DCS payload begins with another C1
            "a\u{9b}",           // CSI without a final byte
            "a\u{9b}\u{7}b",     // BEL is not a CSI final; `b` is
            "a\u{90}\u{1b}",     // bare ESC at end of input
            "a\u{90}\u{1b}xb",   // ESC + `x` is not ST
            "a\u{90}\u{9b}b",    // C1 CSI nested inside a DCS payload
        ] {
            let rows = wrap_prompt(draft, 43, draft.len());
            assert_eq!(rows.rows, ["a"], "draft={draft:?}");
            assert_eq!(rows.cursor, Some((1, 0)), "draft={draft:?}");
            assert_no_control_scalars(&rows);
            assert!(
                !rows.rows.iter().any(|row| row.contains('0') || row.contains('x')),
                "draft={draft:?} rows={:?}",
                rows.rows
            );
        }
        // Stacked ST scalars are inert no-ops and the following text survives.
        assert_eq!(wrap_prompt("a\u{9c}\u{9c}b", 43, 8).rows, ["ab"]);
    }

    #[test]
    fn c1_consumption_keeps_c0_caret_projection_and_cursor_offsets() {
        // Byte layout: `a` | `C2 90` (DCS) | `07` (BEL terminator) | `b` | 界.
        let draft = "a\u{90}\u{7}b\u{754c}";
        for (offset, expected) in [
            (0usize, Some((0, 0))),
            (1, Some((1, 0))),
            (3, Some((1, 0))),
            (4, Some((1, 0))),
            (5, Some((2, 0))),
            (8, Some((4, 0))),
        ] {
            assert!(draft.is_char_boundary(offset), "offset={offset}");
            assert_eq!(wrap_prompt(draft, 43, offset).cursor, expected, "offset={offset}");
        }
        for non_boundary in [2usize, 6, 7] {
            assert!(!draft.is_char_boundary(non_boundary));
            assert_eq!(wrap_prompt(draft, 43, non_boundary).cursor, None, "offset={non_boundary}");
        }
        let rows = wrap_prompt(draft, 43, draft.len());
        assert_eq!(rows.rows, ["ab\u{754c}"]);
        assert_no_control_scalars(&rows);

        // C0 caret notation still applies after a consumed C1 string, and the
        // literal draft bytes are untouched.
        let mixed = "a\u{90}\u{7}\u{0c}b";
        let rows = wrap_prompt(mixed, 43, mixed.len());
        assert_eq!(rows.rows, ["a^Lb"]);
        assert_eq!(rows.cursor, Some((4, 0)));
        assert_eq!(mixed.len(), 6, "the source draft bytes are unchanged");
    }

    #[test]
    fn bounded_huge_unterminated_c1_stays_bounded_and_projects() {
        let mut draft = String::from("a\u{90}");
        draft.push_str(&"p".repeat(MAX_BYTES));
        let rows = wrap_prompt(&draft, 43, draft.len());
        assert_eq!(rows.rows, ["a"]);
        assert_eq!(rows.cursor, Some((1, 0)));
        assert!(
            rows.rows.iter().all(|row| visit_unpadded_cells(row, usize::MAX, |_, _| {}) <= 43),
            "rows={:?}",
            rows.rows
        );
        assert!(!rows.rows.iter().any(|row| row.contains('p')));
        assert_no_control_scalars(&rows);
        assert_eq!(bounded_source(&draft).len(), MAX_BYTES);
    }
}

#[cfg(test)]
mod prompt_geometry_tests {
    //! CP-04 geometry, pinned at `component/prompt/index.tsx:1674,1691-1698,
    //! 1773-1774,1855-1898`. The 40/43/44-column boundary is where the
    //! responsive inset changes the text column.

    use super::*;

    #[test]
    fn prompt_inset_flips_at_the_strict_44_column_threshold() {
        for (terminal, inset) in [(40, 1), (43, 1), (44, 2), (80, 2), (200, 2)] {
            assert_eq!(prompt_inset(terminal), inset, "terminal={terminal}");
        }
    }

    #[test]
    fn prompt_text_width_is_non_monotonic_across_the_44_column_boundary() {
        // 43 -> 43 - 1 - 2*1 = 40 text cells; 44 -> 44 - 1 - 2*2 = 39.
        for (columns, text) in [(40, 37), (43, 40), (44, 39), (45, 40)] {
            assert_eq!(
                prompt_text_width(columns, columns),
                text,
                "columns={columns}"
            );
        }
        assert_eq!(prompt_text_width(0, 0), 0);
        assert_eq!(prompt_text_width(1, 43), 0);
    }

    #[test]
    fn editor_grows_to_the_height_third_cap_then_scrolls() {
        // max(6, floor(height / 3)); heights from the CP-04 fixtures.
        for (height, cap) in [(12, 6), (24, 8), (40, 13)] {
            assert_eq!(editor_max_rows(height), cap, "height={height}");
            assert_eq!(editor_rows(0, height), 1, "one-row minimum at {height}");
            assert_eq!(editor_rows(1, height), 1);
            assert_eq!(editor_rows(cap - 1, height), cap - 1);
            assert_eq!(editor_rows(cap, height), cap);
            assert_eq!(editor_rows(usize::MAX, height), cap, "cap then scrolls");
        }
    }

    #[test]
    fn surface_owns_top_padding_editor_and_metadata_block_and_lip_is_separate() {
        // One-row prompt: pad, text, metadata pad, metadata = 4; lip = 1;
        // the status rail stays outside.
        assert_eq!(surface_rows(1), 4);
        assert_eq!(prompt_rows(1), 5);
        for rows in [1, 6, 8, 13] {
            assert_eq!(prompt_rows(rows), rows + 4);
        }
    }
}
