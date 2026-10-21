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
    input_rows: Vec<String>,
}

impl TextSelection {
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
        true
    }

    /// Extend a live selection. Coordinates in padding clamp to the nearest
    /// real cell on that row; empty rows are skipped toward the selected side.
    pub fn drag(&mut self, x: u32, y: u32) {
        if !self.dragging || self.anchor.is_none() {
            return;
        }
        if let Some(pos) = self.nearest(x, y, self.anchor.unwrap()) {
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

    /// Clear anchor, focus, and the active drag state.
    pub fn clear(&mut self) {
        self.anchor = None;
        self.focus = None;
        self.dragging = false;
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

#[cfg(test)]
mod tests {
    use super::TextSelection;

    fn rows(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
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
}
