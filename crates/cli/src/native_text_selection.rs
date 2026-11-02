#![forbid(unsafe_code)]
//! Cell-aware selection for the native text renderer.
//!
//! This module deliberately has no terminal or clipboard dependency.  The
//! renderer supplies the rows it actually painted; consequently padding can
//! never become selectable text.
//!
//! Cell segmentation is shared with native painting, including regional flags,
//! emoji modifiers, joined emoji and combining sequences.

#[derive(Clone, Debug)]
struct Cell {
    start: u32,
    width: u32,
    text: String,
}

#[derive(Clone, Debug)]
struct Row {
    cells: Vec<Cell>,
    first_text: Option<usize>,
    last_text: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Pos {
    row: usize,
    cell: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseCopyEvent {
    LeftRelease,
    RightPress,
    RightRelease,
    Other,
}

impl MouseCopyEvent {
    pub fn from_input_kind(kind: u8) -> Self {
        match kind {
            2 => Self::LeftRelease,
            5 => Self::RightPress,
            6 => Self::RightRelease,
            _ => Self::Other,
        }
    }
}

/// Upstream flag uses truthy `true`/`1`; default disables copy only on Windows.
pub fn copy_on_select_disabled(value: Option<&str>, windows_default: bool) -> bool {
    value.map_or(windows_default, |value| {
        matches!(value.to_ascii_lowercase().as_str(), "true" | "1")
    })
}

/// Selection state over the real, rendered cells in a bounded view.
#[derive(Clone, Debug, Default)]
pub struct TextSelection {
    rows: Vec<Row>,
    origin_x: u32,
    origin_y: u32,
    cols: u32,
    anchor: Option<Pos>,
    focus: Option<Pos>,
    dragging: bool,
    dragged: bool,
    input_rows: Vec<String>,
}

impl TextSelection {
    /// Apply platform-sensitive mouse copy policy and return clipboard text.
    pub fn copy_for_input_kind(&mut self, kind: u8, disable_copy_on_select: bool) -> Option<String> {
        let event = MouseCopyEvent::from_input_kind(kind);
        let should_copy = match event {
            MouseCopyEvent::LeftRelease => {
                let dragged = std::mem::replace(&mut self.dragged, false);
                self.release();
                !disable_copy_on_select && dragged
            }
            MouseCopyEvent::RightPress => disable_copy_on_select,
            MouseCopyEvent::RightRelease | MouseCopyEvent::Other => false,
        };
        if !should_copy {
            return None;
        }
        // A single-cell start is not a meaningful upstream cell selection.
        if self.anchor == self.focus {
            return None;
        }
        // Upstream clipboard copy retains the selection highlight and click history.
        self.selected_text()
    }

    /// Replace the rendered view and discard any selection tied to the old view.
    pub fn set_rows(&mut self, lines: &[String], origin_x: u32, origin_y: u32, cols: u32) {
        let mut bounded = Vec::new();
        let mut total = 0usize;
        for line in lines.iter().take(512) {
            if total >= 256 * 1024 {
                break;
            }
            let room = (256 * 1024 - total).min(16 * 1024);
            let end = floor_char_boundary(line, room);
            bounded.push(line[..end].to_owned());
            total += end;
        }
        if self.origin_x == origin_x
            && self.origin_y == origin_y
            && self.cols == cols
            && self.input_rows == bounded
        {
            return;
        }
        self.rows = bounded
            .iter()
            .map(|line| {
                let cells = cells(line, cols);
                let first_text = cells
                    .iter()
                    .position(|c| !c.text.chars().all(char::is_whitespace));
                let last_text = cells
                    .iter()
                    .rposition(|c| !c.text.chars().all(char::is_whitespace));
                Row {
                    cells,
                    first_text,
                    last_text,
                }
            })
            .collect();
        self.input_rows = bounded;
        self.origin_x = origin_x;
        self.origin_y = origin_y;
        self.cols = cols;
        self.clear();
    }

    /// Begin selection only when the screen coordinate is on a real text cell.
    pub fn press(&mut self, x: u32, y: u32) -> bool {
        let Some(pos) = self.hit(x, y) else {
            self.clear();
            return false;
        };
        self.anchor = Some(pos);
        self.focus = Some(pos);
        self.dragging = true;
        self.dragged = false;
        true
    }

    /// Extend a live selection. Coordinates in padding clamp to the nearest
    /// real cell on that row; empty rows are skipped toward the selected side.
    pub fn drag(&mut self, x: u32, y: u32) {
        if !self.dragging || self.anchor.is_none() {
            return;
        }
        if let Some(pos) = self.nearest(x, y, self.anchor.unwrap()) {
            self.dragged = true;
            self.focus = Some(pos);
        }
    }

    /// End dragging while retaining the selection for copy/highlight.
    pub fn release(&mut self) {
        self.dragging = false;
    }

    /// Return only rendered text, with line breaks between selected view rows.
    pub fn selected_text(&self) -> Option<String> {
        let (lo, hi) = self.bounds()?;
        let mut out = String::new();
        for row_index in lo.row..=hi.row {
            let row = &self.rows[row_index];
            if row.cells.is_empty() {
                continue;
            }
            let Some(text_first) = row.first_text else {
                continue;
            };
            let text_last = row.last_text.unwrap();
            let first = (if row_index == lo.row {
                lo.cell
            } else {
                text_first
            })
            .max(text_first);
            let last = (if row_index == hi.row {
                hi.cell
            } else {
                text_last
            })
            .min(text_last);
            if first > last || row.cells.is_empty() {
                continue;
            }
            if !out.is_empty() {
                out.push('\n');
            }
            for cell in &row.cells[first..=last] {
                out.push_str(&cell.text);
            }
        }
        (!out.is_empty()).then_some(out)
    }

    /// Highlight spans as `(screen row, screen x, cell width)`.
    pub fn spans(&self) -> Vec<(u32, u32, u32)> {
        let Some((lo, hi)) = self.bounds() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for r in lo.row..=hi.row {
            let row = &self.rows[r];
            if row.cells.is_empty() {
                continue;
            }
            let Some(text_first) = row.first_text else {
                continue;
            };
            let text_last = row.last_text.unwrap();
            let first = (if r == lo.row { lo.cell } else { text_first }).max(text_first);
            let last = (if r == hi.row { hi.cell } else { text_last }).min(text_last);
            if first > last {
                continue;
            }
            for cell in &row.cells[first..=last] {
                out.push((
                    self.origin_y + r as u32,
                    self.origin_x + cell.start,
                    cell.width,
                ));
            }
        }
        out
    }

    /// True when a selection exists (anchor and focus set), matching the
    /// upstream `renderer.getSelection()` presence check (`selection.ts:73`).
    pub fn has_selection(&self) -> bool {
        self.anchor.is_some() && self.focus.is_some()
    }

    /// Clear anchor, focus, and the active drag state.
    pub fn clear(&mut self) {
        self.anchor = None;
        self.focus = None;
        self.dragging = false;
        self.dragged = false;
    }

    fn bounds(&self) -> Option<(Pos, Pos)> {
        let (a, f) = (self.anchor?, self.focus?);
        if a <= f { Some((a, f)) } else { Some((f, a)) }
    }

    fn hit(&self, x: u32, y: u32) -> Option<Pos> {
        if x < self.origin_x || y < self.origin_y || x >= self.origin_x + self.cols {
            return None;
        }
        let r = (y - self.origin_y) as usize;
        let row = self.rows.get(r)?;
        let local = x - self.origin_x;
        let first = row.first_text?;
        let last = row.last_text?;
        row.cells[first..=last]
            .iter()
            .position(|c| local >= c.start && local < c.start + c.width)
            .map(|cell| Pos {
                row: r,
                cell: first + cell,
            })
    }

    fn nearest(&self, x: u32, y: u32, anchor: Pos) -> Option<Pos> {
        if let Some(hit) = self.hit(x, y) {
            return Some(hit);
        }
        let target = if y <= self.origin_y {
            0
        } else {
            (y - self.origin_y) as usize
        };
        let target = target.min(self.rows.len().saturating_sub(1));
        if target >= anchor.row {
            for r in target..self.rows.len() {
                if let Some(last) = self.rows[r].last_text {
                    return Some(Pos { row: r, cell: last });
                }
            }
            for r in (0..=target).rev() {
                if let Some(last) = self.rows[r].last_text {
                    return Some(Pos { row: r, cell: last });
                }
            }
        } else {
            for r in (0..=target).rev() {
                if let Some(first) = self.rows[r].first_text {
                    return Some(Pos {
                        row: r,
                        cell: first,
                    });
                }
            }
            for r in target..self.rows.len() {
                if let Some(first) = self.rows[r].first_text {
                    return Some(Pos {
                        row: r,
                        cell: first,
                    });
                }
            }
        }
        None
    }
}

fn cells(s: &str, cols: u32) -> Vec<Cell> {
    let mut out = Vec::new();
    // visit_cells pads to its width. Bound that work by retained text bytes,
    // so a huge empty viewport cannot manufacture millions of padding cells.
    let width = cols
        .min(u32::from(u16::MAX))
        .min(s.len().saturating_mul(2).min(u32::MAX as usize) as u32);
    crate::native_view_layout::visit_cells(s, width as u16, |start, glyph, width| {
        out.push(Cell {
            start: u32::from(start),
            width: u32::from(width),
            text: glyph.to_owned(),
        });
    });
    out
}

fn floor_char_boundary(s: &str, max: usize) -> usize {
    let mut end = s.len().min(max);
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    end
}

/// Ctrl+C selection-key action. Mirrors upstream `handleSelectionKey`
/// (`packages/tui/src/util/selection.ts:78-85`): with no selection the key
/// falls through (`:73`); with copy-on-select on it clears and returns WITHOUT
/// consuming the event, so the key keeps propagating; a successful copy
/// retains the highlight; and only a failed copy clears and consumes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CtrlCKeyAction {
    /// Let the key reach the remaining handlers (app exit).
    FallThrough,
    /// Clear the highlight, then let the key reach the remaining handlers.
    ClearAndFallThrough,
    /// Copy succeeded; keep the highlight and click history.
    Retain,
    /// Copy failed on a real selection; clear it and consume the key.
    ClearAndConsume,
}

/// Decide the Ctrl+C selection-key action. `copy_on_select` is the effective
/// policy (copy-on-select enabled = not disabled) and `has_text` is whether
/// the copy would actually produce text.
pub fn ctrl_c_selection_key(
    copy_on_select: bool,
    has_selection: bool,
    has_text: bool,
) -> CtrlCKeyAction {
    if !has_selection {
        return CtrlCKeyAction::FallThrough;
    }
    if copy_on_select {
        return CtrlCKeyAction::ClearAndFallThrough;
    }
    if has_text {
        CtrlCKeyAction::Retain
    } else {
        CtrlCKeyAction::ClearAndConsume
    }
}

#[cfg(test)]
mod tests {
    use super::{copy_on_select_disabled, CtrlCKeyAction, MouseCopyEvent, TextSelection};
    use super::ctrl_c_selection_key;

    fn rows(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn ctrl_c_retains_on_success_and_clears_only_on_failure() {
        // No selection: the key falls through to app exit (selection.ts:73).
        assert_eq!(ctrl_c_selection_key(true, false, false), CtrlCKeyAction::FallThrough);
        assert_eq!(ctrl_c_selection_key(false, false, false), CtrlCKeyAction::FallThrough);
        // Copy-on-select ON: clear the highlight but let the key propagate
        // (upstream selection.ts:79-81 `if ((copyOnSelect && !editing) || ...)`).
        assert_eq!(
            ctrl_c_selection_key(true, true, true),
            CtrlCKeyAction::ClearAndFallThrough
        );
        assert_eq!(
            ctrl_c_selection_key(true, true, false),
            CtrlCKeyAction::ClearAndFallThrough
        );
        // Copy-on-select OFF: copy and retain on success (selection.ts:84-86).
        assert_eq!(ctrl_c_selection_key(false, true, true), CtrlCKeyAction::Retain);
        // Failed copy (selection, no text) clears AND consumes (:79-81).
        assert_eq!(
            ctrl_c_selection_key(false, true, false),
            CtrlCKeyAction::ClearAndConsume
        );
    }

    #[test]
    fn copied_selection_remains_present_for_ctrl_c() {
        let mut selection = selected();
        assert_eq!(selection.copy_for_input_kind(2, false).as_deref(), Some("界e\u{301}"));
        // The copy must NOT clear: has_selection stays true (upstream retains).
        assert!(selection.has_selection());
        assert_eq!(selection.selected_text().as_deref(), Some("界e\u{301}"));
        assert!(!selection.spans().is_empty());
    }

    #[test]
    fn blank_press_does_not_select() {
        let mut s = TextSelection::default();
        s.set_rows(&rows(&["abc"]), 2, 3, 8);
        assert!(!s.press(6, 3));
        assert_eq!(s.selected_text(), None);
    }

    #[test]
    fn drag_to_blank_clamps_to_text() {
        let mut s = TextSelection::default();
        s.set_rows(&rows(&["abc"]), 0, 0, 10);
        assert!(s.press(1, 0));
        s.drag(9, 0);
        s.release();
        assert_eq!(s.selected_text().as_deref(), Some("bc"));
        assert_eq!(s.spans(), vec![(0, 1, 1), (0, 2, 1)]);
    }

    #[test]
    fn multiline_skips_empty_row_and_joins_real_text() {
        let mut s = TextSelection::default();
        s.set_rows(&rows(&["ab", "", "cd"]), 4, 5, 8);
        assert!(s.press(4, 5));
        s.drag(7, 7);
        s.release();
        assert_eq!(s.selected_text().as_deref(), Some("ab\ncd"));
        assert_eq!(s.spans(), vec![(5, 4, 1), (5, 5, 1), (7, 4, 1), (7, 5, 1)]);
    }

    #[test]
    fn utf8_wide_and_combining_are_cell_aware() {
        let mut s = TextSelection::default();
        s.set_rows(&rows(&["a界e\u{301}"]), 0, 0, 8);
        assert!(s.press(1, 0));
        s.drag(3, 0);
        assert_eq!(s.selected_text().as_deref(), Some("界e\u{301}"));
        assert_eq!(s.spans(), vec![(0, 1, 2), (0, 3, 1)]);
    }

    #[test]
    fn empty_row_has_no_hit_or_span() {
        let mut s = TextSelection::default();
        s.set_rows(&rows(&["", "x"]), 0, 0, 4);
        assert!(!s.press(0, 0));
        assert!(s.spans().is_empty());
    }

    fn selected() -> TextSelection {
        let mut selection = TextSelection::default();
        selection.set_rows(&rows(&["a界e\u{301}"]), 0, 0, 8);
        assert!(selection.press(1, 0));
        selection.drag(3, 0);
        selection.release();
        selection
    }

    #[test]
    fn default_left_release_copies_bounded_utf8_selection_and_retains_it() {
        let mut selection = selected();
        assert_eq!(
            selection.copy_for_input_kind(2, false).as_deref(),
            Some("界e\u{301}")
        );
        assert_eq!(selection.selected_text().as_deref(), Some("界e\u{301}"));
    }

    #[test]
    fn disabled_copy_on_select_uses_right_press_only() {
        let mut selection = selected();
        assert_eq!(
            selection.copy_for_input_kind(5, true).as_deref(),
            Some("界e\u{301}")
        );
        assert_eq!(selection.selected_text().as_deref(), Some("界e\u{301}"));

        let mut selection = selected();
        assert_eq!(selection.copy_for_input_kind(2, true), None);
        assert_eq!(selection.selected_text().as_deref(), Some("界e\u{301}"));

        let mut selection = selected();
        assert_eq!(selection.copy_for_input_kind(5, false), None);
        assert_eq!(selection.selected_text().as_deref(), Some("界e\u{301}"));
    }

    #[test]
    fn right_release_never_copies_or_clears_selection() {
        for disabled in [false, true] {
            let mut selection = selected();
            assert_eq!(selection.copy_for_input_kind(6, disabled), None);
            assert_eq!(selection.selected_text().as_deref(), Some("界e\u{301}"));
        }
    }

    #[test]
    fn click_only_release_does_not_copy_and_keeps_single_cell_selection() {
        let mut selection = TextSelection::default();
        selection.set_rows(&rows(&["alpha"]), 0, 0, 8);
        assert!(selection.press(1, 0));
        assert_eq!(selection.copy_for_input_kind(2, false), None);
        assert_eq!(selection.selected_text().as_deref(), Some("l"));
        assert!(!selection.dragging);
        assert_eq!(selection.copy_for_input_kind(5, true), None);
    }

    #[test]
    fn drag_release_copies_unicode_multiline_and_retains_highlight() {
        let mut selection = TextSelection::default();
        selection.set_rows(&rows(&["a界", "e\u{301}x"]), 2, 3, 8);
        assert!(selection.press(3, 3));
        selection.drag(3, 4);
        let expected = "界\ne\u{301}x";
        assert_eq!(selection.copy_for_input_kind(2, false).as_deref(), Some(expected));
        assert_eq!(selection.selected_text().as_deref(), Some(expected));
        assert_eq!(selection.spans(), vec![(3, 3, 2), (4, 2, 1), (4, 3, 1)]);
    }

    #[test]
    fn left_release_ends_drag_even_when_copy_is_disabled() {
        let mut selection = TextSelection::default();
        selection.set_rows(&rows(&["abc"]), 0, 0, 8);
        assert!(selection.press(0, 0));
        selection.drag(1, 0);
        assert_eq!(selection.copy_for_input_kind(2, true), None);
        selection.drag(2, 0);
        assert_eq!(selection.selected_text().as_deref(), Some("ab"));
    }

    #[test]
    fn mouse_kind_and_upstream_flag_are_explicit() {
        assert_eq!(MouseCopyEvent::from_input_kind(2), MouseCopyEvent::LeftRelease);
        assert_eq!(MouseCopyEvent::from_input_kind(5), MouseCopyEvent::RightPress);
        assert_eq!(MouseCopyEvent::from_input_kind(6), MouseCopyEvent::RightRelease);
        assert_eq!(MouseCopyEvent::from_input_kind(0), MouseCopyEvent::Other);
        assert!(!copy_on_select_disabled(None, false));
        assert!(copy_on_select_disabled(None, true));
        assert!(copy_on_select_disabled(Some("TRUE"), false));
        assert!(copy_on_select_disabled(Some("1"), false));
        assert!(!copy_on_select_disabled(Some("false"), true));
        assert!(!copy_on_select_disabled(Some("0"), true));
    }

    #[test]
    fn copy_gesture_with_no_real_selection_is_inert() {
        let mut selection = TextSelection::default();
        selection.set_rows(&rows(&["  "]), 0, 0, 8);
        assert_eq!(selection.copy_for_input_kind(2, false), None);
        assert_eq!(selection.copy_for_input_kind(5, true), None);
        assert_eq!(selection.spans(), Vec::new());

        let mut outside = selected();
        outside.set_rows(&rows(&["changed"]), 0, 0, 8);
        assert_eq!(outside.copy_for_input_kind(2, false), None);
        assert_eq!(outside.selected_text(), None);
    }
}

#[cfg(test)]
mod cp41_cell_width_tests {
    //! CP-41: renderer-compatible offsets and cell widths
    //! (`component/prompt/autocomplete.tsx:660-960`,
    //! `component/prompt/index.tsx:707-761`).

    use super::TextSelection;

    #[test]
    fn cp41_selection_uses_cells_for_tabs_cjk_zwj_tamil_and_combining_marks() {
        let lines = vec![
            "a\tb".to_owned(),
            "界👩‍👩‍👧‍👦e\u{301}".to_owned(),
            "தமிழ்".to_owned(),
        ];
        let mut selection = TextSelection::default();
        selection.set_rows(&lines, 0, 0, 20);
        assert!(selection.press(0, 0));
        selection.drag(19, 2);
        selection.release();
        assert_eq!(
            selection.selected_text().as_deref(),
            // Tabs are painted as a space cell, so the renderer's text is what
            // is copied; wide/ZWJ/Tamil/combining clusters survive intact.
            Some("a b\n界👩‍👩‍👧‍👦e\u{301}\nதமிழ்"),
            "no byte-offset slicing: wide, ZWJ, Tamil and marks survive"
        );
        let spans = selection.spans();
        let wide: Vec<_> = spans.iter().filter(|(row, _, _)| *row == 1).copied().collect();
        assert_eq!(
            wide,
            vec![(1, 0, 2), (1, 2, 2), (1, 4, 1)],
            "CJK and ZWJ occupy two cells; e+combining occupies one"
        );
        let tamil: Vec<_> = spans.iter().filter(|(row, _, _)| *row == 2).collect();
        assert!(!tamil.is_empty());
        assert!(tamil.iter().all(|(_, _, width)| *width >= 1));
        // A drag past the right edge clamps to the last real cell, never
        // selecting viewport padding.
        assert!(spans.iter().all(|(_, x, width)| x + width <= 20));
    }
}
