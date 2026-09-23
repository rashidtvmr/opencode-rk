//! BRG-GOLDEN: self-contained golden snapshot tests for a tiny text-grid
//! renderer (chat viewport with wrap, resize, and wide CJK characters).
//!
//! The `opentui-bridge` crate does not build mid-lane (`lib.rs` leaves modules
//! unwired; `input.rs`/`buffer.rs` are broken), so this file is deliberately
//! self-contained: std-only, no `use opentui_bridge::...` imports, compiling
//! directly with:
//!
//! ```sh
//! rustc --edition 2021 --test crates/opentui-bridge/tests/bridge_golden.rs \
//!   -o /tmp/opencode/brg_golden_test && /tmp/opencode/brg_golden_test
//! ```
//!
//! Layout: the `IMPL` section below holds the renderer (fixable); the `TESTS`
//! section holds the frozen golden tests. Post-freeze fixes touch IMPL only.
#![forbid(unsafe_code)]
#![allow(dead_code)]

// ============================ IMPL (fixable) ============================

/// Display width of one character: 2 for wide CJK ranges, else 1.
fn char_width(c: char) -> usize {
    let n = c as u32;
    if (0x1100..=0x115F).contains(&n)
        || (0x2E80..=0x9FFF).contains(&n)
        || (0xAC00..=0xD7A3).contains(&n)
        || (0xF900..=0xFAFF).contains(&n)
        || (0xFE30..=0xFE4F).contains(&n)
        || (0xFF00..=0xFF60).contains(&n)
        || (0xFFE0..=0xFFE6).contains(&n)
        || (0x20000..=0x3FFFD).contains(&n)
    {
        2
    } else {
        1
    }
}

/// Display width of a whole string.
fn line_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

/// Greedy wrap of one logical line into display rows of at most `cols`
/// display columns. A wide char is never split across rows.
fn wrap_line(line: &str, cols: usize) -> Vec<String> {
    if cols == 0 {
        return Vec::new();
    }
    let mut rows = Vec::new();
    let mut cur = String::new();
    let mut width = 0;
    for c in line.chars() {
        let w = char_width(c);
        if width + w > cols {
            rows.push(cur);
            cur = String::new();
            width = 0;
        }
        cur.push(c);
        width += w;
    }
    rows.push(cur);
    rows
}

/// Fixed-size text grid: logical lines plus a (cols, rows) viewport.
struct TextGrid {
    cols: usize,
    rows: usize,
    lines: Vec<String>,
}

impl TextGrid {
    fn new(cols: usize, rows: usize) -> Self {
        TextGrid {
            cols,
            rows,
            lines: Vec::new(),
        }
    }

    fn push_line(&mut self, s: &str) {
        self.lines.push(s.to_string());
    }

    /// Resize the viewport. Logical lines are untouched, so content is
    /// preserved and simply re-wrapped at the new width on render.
    fn resize(&mut self, cols: usize, rows: usize) {
        self.cols = cols;
        self.rows = rows;
    }

    /// All logical lines greedily wrapped at the current viewport width.
    fn wrapped(&self) -> Vec<String> {
        let mut out = Vec::new();
        for line in &self.lines {
            out.extend(wrap_line(line, self.cols));
        }
        out
    }

    /// Pad one display row with spaces to exactly `cols` display columns.
    fn pad_row(&self, s: &str) -> String {
        let mut row = s.to_string();
        for _ in 0..self.cols.saturating_sub(line_width(s)) {
            row.push(' ');
        }
        row
    }

    /// Last `rows` wrapped rows, each padded to full viewport width.
    fn render_rows(&self) -> Vec<String> {
        let wrapped = self.wrapped();
        let take = self.rows.min(wrapped.len());
        let mut rows: Vec<String> = wrapped[wrapped.len().saturating_sub(take)..]
            .iter()
            .map(|s| self.pad_row(s))
            .collect();
        while rows.len() < self.rows {
            rows.push(" ".repeat(self.cols));
        }
        rows
    }

    /// Full viewport snapshot, rows joined by `\n`.
    fn render(&self) -> String {
        self.render_rows().join("\n")
    }
}

// ============================ TESTS (frozen) ============================

/// Golden snapshot: two chat lines on an 80x24 grid (no wrapping needed).
const SNAPSHOT: &str = "user: hello\nassistant: hi there";

/// Golden snapshot before resize: 20-col wrap of a 23-char line plus "hi".
const SNAPSHOT_RESIZE: &str = "abcdefghij0123456789\nXYZ\nhi";

/// Golden snapshot after resize to 10 cols: same content, re-wrapped.
const SNAPSHOT_RESIZED: &str = "abcdefghij\n0123456789\nXYZ\nhi";

/// Golden snapshot with wide CJK characters (each width 2) on 10 cols.
const SNAPSHOT_WIDE: &str = "ABCあいう\nあいうえお";

#[test]
fn golden_chat_80x24() {
    let mut grid = TextGrid::new(80, 24);
    grid.push_line("user: hello");
    grid.push_line("assistant: hi there");
    assert_eq!(grid.wrapped().join("\n"), SNAPSHOT);
    let rows = grid.render_rows();
    assert_eq!(rows.len(), 24);
    assert_eq!(rows[0], format!("{:<80}", "user: hello"));
    assert_eq!(rows[1], format!("{:<80}", "assistant: hi there"));
    for row in rows.iter().skip(2) {
        assert_eq!(row, &" ".repeat(80));
    }
    assert_eq!(grid.render(), rows.join("\n"));
}

#[test]
fn golden_resize() {
    let mut grid = TextGrid::new(20, 4);
    grid.push_line("abcdefghij0123456789XYZ");
    grid.push_line("hi");
    assert_eq!(grid.wrapped().join("\n"), SNAPSHOT_RESIZE);
    assert_eq!(grid.render_rows().len(), 4);
    grid.resize(10, 6);
    // Same logical content, re-wrapped at the narrower width.
    assert_eq!(grid.wrapped().join("\n"), SNAPSHOT_RESIZED);
    let rows = grid.render_rows();
    assert_eq!(rows.len(), 6);
    for row in &rows {
        assert_eq!(line_width(row), 10);
    }
    assert_eq!(rows[0], "abcdefghij");
    assert_eq!(rows[1], "0123456789");
    assert_eq!(rows[2], format!("{:<10}", "XYZ"));
    assert_eq!(rows[3], format!("{:<10}", "hi"));
    assert_eq!(rows[4], " ".repeat(10));
    assert_eq!(rows[5], " ".repeat(10));
}

#[test]
fn golden_wide_chars() {
    assert_eq!(char_width('A'), 1);
    assert_eq!(char_width('あ'), 2);
    assert_eq!(char_width('中'), 2);
    assert_eq!(line_width("ABCあいう"), 9);
    let mut grid = TextGrid::new(10, 3);
    grid.push_line("ABCあいう");
    grid.push_line("あいうえお");
    assert_eq!(grid.wrapped().join("\n"), SNAPSHOT_WIDE);
    let rows = grid.render_rows();
    assert_eq!(rows.len(), 3);
    assert_eq!(line_width(&rows[0]), 10);
    assert_eq!(line_width(&rows[1]), 10);
    // Wrap splits on display width and never splits a wide char.
    let mut narrow = TextGrid::new(5, 4);
    narrow.push_line("あいう");
    assert_eq!(
        narrow.wrapped(),
        vec!["あい".to_string(), "う".to_string()]
    );
}
