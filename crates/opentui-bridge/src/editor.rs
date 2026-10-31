#![forbid(unsafe_code)]
//! Editor / text-area model: edit buffer plus wrapped editor view (FFI-02
//! "text views and editors", FFI-05 measurement).
//!
//! EVIDENCE / BOUNDARY
//! - The audited boundary (`docs/ui-parity/opencode-ui-parity-kit/
//!   audit-opentui-boundary.md` §1 "Text and editing", §2 [N5]) records that the
//!   native side owns text buffers/views, styled content, selection and editor
//!   resources, and that `NativeRenderable` measures through that native text
//!   implementation.
//! - The vendored library **does** export an edit-buffer surface. Verified with
//!   `nm -gU crates/opentui-bridge/native/lib/aarch64-apple-darwin/libopentui.dylib`:
//!   `createEditBuffer`, `destroyEditBuffer`, `editBufferSetText`,
//!   `editBufferInsertText`, `editBufferInsertChar`, `editBufferNewLine`,
//!   `editBufferDeleteChar`, `editBufferDeleteCharBackward`,
//!   `editBufferDeleteLine`, `editBufferDeleteRange`, `editBufferGetCursor`,
//!   `editBufferSetCursor`, `editBufferSetCursorByOffset`,
//!   `editBufferSetCursorToLineCol`, `editBufferGetText`,
//!   `editBufferGetTextRange`, `editBufferGetTextRangeByCoords`,
//!   `editBufferGetLineStartOffset`, `editBufferGotoLine`,
//!   `editBufferGetNextWordBoundary`, `editBufferGetPrevWordBoundary`,
//!   `editBufferOffsetToPosition`, `editBufferPositionToOffset`,
//!   `editBufferUndo`, `editBufferRedo`, `editBufferSetTabWidth`, plus
//!   `editorView*` (viewport, wrap mode, visual cursor, selection, scroll
//!   margin, virtual line counts).
//! - **No audited header at the selected OpenTUI commit records these
//!   signatures** (the crate's `text.rs` ABI extract does not contain
//!   `createEditBuffer`, and `docs/…/audit-opentui-boundary.md` does not bind
//!   the edit surface), so declaring `extern "C"` here would be invented ABI.
//!   Fail closed: the model below is pure Rust over owned text, and the exported
//!   native surface is left unbound rather than guessed.
//! - Operation set and naming follow the exported symbol list above so a later
//!   audited header can replace this model one-to-one.

use crate::text;

/// Line cap (fail closed).
pub const MAX_LINES: usize = 100_000;
/// Byte cap for the whole document.
pub const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;
/// Undo/redo depth.
pub const MAX_HISTORY: usize = 128;
/// Default tab width of this model (2, the width OpenCode's editor widgets
/// assume). The native default is not recorded by the audit, so callers that
/// need a specific width must set it explicitly.
pub const DEFAULT_TAB_WIDTH: u8 = 2;

/// Text position: zero-based line, and zero-based **char** index in that line
/// (`editBufferOffsetToPosition`/`PositionToOffset` use the same pair).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Position {
    pub line: usize,
    pub col: usize,
}

impl Position {
    #[must_use]
    pub const fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

/// Fail-closed editor errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorError {
    /// Document would exceed [`MAX_LINES`] or [`MAX_TEXT_BYTES`].
    TooLarge,
    /// Position outside the document.
    OutOfRange,
    /// Tab width must be 1..=8.
    BadTabWidth,
    /// Wrap width zero is not a valid editor viewport.
    ZeroWidth,
}

impl core::fmt::Display for EditorError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::TooLarge => "editor document too large",
            Self::OutOfRange => "editor position out of range",
            Self::BadTabWidth => "editor tab width must be 1..=8",
            Self::ZeroWidth => "editor viewport width is zero",
        })
    }
}

impl std::error::Error for EditorError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    lines: Vec<Vec<char>>,
    cursor: Position,
}

/// Mutable text document with cursor, selection and history.
#[derive(Debug, Clone)]
pub struct Editor {
    lines: Vec<Vec<char>>,
    cursor: Position,
    anchor: Option<Position>,
    tab_width: u8,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    /// One empty line, cursor at the origin.
    #[must_use]
    pub fn new() -> Self {
        Self {
            lines: vec![Vec::new()],
            cursor: Position::default(),
            anchor: None,
            tab_width: DEFAULT_TAB_WIDTH,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let mut editor = Self::new();
        editor.set_text(text);
        editor
    }

    /// Replace the document (`editBufferSetText`). Clears selection and history.
    pub fn set_text(&mut self, text: &str) {
        let bytes = text.len().min(MAX_TEXT_BYTES);
        let mut limited = text;
        if bytes < text.len() {
            limited = &text[..floor_char_boundary(text, bytes)];
        }
        self.lines = limited
            .split('\n')
            .take(MAX_LINES)
            .map(|line| line.chars().collect())
            .collect();
        if self.lines.is_empty() {
            self.lines.push(Vec::new());
        }
        self.cursor = Position::default();
        self.anchor = None;
        self.undo.clear();
        self.redo.clear();
    }

    /// Whole document text with `\n` separators (`editBufferGetText`).
    #[must_use]
    pub fn text(&self) -> String {
        let mut out = String::new();
        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.extend(line.iter());
        }
        out
    }

    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// One logical line without its newline.
    #[must_use]
    pub fn line(&self, index: usize) -> Option<String> {
        self.lines.get(index).map(|line| line.iter().collect())
    }

    #[must_use]
    pub const fn cursor(&self) -> Position {
        self.cursor
    }

    #[must_use]
    pub const fn tab_width(&self) -> u8 {
        self.tab_width
    }

    /// `editBufferSetTabWidth`; 1..=8, fail closed otherwise.
    pub fn set_tab_width(&mut self, width: u8) -> Result<(), EditorError> {
        if !(1..=8).contains(&width) {
            return Err(EditorError::BadTabWidth);
        }
        self.tab_width = width;
        Ok(())
    }

    #[must_use]
    pub const fn selection_anchor(&self) -> Option<Position> {
        self.anchor
    }

    /// Select from `anchor` to the cursor, or clear with `None`.
    pub fn set_selection_anchor(&mut self, anchor: Option<Position>) {
        self.anchor = anchor;
    }

    pub fn clear_selection(&mut self) {
        self.anchor = None;
    }

    /// Normalised selection range `(start, end)`, `None` when empty.
    #[must_use]
    pub fn selection(&self) -> Option<(Position, Position)> {
        let anchor = self.anchor?;
        if anchor == self.cursor {
            return None;
        }
        Some(if anchor < self.cursor {
            (anchor, self.cursor)
        } else {
            (self.cursor, anchor)
        })
    }

    /// Selected text, `None` when there is no selection.
    #[must_use]
    pub fn selected_text(&self) -> Option<String> {
        let (start, end) = self.selection()?;
        Some(self.range_text(start, end))
    }

    fn range_text(&self, start: Position, end: Position) -> String {
        if start.line == end.line {
            return self.lines[start.line][start.col..end.col].iter().collect();
        }
        let mut out: String = self.lines[start.line][start.col..].iter().collect();
        for line in &self.lines[start.line + 1..end.line] {
            out.push('\n');
            out.extend(line.iter());
        }
        out.push('\n');
        out.extend(self.lines[end.line][..end.col].iter());
        out
    }

    fn snapshot(&mut self) {
        self.undo.push(Snapshot {
            lines: self.lines.clone(),
            cursor: self.cursor,
        });
        if self.undo.len() > MAX_HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// `editBufferUndo`.
    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(Snapshot {
            lines: self.lines.clone(),
            cursor: self.cursor,
        });
        self.lines = previous.lines;
        self.cursor = previous.cursor;
        self.anchor = None;
        true
    }

    /// `editBufferRedo`.
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(Snapshot {
            lines: self.lines.clone(),
            cursor: self.cursor,
        });
        self.lines = next.lines;
        self.cursor = next.cursor;
        self.anchor = None;
        true
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// `editBufferClearHistory`.
    pub fn clear_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// `editBufferSetCursorToLineCol`; clamped into the document.
    pub fn set_cursor(&mut self, line: usize, col: usize) {
        let line = line.min(self.lines.len() - 1);
        let col = col.min(self.lines[line].len());
        self.cursor = Position::new(line, col);
    }

    /// `editBufferGotoLine`; column resets to 0, out-of-range clamps.
    pub fn goto_line(&mut self, line: usize) {
        self.set_cursor(line, 0);
    }

    /// `editBufferGetCursor`-style move without going through the text.
    pub fn move_cursor(&mut self, delta_lines: i32, delta_cols: i32) {
        let line = if delta_lines < 0 {
            self.cursor
                .line
                .saturating_sub(delta_lines.unsigned_abs() as usize)
        } else {
            (self.cursor.line + delta_lines as usize).min(self.lines.len() - 1)
        };
        let col = if delta_cols < 0 {
            let base = if line == self.cursor.line {
                self.cursor.col
            } else {
                self.lines[line].len()
            };
            base.saturating_sub(delta_cols.unsigned_abs() as usize)
        } else {
            let base = if line == self.cursor.line {
                self.cursor.col
            } else {
                0
            };
            (base + delta_cols as usize).min(self.lines[line].len())
        };
        self.cursor = Position::new(line, col);
    }

    /// Line start as an absolute char offset (`editBufferGetLineStartOffset`).
    #[must_use]
    pub fn line_start_offset(&self, line: usize) -> Option<usize> {
        if line > self.lines.len() {
            return None;
        }
        Some(
            self.lines
                .iter()
                .take(line.min(self.lines.len()))
                .map(|l| l.len() + 1)
                .sum(),
        )
    }

    /// Absolute char offset of a position (`editBufferPositionToOffset`).
    #[must_use]
    pub fn position_to_offset(&self, position: Position) -> Option<usize> {
        if position.line >= self.lines.len() || position.col > self.lines[position.line].len() {
            return None;
        }
        Some(self.line_start_offset(position.line)? + position.col)
    }

    /// Inverse of [`Editor::position_to_offset`] (`editBufferOffsetToPosition`).
    #[must_use]
    pub fn offset_to_position(&self, offset: usize) -> Option<Position> {
        let mut remaining = offset;
        for (line, chars) in self.lines.iter().enumerate() {
            if remaining <= chars.len() {
                return Some(Position::new(line, remaining));
            }
            remaining -= chars.len() + 1;
        }
        None
    }

    /// `editBufferSetCursorByOffset`.
    pub fn set_cursor_by_offset(&mut self, offset: usize) {
        if let Some(position) = self.offset_to_position(offset) {
            self.cursor = position;
        }
    }

    /// `editBufferGetNextWordBoundary`: next start-of-word char index.
    #[must_use]
    pub fn next_word_boundary(&self, line: usize, col: usize) -> usize {
        let Some(chars) = self.lines.get(line) else {
            return col;
        };
        let mut i = col.min(chars.len());
        // Skip the current word, then any separators.
        while i < chars.len() && is_word_char(chars[i]) {
            i += 1;
        }
        while i < chars.len() && !is_word_char(chars[i]) {
            i += 1;
        }
        i
    }

    /// `editBufferGetPrevWordBoundary`: previous start-of-word char index.
    #[must_use]
    pub fn prev_word_boundary(&self, line: usize, col: usize) -> usize {
        let Some(chars) = self.lines.get(line) else {
            return col;
        };
        let mut i = col.min(chars.len());
        while i > 0 && !is_word_char(chars[i - 1]) {
            i -= 1;
        }
        while i > 0 && is_word_char(chars[i - 1]) {
            i -= 1;
        }
        i
    }

    /// Insert text at the cursor (replacing the selection). `\n` splits lines.
    /// Fails closed when the document would exceed the caps.
    pub fn insert_text(&mut self, text: &str) -> Result<(), EditorError> {
        if text.is_empty() {
            return Ok(());
        }
        if self.lines.len() + text.matches('\n').count() > MAX_LINES {
            return Err(EditorError::TooLarge);
        }
        let prospective = self.text().len() + text.len();
        if prospective > MAX_TEXT_BYTES {
            return Err(EditorError::TooLarge);
        }
        self.snapshot();
        self.delete_selection_internal();
        let parts: Vec<Vec<char>> = text.split('\n').map(|p| p.chars().collect()).collect();
        let Position { line, col } = self.cursor;
        if parts.len() == 1 {
            self.lines[line].splice(col..col, parts[0].iter().copied());
            self.cursor = Position::new(line, col + parts[0].len());
            return Ok(());
        }
        let suffix: Vec<char> = self.lines[line][col..].to_vec();
        self.lines[line].truncate(col);
        self.lines[line].extend(parts[0].iter().copied());
        let last = parts.len() - 1;
        let mut insert_at = line + 1;
        for middle in &parts[1..last] {
            self.lines.insert(insert_at, middle.clone());
            insert_at += 1;
        }
        let mut last_line = parts[last].clone();
        let last_col = last_line.len();
        last_line.extend(suffix);
        self.lines.insert(insert_at, last_line);
        self.cursor = Position::new(insert_at, last_col);
        Ok(())
    }

    /// `editBufferInsertChar`.
    pub fn insert_char(&mut self, ch: char) -> Result<(), EditorError> {
        let mut buf = [0u8; 4];
        self.insert_text(ch.encode_utf8(&mut buf))
    }

    /// `editBufferNewLine`.
    pub fn new_line(&mut self) -> Result<(), EditorError> {
        self.insert_text("\n")
    }

    /// `editBufferDeleteCharBackward`. At column 0 it joins with the previous
    /// line; at the document start it is a no-op.
    pub fn delete_char_backward(&mut self) -> bool {
        if self.selection().is_some() {
            self.snapshot();
            self.delete_selection_internal();
            return true;
        }
        let Position { line, col } = self.cursor;
        if col > 0 {
            self.snapshot();
            self.lines[line].remove(col - 1);
            self.cursor.col -= 1;
            return true;
        }
        if line == 0 {
            return false;
        }
        self.snapshot();
        let current = self.lines.remove(line);
        let previous_len = self.lines[line - 1].len();
        self.lines[line - 1].extend(current);
        self.cursor = Position::new(line - 1, previous_len);
        true
    }

    /// `editBufferDeleteChar`. At end of line it joins the next line.
    pub fn delete_char(&mut self) -> bool {
        if self.selection().is_some() {
            self.snapshot();
            self.delete_selection_internal();
            return true;
        }
        let Position { line, col } = self.cursor;
        if col < self.lines[line].len() {
            self.snapshot();
            self.lines[line].remove(col);
            return true;
        }
        if line + 1 >= self.lines.len() {
            return false;
        }
        self.snapshot();
        let next = self.lines.remove(line + 1);
        self.lines[line].extend(next);
        true
    }

    /// `editBufferDeleteLine`: removes the cursor line; the cursor moves to the
    /// start of the following line (or the previous one when it was last).
    pub fn delete_line(&mut self) -> bool {
        if self.lines.len() == 1 && self.lines[0].is_empty() {
            return false;
        }
        self.snapshot();
        self.lines.remove(self.cursor.line);
        if self.lines.is_empty() {
            self.lines.push(Vec::new());
        }
        let line = self.cursor.line.min(self.lines.len() - 1);
        self.cursor = Position::new(line, 0);
        self.anchor = None;
        true
    }

    /// `editBufferDeleteRange`; positions must be ordered and in range.
    pub fn delete_range(
        &mut self,
        start: Position,
        end: Position,
    ) -> Result<(), EditorError> {
        if start > end {
            return Err(EditorError::OutOfRange);
        }
        if start.line >= self.lines.len()
            || end.line >= self.lines.len()
            || start.col > self.lines[start.line].len()
            || end.col > self.lines[end.line].len()
        {
            return Err(EditorError::OutOfRange);
        }
        if start == end {
            return Ok(());
        }
        self.snapshot();
        if start.line == end.line {
            self.lines[start.line].drain(start.col..end.col);
        } else {
            let tail = self.lines[end.line][end.col..].to_vec();
            self.lines[start.line].truncate(start.col);
            self.lines[start.line].extend(tail);
            self.lines.drain(start.line + 1..=end.line);
        }
        self.cursor = start;
        self.anchor = None;
        Ok(())
    }

    fn delete_selection_internal(&mut self) {
        if let Some((start, end)) = self.selection() {
            if start.line == end.line {
                self.lines[start.line].drain(start.col..end.col);
            } else {
                let tail = self.lines[end.line][end.col..].to_vec();
                self.lines[start.line].truncate(start.col);
                self.lines[start.line].extend(tail);
                self.lines.drain(start.line + 1..=end.line);
            }
            self.cursor = start;
        }
        self.anchor = None;
    }
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn floor_char_boundary(text: &str, mut index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// Wrap mode of an [`EditorView`] (`editorViewSetWrapMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WrapMode {
    /// One visual row per logical line; content past the width is clipped.
    None,
    /// Cell-accurate wrapping, never splitting a wide glyph.
    #[default]
    Cell,
}

/// One visual row of a wrapped document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualRow {
    pub line: usize,
    /// First char index of the logical line shown on this row.
    pub start_col: usize,
    /// Exclusive end char index.
    pub end_col: usize,
}

/// Wrapped viewport over an [`Editor`] (`editorView*`).
#[derive(Debug, Clone)]
pub struct EditorView {
    width: u32,
    height: u32,
    wrap: WrapMode,
    scroll_top: u32,
    scroll_margin: u32,
    tab_width: u8,
}

impl EditorView {
    /// Fail closed on a zero-width viewport.
    pub fn new(width: u32, height: u32) -> Result<Self, EditorError> {
        if width == 0 {
            return Err(EditorError::ZeroWidth);
        }
        Ok(Self {
            width,
            height,
            wrap: WrapMode::Cell,
            scroll_top: 0,
            scroll_margin: 0,
            tab_width: DEFAULT_TAB_WIDTH,
        })
    }

    /// `editorViewSetViewportSize`.
    pub fn set_viewport_size(&mut self, width: u32, height: u32) -> Result<(), EditorError> {
        if width == 0 {
            return Err(EditorError::ZeroWidth);
        }
        self.width = width;
        self.height = height;
        Ok(())
    }

    #[must_use]
    pub const fn viewport_size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// `editorViewSetWrapMode`.
    pub fn set_wrap_mode(&mut self, wrap: WrapMode) {
        self.wrap = wrap;
    }

    #[must_use]
    pub const fn wrap_mode(&self) -> WrapMode {
        self.wrap
    }

    #[must_use]
    pub const fn scroll_top(&self) -> u32 {
        self.scroll_top
    }

    /// `editorViewSetScrollMargin`: rows kept visible above/below the cursor.
    pub fn set_scroll_margin(&mut self, rows: u32) {
        self.scroll_margin = rows;
    }

    /// `editorViewSetTabIndicator`-adjacent: tab stops for measurement.
    pub fn set_tab_width(&mut self, width: u8) -> Result<(), EditorError> {
        if !(1..=8).contains(&width) {
            return Err(EditorError::BadTabWidth);
        }
        self.tab_width = width;
        Ok(())
    }

    fn expansion_width(&self, line: &[char], start: usize) -> usize {
        let mut cells = 0;
        for ch in &line[start..] {
            cells += if *ch == '\t' {
                let stop = self.tab_width as usize;
                stop - (cells % stop)
            } else {
                text::char_cells(*ch)
            };
        }
        cells
    }

    /// Visual rows of the whole document (`editorViewGetVirtualLineCount`).
    #[must_use]
    pub fn rows(&self, editor: &Editor) -> Vec<VisualRow> {
        let mut out = Vec::new();
        let budget = self.width as usize;
        for (line_index, chars) in editor.lines.iter().enumerate() {
            if self.wrap == WrapMode::None {
                out.push(VisualRow {
                    line: line_index,
                    start_col: 0,
                    end_col: chars.len(),
                });
                continue;
            }
            let mut start = 0usize;
            let mut used = 0usize;
            let mut i = 0usize;
            while i < chars.len() {
                let cells = if chars[i] == '\t' {
                    let stop = self.tab_width as usize;
                    stop - (used % stop)
                } else {
                    text::char_cells(chars[i])
                };
                if used + cells > budget && i > start {
                    out.push(VisualRow {
                        line: line_index,
                        start_col: start,
                        end_col: i,
                    });
                    start = i;
                    used = 0;
                    continue;
                }
                used += cells;
                i += 1;
            }
            out.push(VisualRow {
                line: line_index,
                start_col: start,
                end_col: chars.len(),
            });
        }
        out
    }

    /// Total visual rows (`editorViewGetTotalVirtualLineCount`).
    #[must_use]
    pub fn total_rows(&self, editor: &Editor) -> u32 {
        self.rows(editor).len() as u32
    }

    /// Visual row/column of the cursor (`editorViewGetVisualCursor`).
    #[must_use]
    pub fn visual_cursor(&self, editor: &Editor) -> (u32, u32) {
        let cursor = editor.cursor;
        let rows = self.rows(editor);
        let mut chosen: Option<(u32, u32)> = None;
        for (row, visual) in rows.iter().enumerate() {
            if visual.line != cursor.line {
                continue;
            }
            if cursor.col >= visual.start_col && cursor.col <= visual.end_col {
                // A wrap boundary belongs to the *following* row (the cursor is
                // drawn at the start of the tail, not past the head's edge), so
                // the last matching row wins.
                let col = self.expansion_width(
                    &editor.lines[cursor.line][visual.start_col..cursor.col],
                    0,
                );
                chosen = Some((row as u32, col as u32));
            }
        }
        chosen.unwrap_or((0, 0))
    }

    /// Scroll so the cursor row is inside the viewport plus scroll margin.
    pub fn ensure_cursor_visible(&mut self, editor: &Editor) {
        let (row, _) = self.visual_cursor(editor);
        let margin = self.scroll_margin.min(self.height.saturating_sub(1) / 2);
        let top_limit = row.saturating_sub(margin);
        if top_limit < self.scroll_top {
            self.scroll_top = top_limit;
        }
        let bottom = self.scroll_top.saturating_add(self.height);
        if row.saturating_add(margin).saturating_add(1) > bottom {
            self.scroll_top = row
                .saturating_add(margin)
                .saturating_add(1)
                .saturating_sub(self.height);
        }
        let max = self.total_rows(editor).saturating_sub(self.height);
        self.scroll_top = self.scroll_top.min(max);
    }

    /// Rows currently painted (absolute visual rows, clipped to the document).
    #[must_use]
    pub fn visible_rows(&self, editor: &Editor) -> Vec<VisualRow> {
        let rows = self.rows(editor);
        rows.into_iter()
            .skip(self.scroll_top as usize)
            .take(self.height as usize)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_insert_text_and_cursor() {
        let mut e = Editor::new();
        e.insert_text("hello").unwrap();
        assert_eq!(e.cursor(), Position::new(0, 5));
        e.insert_text("\nworld").unwrap();
        assert_eq!(e.text(), "hello\nworld");
        assert_eq!(e.cursor(), Position::new(1, 5));
        e.set_cursor(0, 0);
        e.insert_text("> ").unwrap();
        assert_eq!(e.text(), "> hello\nworld");
        assert_eq!(e.cursor(), Position::new(0, 2));
        assert_eq!(e.line_count(), 2);
    }

    #[test]
    fn editor_backspace_and_delete_join_lines() {
        let mut e = Editor::from_text("ab\ncd");
        e.set_cursor(1, 0);
        assert!(e.delete_char_backward());
        assert_eq!(e.text(), "abcd");
        assert_eq!(e.cursor(), Position::new(0, 2));
        // Backspace at document start is a no-op.
        e.set_cursor(0, 0);
        assert!(!e.delete_char_backward());
        // Forward delete at end of line joins the next line.
        let mut e = Editor::from_text("ab\ncd");
        e.set_cursor(0, 2);
        assert!(e.delete_char());
        assert_eq!(e.text(), "abcd");
        assert_eq!(e.cursor(), Position::new(0, 2));
        e.set_cursor(0, 4);
        assert!(!e.delete_char(), "nothing after the last char");
        assert!(e.delete_char_backward());
        assert_eq!(e.text(), "abc");
    }

    #[test]
    fn editor_delete_line_and_empty_document_guard() {
        let mut e = Editor::from_text("a\nb\nc");
        e.set_cursor(1, 0);
        assert!(e.delete_line());
        assert_eq!(e.text(), "a\nc");
        assert_eq!(e.cursor(), Position::new(1, 0));
        assert!(e.delete_line());
        assert_eq!(e.text(), "a");
        assert!(e.delete_line(), "last line removed -> empty document");
        assert_eq!(e.text(), "");
        assert!(!e.delete_line(), "empty document has nothing to delete");
    }

    #[test]
    fn editor_selection_replace_and_selected_text() {
        let mut e = Editor::from_text("hello world");
        e.set_cursor(0, 0);
        e.set_selection_anchor(Some(Position::new(0, 6)));
        e.set_cursor(0, 11);
        assert_eq!(e.selection(), Some((Position::new(0, 6), Position::new(0, 11))));
        assert_eq!(e.selected_text().as_deref(), Some("world"));
        e.insert_text("there").unwrap();
        assert_eq!(e.text(), "hello there");
        assert_eq!(e.selected_text(), None, "selection consumed by insert");
        // Multi-line selection text keeps the newline.
        e.set_selection_anchor(Some(Position::new(0, 0)));
        e.set_cursor(0, 5);
        e.set_selection_anchor(Some(Position::new(0, 0)));
        e.insert_text("A\nB").unwrap();
        assert_eq!(e.text(), "A\nB there");
    }

    #[test]
    fn editor_delete_range_and_offsets_roundtrip() {
        let mut e = Editor::from_text("one\ntwo\nthree");
        assert_eq!(e.line_start_offset(0), Some(0));
        assert_eq!(e.line_start_offset(1), Some(4));
        assert_eq!(e.line_start_offset(2), Some(8));
        assert_eq!(e.line_start_offset(3), Some(14), "one past the last line");
        assert_eq!(e.line_start_offset(4), None);
        for (offset, expected) in [
            (0, Position::new(0, 0)),
            (3, Position::new(0, 3)),
            (4, Position::new(1, 0)),
            (13, Position::new(2, 5)),
        ] {
            let pos = e.offset_to_position(offset).unwrap();
            assert_eq!(pos, expected, "offset {offset}");
            assert_eq!(e.position_to_offset(pos), Some(offset));
        }
        assert_eq!(e.offset_to_position(14), None);
        e.delete_range(Position::new(0, 1), Position::new(2, 2)).unwrap();
        // "o" + tail of the last line after column 2 ("ree").
        assert_eq!(e.text(), "oree");
        assert_eq!(e.cursor(), Position::new(0, 1));
        assert_eq!(
            e.delete_range(Position::new(1, 0), Position::new(0, 0)),
            Err(EditorError::OutOfRange)
        );
        assert_eq!(
            e.delete_range(Position::new(0, 0), Position::new(9, 0)),
            Err(EditorError::OutOfRange)
        );
    }

    #[test]
    fn editor_word_boundaries() {
        let e = Editor::from_text("foo bar_baz  qux");
        assert_eq!(e.next_word_boundary(0, 0), 4, "after foo + space");
        assert_eq!(e.next_word_boundary(0, 4), 13);
        assert_eq!(e.prev_word_boundary(0, 12), 4);
        assert_eq!(e.prev_word_boundary(0, 0), 0);
        assert_eq!(e.next_word_boundary(0, 16), 16, "end of line");
        assert_eq!(e.next_word_boundary(9, 3), 3, "missing line is a no-op");
    }

    #[test]
    fn editor_undo_redo_bounded() {
        let mut e = Editor::new();
        e.insert_text("a").unwrap();
        e.insert_text("b").unwrap();
        assert!(e.undo());
        assert_eq!(e.text(), "a");
        assert!(e.can_redo());
        assert!(e.redo());
        assert_eq!(e.text(), "ab");
        // A new edit after undo drops the redo stack.
        e.undo();
        e.insert_text("c").unwrap();
        assert!(!e.can_redo());
        assert_eq!(e.text(), "ac");
        // History is bounded.
        for _ in 0..MAX_HISTORY + 10 {
            e.insert_text("x").unwrap();
        }
        let mut depth = 0;
        while e.undo() {
            depth += 1;
            assert!(depth <= MAX_HISTORY, "history exceeded bound");
        }
    }

    #[test]
    fn editor_caps_fail_closed() {
        let mut e = Editor::new();
        let big = "x".repeat(MAX_TEXT_BYTES + 1);
        assert_eq!(e.insert_text(&big), Err(EditorError::TooLarge));
        let many = "\n".repeat(MAX_LINES);
        assert_eq!(e.insert_text(&many), Err(EditorError::TooLarge));
        assert_eq!(e.set_tab_width(0), Err(EditorError::BadTabWidth));
        assert_eq!(e.set_tab_width(9), Err(EditorError::BadTabWidth));
        assert!(e.set_tab_width(4).is_ok());
        // Simulated over-long input is clipped, never panics.
        e.set_text(&big);
        assert_eq!(e.text().len(), MAX_TEXT_BYTES);
    }

    #[test]
    fn ffi05_editor_view_wraps_wide_chars() {
        let mut e = Editor::from_text("中中中\nabc");
        let view = EditorView::new(4, 3).unwrap();
        let rows = view.rows(&e);
        // "中中中" is 6 cells -> 2 rows at width 4 (2 wide glyphs, then 1).
        assert_eq!(
            rows[..2],
            [
                VisualRow {
                    line: 0,
                    start_col: 0,
                    end_col: 2
                },
                VisualRow {
                    line: 0,
                    start_col: 2,
                    end_col: 3
                }
            ]
        );
        assert_eq!(view.total_rows(&e), 3);
        assert_eq!(view.visible_rows(&e).len(), 3);
        // Cursor after the first wide glyph sits at visual column 2.
        e.set_cursor(0, 1);
        assert_eq!(view.visual_cursor(&e), (0, 2));
        // Cursor at the start of the wrapped tail is on the second row.
        e.set_cursor(0, 2);
        assert_eq!(view.visual_cursor(&e), (1, 0));
        assert_eq!(EditorView::new(0, 1).unwrap_err(), EditorError::ZeroWidth);
    }

    #[test]
    fn editor_view_scroll_margin_keeps_cursor_visible() {
        let text: Vec<String> = (0..20).map(|i| format!("line{i}")).collect();
        let mut e = Editor::from_text(&text.join("\n"));
        let mut view = EditorView::new(10, 5).unwrap();
        view.set_scroll_margin(1);
        e.set_cursor(9, 0);
        view.ensure_cursor_visible(&e);
        let (row, _) = view.visual_cursor(&e);
        assert!(row >= view.scroll_top());
        assert!(row < view.scroll_top() + 5);
        assert_eq!(view.scroll_top(), 6, "cursor row 9 + margin 1 + 1 - height 5");
        // Moving back to the top scrolls up again and clamps at 0.
        e.set_cursor(0, 0);
        view.ensure_cursor_visible(&e);
        assert_eq!(view.scroll_top(), 0);
        // Very short documents cannot scroll past their end.
        let mut e = Editor::from_text("a\nb");
        let mut view = EditorView::new(10, 5).unwrap();
        e.set_cursor(1, 0);
        view.ensure_cursor_visible(&e);
        assert_eq!(view.scroll_top(), 0);
    }

    #[test]
    fn editor_view_no_wrap_clips_to_width() {
        let e = Editor::from_text("abcdefghij\nklm");
        let mut view = EditorView::new(4, 2).unwrap();
        view.set_wrap_mode(WrapMode::None);
        let rows = view.rows(&e);
        assert_eq!(rows.len(), 2, "one row per logical line");
        assert_eq!(rows[0].end_col, 10);
        assert!(view.set_viewport_size(0, 5).is_err());
        assert_eq!(view.set_tab_width(3).ok(), Some(()));
    }
}
