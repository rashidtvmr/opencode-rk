//! Phase-1 widget ports from opentui renderables.
//!
//! Sources: `opentui/packages/core/src/renderables/` — Box.ts (BoxRenderable,
//! border styles from `lib/border.ts`), Text.ts (TextRenderable, wrap/align via
//! text-buffer-view wrapMode), Input.ts (InputRenderable: single line,
//! height=1, wrapMode none, newlines stripped, maxLength), Select.ts
//! (SelectRenderable: move-up/down/fast, wrapSelection, scrollOffset),
//! TabSelect.ts (move-left/right horizontal tabs), ScrollBox.ts
//! (ScrollBoxRenderable: scrollX/scrollY viewport + offset), ScrollBar.ts
//! (scrollSize/scrollPosition/viewportSize clamping), Slider.ts
//! (SliderRenderable value clamp + fill ratio), TextTable.ts +
//! text-table-width.ts (content widths + padding, proportional fit).
//!
//! Cut (Phase-1 scope): Image, EmbeddedTerminal, ASCIIFont, Code,
//! Three/audio, Markdown, Diff, FrameBuffer.
//!
//! Self-contained: own minimal Grid/Cell, no sibling imports, std-only,
//! bounded, `#![forbid(unsafe_code)]`.

#![forbid(unsafe_code)]

/// Hard bound on grid dimensions (real terminals stay far below).
pub const MAX_GRID_DIM: usize = 256;
/// Hard bound on text input length (mirrors Input default maxLength 1000).
pub const MAX_INPUT_LEN: usize = 1000;
/// Hard bound on select options / table cells per axis.
pub const MAX_ITEMS: usize = 1024;

// ---------------------------------------------------------------------------
// Grid / Cell
// ---------------------------------------------------------------------------

/// A single terminal cell: one grapheme cluster slot plus highlight flag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: String,
    pub highlight: bool,
}

impl Cell {
    pub fn new(ch: &str) -> Cell {
        Cell { ch: ch.to_string(), highlight: false }
    }
    pub fn blank() -> Cell {
        Cell { ch: " ".to_string(), highlight: false }
    }
}

/// Minimal fixed-size character grid widgets render into.
#[derive(Clone, Debug)]
pub struct Grid {
    pub width: usize,
    pub height: usize,
    cells: Vec<Cell>,
}

impl Grid {
    pub fn new(width: usize, height: usize) -> Grid {
        let w = width.min(MAX_GRID_DIM);
        let h = height.min(MAX_GRID_DIM);
        Grid { width: w, height: h, cells: vec![Cell::blank(); w.saturating_mul(h)] }
    }

    pub fn set(&mut self, x: usize, y: usize, ch: &str) {
        if x < self.width && y < self.height {
            self.cells[y * self.width + x] = Cell::new(ch);
        }
    }

    pub fn set_highlight(&mut self, x: usize, y: usize, on: bool) {
        if x < self.width && y < self.height {
            self.cells[y * self.width + x].highlight = on;
        }
    }

    pub fn get(&self, x: usize, y: usize) -> Option<&Cell> {
        if x < self.width && y < self.height {
            Some(&self.cells[y * self.width + x])
        } else {
            None
        }
    }

    /// Plain-text contents of one row (concatenated cell strings).
    pub fn row_string(&self, y: usize) -> String {
        if y >= self.height {
            return String::new();
        }
        self.cells[y * self.width..(y + 1) * self.width].iter().map(|c| c.ch.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// Unicode display width (bounded, std-only)
// ---------------------------------------------------------------------------

/// Display width of a char: 2 for wide CJK/emoji ranges, else 1.
/// Control chars count as 0.
pub fn char_width(c: char) -> usize {
    if c < '\u{20}' || ('\u{7f}'..='\u{9f}').contains(&c) {
        return 0;
    }
    let v = c as u32;
    match v {
        0x1100..=0x115F
        | 0x2E80..=0x9FFF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x1F000..=0x1FAFF
        | 0x20000..=0x3FFFD => 2,
        _ => 1,
    }
}

/// Display width of a string.
pub fn str_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

/// Truncate `s` so its display width is at most `max_w`.
pub fn truncate_to_width(s: &str, max_w: usize) -> String {
    let mut out = String::new();
    let mut w = 0;
    for c in s.chars() {
        let cw = char_width(c);
        if w + cw > max_w {
            break;
        }
        out.push(c);
        w += cw;
    }
    out
}

// ---------------------------------------------------------------------------
// Box
// ---------------------------------------------------------------------------

/// Border styles mirroring `lib/border.ts` BorderChars.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BorderStyle {
    Single,
    Double,
    Rounded,
    Heavy,
}

impl BorderStyle {
    fn chars(self) -> [char; 6] {
        match self {
            // tl, tr, bl, br, h, v — exact glyphs from border.ts:39-84
            BorderStyle::Single => ['┌', '┐', '└', '┘', '─', '│'],
            BorderStyle::Double => ['╔', '╗', '╚', '╝', '═', '║'],
            BorderStyle::Rounded => ['╭', '╮', '╰', '╯', '─', '│'],
            BorderStyle::Heavy => ['┏', '┓', '┗', '┛', '━', '┃'],
        }
    }
}

/// Box widget: border + optional title + fill (Box.ts BoxRenderable).
#[derive(Clone, Debug)]
pub struct BoxWidget {
    pub width: usize,
    pub height: usize,
    pub style: BorderStyle,
    pub title: String,
    pub fill: char,
}

impl BoxWidget {
    pub fn new(width: usize, height: usize, style: BorderStyle) -> BoxWidget {
        BoxWidget { width: width.min(MAX_GRID_DIM), height: height.min(MAX_GRID_DIM), style, title: String::new(), fill: ' ' }
    }

    pub fn render(&self, g: &mut Grid, ox: usize, oy: usize) {
        if self.width < 2 || self.height < 2 {
            return;
        }
        let [tl, tr, bl, br, h, v] = self.style.chars();
        let mut hs = String::new();
        hs.push(h);
        let vs = v.to_string();
        for x in 0..self.width {
            let top = if x == 0 { tl } else if x + 1 == self.width { tr } else { h };
            let bot = if x == 0 { bl } else if x + 1 == self.width { br } else { h };
            let mut t = String::new();
            t.push(top);
            let mut b = String::new();
            b.push(bot);
            g.set(ox + x, oy, &t);
            g.set(ox + x, oy + self.height - 1, &b);
        }
        for y in 1..self.height - 1 {
            g.set(ox, oy + y, &vs);
            g.set(ox + self.width - 1, oy + y, &vs);
            if self.fill != ' ' {
                let mut f = String::new();
                f.push(self.fill);
                for x in 1..self.width - 1 {
                    g.set(ox + x, oy + y, &f);
                }
            }
        }
        // Title overwrites the top border, left-aligned after corner.
        if !self.title.is_empty() && self.width > 3 {
            let t = truncate_to_width(&self.title, self.width - 2);
            let mut col = ox + 1;
            for c in t.chars() {
                let mut s = String::new();
                s.push(c);
                g.set(col, oy, &s);
                col += char_width(c);
                if col >= ox + self.width - 1 {
                    break;
                }
            }
            let _ = hs;
        }
    }
}

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// Wrapped/aligned text (Text.ts TextRenderable; wrapMode char/word).
#[derive(Clone, Debug)]
pub struct TextWidget {
    pub width: usize,
    pub height: usize,
    pub content: String,
    pub align: TextAlign,
    pub wrap: bool,
}

impl TextWidget {
    pub fn new(width: usize, height: usize, content: &str) -> TextWidget {
        TextWidget {
            width: width.min(MAX_GRID_DIM),
            height: height.min(MAX_GRID_DIM),
            content: content.chars().take(MAX_GRID_DIM * MAX_GRID_DIM).collect(),
            align: TextAlign::Left,
            wrap: true,
        }
    }

    /// Lay out content into display lines (wrap at width, keep explicit \n).
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.width == 0 {
            return out;
        }
        for para in self.content.split('\n') {
            if !self.wrap {
                out.push(truncate_to_width(para, self.width));
                continue;
            }
            let mut cur = String::new();
            let mut cur_w = 0;
            for word in para.split(' ') {
                let ww = str_width(word);
                if ww > self.width {
                    // Long word: hard-split by chars.
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                    let mut chunk = String::new();
                    let mut chunk_w = 0;
                    for c in word.chars() {
                        let cw = char_width(c);
                        if chunk_w + cw > self.width {
                            out.push(chunk);
                            chunk = String::new();
                            chunk_w = 0;
                        }
                        chunk.push(c);
                        chunk_w += cw;
                    }
                    cur = chunk;
                    cur_w = chunk_w;
                    continue;
                }
                let need = if cur.is_empty() { ww } else { cur_w + 1 + ww };
                if need > self.width {
                    out.push(cur);
                    cur = word.to_string();
                    cur_w = ww;
                } else {
                    if !cur.is_empty() {
                        cur.push(' ');
                        cur_w += 1;
                    }
                    cur.push_str(word);
                    cur_w += ww;
                }
            }
            out.push(cur);
        }
        out.truncate(MAX_GRID_DIM);
        out
    }

    pub fn render(&self, g: &mut Grid, ox: usize, oy: usize) {
        for (i, line) in self.lines().iter().take(self.height).enumerate() {
            let lw = str_width(line);
            let pad = match self.align {
                TextAlign::Left => 0,
                TextAlign::Center => self.width.saturating_sub(lw) / 2,
                TextAlign::Right => self.width.saturating_sub(lw),
            };
            let mut col = ox + pad;
            for c in line.chars() {
                if col >= ox + self.width {
                    break;
                }
                let mut s = String::new();
                s.push(c);
                g.set(col, oy + i, &s);
                col += char_width(c).max(1);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Input (single-line edit + cursor)
// ---------------------------------------------------------------------------

/// Single-line editable input (Input.ts: height=1, wrap none, no newlines).
#[derive(Clone, Debug)]
pub struct InputWidget {
    pub width: usize,
    chars: Vec<char>,
    /// Cursor as char index into `chars` (0..=len).
    pub cursor: usize,
    pub max_length: usize,
}

impl InputWidget {
    pub fn new(width: usize) -> InputWidget {
        InputWidget { width: width.min(MAX_GRID_DIM), chars: Vec::new(), cursor: 0, max_length: MAX_INPUT_LEN }
    }

    pub fn value(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn set_value(&mut self, v: &str) {
        // Strip newlines like InputRenderable constructor/insertText.
        self.chars = v.chars().filter(|c| *c != '\n' && *c != '\r').take(self.max_length).collect();
        // Input.ts: cursor goes to end of initial value.
        self.cursor = self.chars.len();
    }

    /// Byte-column of the cursor for rendering (display width of prefix).
    pub fn cursor_col(&self) -> usize {
        self.chars[..self.cursor.min(self.chars.len())].iter().map(|c| char_width(*c)).sum()
    }

    pub fn insert(&mut self, c: char) {
        if c == '\n' || c == '\r' || self.chars.len() >= self.max_length {
            return;
        }
        let at = self.cursor.min(self.chars.len());
        self.chars.insert(at, c);
        self.cursor = at + 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 && !self.chars.is_empty() {
            let at = self.cursor.min(self.chars.len());
            self.chars.remove(at - 1);
            self.cursor = at - 1;
        }
    }

    pub fn delete(&mut self) {
        let at = self.cursor.min(self.chars.len());
        if at < self.chars.len() {
            self.chars.remove(at);
        }
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.chars.len());
    }

    pub fn render(&self, g: &mut Grid, ox: usize, oy: usize) {
        let mut col = ox;
        for c in self.chars.iter() {
            if col >= ox + self.width {
                break;
            }
            let mut s = String::new();
            s.push(*c);
            g.set(col, oy, &s);
            col += char_width(*c).max(1);
        }
        // Mark cursor cell with highlight (cursorCol clamped into width).
        let cc = (ox + self.cursor_col()).min(ox + self.width.saturating_sub(1).max(0));
        if self.width > 0 {
            g.set_highlight(cc, oy, true);
        }
    }
}

// ---------------------------------------------------------------------------
// Select / TabSelect
// ---------------------------------------------------------------------------

/// Option list with highlight + scroll (Select.ts; horizontal=true ~ TabSelect.ts).
#[derive(Clone, Debug)]
pub struct SelectWidget {
    pub options: Vec<String>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub viewport_height: usize,
    pub horizontal: bool,
    pub wrap_selection: bool,
}

impl SelectWidget {
    pub fn new(options: Vec<String>, viewport_height: usize) -> SelectWidget {
        SelectWidget {
            options: options.into_iter().take(MAX_ITEMS).collect(),
            selected: 0,
            scroll_offset: 0,
            viewport_height: viewport_height.max(1).min(MAX_GRID_DIM),
            horizontal: false,
            wrap_selection: true,
        }
    }

    fn clamp_selection(&mut self) {
        if self.options.is_empty() {
            self.selected = 0;
            self.scroll_offset = 0;
            return;
        }
        self.selected = self.selected.min(self.options.len() - 1);
        // Keep selection visible (scrollOffset like Select.ts).
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + self.viewport_height {
            self.scroll_offset = self.selected + 1 - self.viewport_height;
        }
    }

    pub fn move_down(&mut self) {
        if self.options.is_empty() {
            return;
        }
        if self.selected + 1 < self.options.len() {
            self.selected += 1;
        } else if self.wrap_selection {
            self.selected = 0;
        }
        self.clamp_selection();
    }

    pub fn move_up(&mut self) {
        if self.options.is_empty() {
            return;
        }
        if self.selected > 0 {
            self.selected -= 1;
        } else if self.wrap_selection {
            self.selected = self.options.len() - 1;
        }
        self.clamp_selection();
    }

    pub fn move_left(&mut self) {
        self.move_up();
    }

    pub fn move_right(&mut self) {
        self.move_down();
    }

    /// Visible option indices for the current viewport.
    pub fn visible(&self) -> Vec<usize> {
        if self.options.is_empty() {
            return Vec::new();
        }
        let start = self.scroll_offset.min(self.options.len() - 1);
        (start..(start + self.viewport_height).min(self.options.len())).collect()
    }

    pub fn render(&self, g: &mut Grid, ox: usize, oy: usize, width: usize) {
        if self.horizontal {
            // TabSelect: single row of tabs.
            let mut col = ox;
            for (i, opt) in self.options.iter().enumerate() {
                let label = format!(" {} ", truncate_to_width(opt, width.saturating_sub(2)));
                for c in label.chars() {
                    if col >= ox + width {
                        break;
                    }
                    let mut s = String::new();
                    s.push(c);
                    g.set(col, oy, &s);
                    if i == self.selected {
                        g.set_highlight(col, oy, true);
                    }
                    col += 1;
                }
            }
            return;
        }
        for (row, idx) in self.visible().iter().enumerate() {
            if row >= self.viewport_height {
                break;
            }
            let marker = if *idx == self.selected { "▸ " } else { "  " };
            let label = format!("{}{}", marker, truncate_to_width(&self.options[*idx], width.saturating_sub(2)));
            let mut col = ox;
            for c in truncate_to_width(&label, width).chars() {
                if col >= ox + width {
                    break;
                }
                let mut s = String::new();
                s.push(c);
                g.set(col, oy + row, &s);
                if *idx == self.selected {
                    g.set_highlight(col, oy + row, true);
                }
                col += 1;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ScrollBox + ScrollBar
// ---------------------------------------------------------------------------

/// Scrollable text viewport (ScrollBox.ts: scroll offsets over content).
#[derive(Clone, Debug)]
pub struct ScrollBoxWidget {
    pub lines: Vec<String>,
    pub width: usize,
    pub height: usize,
    pub scroll_y: usize,
    pub scroll_x: usize,
}

impl ScrollBoxWidget {
    pub fn new(lines: Vec<String>, width: usize, height: usize) -> ScrollBoxWidget {
        ScrollBoxWidget {
            lines: lines.into_iter().take(MAX_ITEMS).collect(),
            width: width.min(MAX_GRID_DIM),
            height: height.min(MAX_GRID_DIM),
            scroll_y: 0,
            scroll_x: 0,
        }
    }

    pub fn max_scroll_y(&self) -> usize {
        self.lines.len().saturating_sub(self.height)
    }

    pub fn scroll_to(&mut self, y: usize) {
        self.scroll_y = y.min(self.max_scroll_y());
    }

    pub fn scroll_by(&mut self, dy: isize) {
        let y = self.scroll_y as isize + dy;
        self.scroll_y = y.max(0).min(self.max_scroll_y() as isize) as usize;
    }

    /// Visible line slice for the current viewport.
    pub fn visible_lines(&self) -> &[String] {
        if self.lines.is_empty() {
            return &[];
        }
        let start = self.scroll_y.min(self.lines.len().saturating_sub(1).max(0).min(self.lines.len().max(1) - 1));
        let start = start.min(self.lines.len());
        let end = (start + self.height).min(self.lines.len());
        &self.lines[start..end]
    }

    pub fn render(&self, g: &mut Grid, ox: usize, oy: usize) {
        for (row, line) in self.visible_lines().iter().enumerate() {
            if row >= self.height {
                break;
            }
            // Horizontal offset in display-width space.
            let mut skipped = 0;
            let mut col = ox;
            for c in line.chars() {
                let cw = char_width(c).max(1);
                if skipped + cw <= self.scroll_x {
                    skipped += cw;
                    continue;
                }
                if col >= ox + self.width {
                    break;
                }
                let mut s = String::new();
                s.push(c);
                g.set(col, oy + row, &s);
                col += cw;
            }
        }
    }
}

/// Scrollbar thumb math (ScrollBar.ts: position clamped to size-viewport).
#[derive(Clone, Copy, Debug)]
pub struct ScrollBarWidget {
    pub vertical: bool,
    pub length: usize,
    pub scroll_size: usize,
    pub viewport_size: usize,
    pub position: usize,
}

impl ScrollBarWidget {
    pub fn new(vertical: bool, length: usize) -> ScrollBarWidget {
        ScrollBarWidget { vertical, length: length.min(MAX_GRID_DIM), scroll_size: 0, viewport_size: 0, position: 0 }
    }

    /// Clamp position like ScrollBar.ts scrollPosition setter.
    pub fn set_position(&mut self, pos: usize) {
        let max = self.scroll_size.saturating_sub(self.viewport_size);
        self.position = pos.min(max);
    }

    /// (thumb_start, thumb_len) in track cells; min thumb 1.
    pub fn thumb_range(&self) -> (usize, usize) {
        if self.length == 0 || self.scroll_size == 0 {
            return (0, self.length);
        }
        let total = self.scroll_size.max(self.viewport_size).max(1);
        let mut len = self.viewport_size * self.length / total;
        len = len.clamp(1, self.length);
        let max_pos = self.scroll_size.saturating_sub(self.viewport_size).max(1);
        let start = (self.position.min(max_pos) * (self.length - len) / max_pos).min(self.length - len);
        (start, len)
    }

    pub fn render(&self, g: &mut Grid, ox: usize, oy: usize) {
        let (start, len) = self.thumb_range();
        for i in 0..self.length {
            let thumb = i >= start && i < start + len;
            let (x, y) = if self.vertical { (ox, oy + i) } else { (ox + i, oy) };
            g.set(x, y, if thumb { "█" } else { "░" });
        }
    }
}

// ---------------------------------------------------------------------------
// TextTable
// ---------------------------------------------------------------------------

/// Bordered text table (TextTable.ts: padding, outer/inner borders).
#[derive(Clone, Debug)]
pub struct TextTableWidget {
    pub rows: Vec<Vec<String>>,
    pub cell_padding: usize,
    pub borders: bool,
}

impl TextTableWidget {
    pub fn new(rows: Vec<Vec<String>>) -> TextTableWidget {
        TextTableWidget {
            rows: rows.into_iter().take(MAX_ITEMS).map(|r| r.into_iter().take(MAX_ITEMS).collect()).collect(),
            cell_padding: 1,
            borders: true,
        }
    }

    fn ncols(&self) -> usize {
        self.rows.iter().map(|r| r.len()).max().unwrap_or(0)
    }

    /// Content width per column (display width incl. wide chars) + padding.
    /// Mirrors TextTable content-measure + text-table-width proportional fit.
    pub fn column_widths(&self) -> Vec<usize> {
        let n = self.ncols();
        let mut widths = vec![0usize; n];
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                widths[i] = widths[i].max(str_width(cell));
            }
        }
        widths.iter().map(|w| w + self.cell_padding * 2).collect()
    }

    /// Fit columns into `target` total width proportionally (water-fill lite:
    /// shrink largest-first down to min 1+padding, expand not needed).
    pub fn fit_widths(&self, target: usize) -> Vec<usize> {
        let mut widths = self.column_widths();
        let total: usize = widths.iter().sum::<usize>() + widths.len().saturating_sub(0) + 1;
        if total <= target || widths.is_empty() {
            return widths;
        }
        let min_w = self.cell_padding * 2 + 1;
        let mut over = total.saturating_sub(target);
        while over > 0 {
            let mut best: Option<usize> = None;
            for (i, w) in widths.iter().enumerate() {
                if *w > min_w && best.map_or(true, |b| *w > widths[b]) {
                    best = Some(i);
                }
            }
            match best {
                Some(i) => {
                    widths[i] -= 1;
                    over -= 1;
                }
                None => break,
            }
        }
        widths
    }

    pub fn render(&self, g: &mut Grid, ox: usize, oy: usize) {
        let widths = self.column_widths();
        if widths.is_empty() {
            return;
        }
        let pad = " ".repeat(self.cell_padding);
        let mut y = oy;
        if self.borders {
            let mut top = String::from("┌");
            for (i, w) in widths.iter().enumerate() {
                top.push_str(&"─".repeat(*w));
                top.push(if i + 1 == widths.len() { '┐' } else { '┬' });
            }
            for (x, c) in top.chars().enumerate() {
                let mut s = String::new();
                s.push(c);
                g.set(ox + x, y, &s);
            }
            y += 1;
        }
        for (ri, row) in self.rows.iter().enumerate() {
            let mut col = ox;
            let put = |g: &mut Grid, col: &mut usize, s: &str| {
                for c in s.chars() {
                    let mut cell = String::new();
                    cell.push(c);
                    g.set(*col, y, &cell);
                    *col += 1;
                }
            };
            if self.borders {
                put(g, &mut col, "│");
            }
            for (i, w) in widths.iter().enumerate() {
                let text = row.get(i).map(|s| s.as_str()).unwrap_or("");
                let inner = w - self.cell_padding * 2;
                let t = truncate_to_width(text, inner);
                let fill = inner.saturating_sub(str_width(&t));
                put(g, &mut col, &pad);
                put(g, &mut col, &t);
                put(g, &mut col, &" ".repeat(fill));
                put(g, &mut col, &pad);
                if self.borders {
                    put(g, &mut col, if i + 1 == widths.len() { "│" } else { "│" });
                }
            }
            y += 1;
            let _ = ri;
        }
        if self.borders {
            let mut bot = String::from("└");
            for (i, w) in widths.iter().enumerate() {
                bot.push_str(&"─".repeat(*w));
                bot.push(if i + 1 == widths.len() { '┘' } else { '┴' });
            }
            for (x, c) in bot.chars().enumerate() {
                let mut s = String::new();
                s.push(c);
                g.set(ox + x, y, &s);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Slider
// ---------------------------------------------------------------------------

/// Value slider (Slider.ts: clamped value, fill ratio over length).
#[derive(Clone, Copy, Debug)]
pub struct SliderWidget {
    pub min: f64,
    pub max: f64,
    pub value: f64,
    pub length: usize,
    pub horizontal: bool,
}

impl SliderWidget {
    pub fn new(horizontal: bool, length: usize, min: f64, max: f64, value: f64) -> SliderWidget {
        let mut s = SliderWidget { min, max, value: min, length: length.min(MAX_GRID_DIM), horizontal };
        s.set_value(value);
        s
    }

    pub fn set_value(&mut self, v: f64) {
        let (lo, hi) = if self.max >= self.min { (self.min, self.max) } else { (self.max, self.min) };
        self.value = v.clamp(lo, hi);
        if self.value.is_nan() {
            self.value = lo;
        }
    }

    /// Fill ratio in [0,1]; degenerate range yields 0.
    pub fn fill_ratio(&self) -> f64 {
        let span = self.max - self.min;
        if !span.is_finite() || span <= 0.0 {
            return 0.0;
        }
        ((self.value - self.min) / span).clamp(0.0, 1.0)
    }

    pub fn filled_cells(&self) -> usize {
        (self.fill_ratio() * self.length as f64).round() as usize
    }

    pub fn render(&self, g: &mut Grid, ox: usize, oy: usize) {
        let filled = self.filled_cells().min(self.length);
        for i in 0..self.length {
            let (x, y) = if self.horizontal { (ox + i, oy) } else { (ox, oy + i) };
            g.set(x, y, if i < filled { "█" } else { "─" });
        }
    }
}

// ---------------------------------------------------------------------------
// render_to_grid dispatch
// ---------------------------------------------------------------------------

/// Any Phase-1 widget, dispatched into a fresh grid.
#[derive(Clone, Debug)]
pub enum Widget {
    Box(BoxWidget),
    Text(TextWidget),
    Input(InputWidget),
    Select(SelectWidget),
    Scroll(ScrollBoxWidget),
    ScrollBar(ScrollBarWidget),
    Table(TextTableWidget),
    Slider(SliderWidget),
}

/// Render any widget into a fresh grid of `width` x `height`.
pub fn render_to_grid(widget: &Widget, width: usize, height: usize) -> Grid {
    let mut g = Grid::new(width, height);
    let w = g.width;
    match widget {
        Widget::Box(b) => b.render(&mut g, 0, 0),
        Widget::Text(t) => t.render(&mut g, 0, 0),
        Widget::Input(i) => i.render(&mut g, 0, 0),
        Widget::Select(s) => s.render(&mut g, 0, 0, w),
        Widget::Scroll(s) => s.render(&mut g, 0, 0),
        Widget::ScrollBar(s) => s.render(&mut g, 0, 0),
        Widget::Table(t) => t.render(&mut g, 0, 0),
        Widget::Slider(s) => s.render(&mut g, 0, 0),
    }
    g
}

// ---------------------------------------------------------------------------
// Tests (frozen)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_single_borders() {
        let b = BoxWidget::new(5, 4, BorderStyle::Single);
        let mut g = Grid::new(5, 4);
        b.render(&mut g, 0, 0);
        assert_eq!(g.row_string(0), "┌───┐");
        assert_eq!(g.row_string(1), "│   │");
        assert_eq!(g.row_string(3), "└───┘");
    }

    #[test]
    fn box_double_borders() {
        let b = BoxWidget::new(4, 3, BorderStyle::Double);
        let mut g = Grid::new(4, 3);
        b.render(&mut g, 0, 0);
        assert_eq!(g.row_string(0), "╔══╗");
        assert_eq!(g.row_string(1), "║  ║");
        assert_eq!(g.row_string(2), "╚══╝");
    }

    #[test]
    fn box_rounded_heavy_and_title() {
        let mut b = BoxWidget::new(8, 3, BorderStyle::Rounded);
        b.title = "Hi".to_string();
        let mut g = Grid::new(8, 3);
        b.render(&mut g, 0, 0);
        assert_eq!(g.row_string(0), "╭Hi────╮");
        let h = BoxWidget::new(4, 3, BorderStyle::Heavy);
        let mut g2 = Grid::new(4, 3);
        h.render(&mut g2, 0, 0);
        assert_eq!(g2.row_string(0), "┏━━┓");
        assert_eq!(g2.row_string(2), "┗━━┛");
    }

    #[test]
    fn text_wrap_word() {
        let t = TextWidget::new(5, 4, "hello world");
        let lines = t.lines();
        assert_eq!(lines, vec!["hello".to_string(), "world".to_string()]);
    }

    #[test]
    fn text_align_center_and_right() {
        let mut t = TextWidget::new(7, 2, "hi");
        t.align = TextAlign::Center;
        let mut g = Grid::new(7, 1);
        t.render(&mut g, 0, 0);
        assert_eq!(g.row_string(0), "  hi   ");
        t.align = TextAlign::Right;
        let mut g2 = Grid::new(7, 1);
        t.render(&mut g2, 0, 0);
        assert_eq!(g2.row_string(0), "     hi");
    }

    #[test]
    fn input_insert_and_cursor_col() {
        let mut inp = InputWidget::new(10);
        inp.set_value("ab");
        inp.move_left();
        inp.insert('X');
        assert_eq!(inp.value(), "aXb");
        assert_eq!(inp.cursor, 2);
        assert_eq!(inp.cursor_col(), 2);
        // Newlines rejected like InputRenderable.
        inp.insert('\n');
        assert_eq!(inp.value(), "aXb");
    }

    #[test]
    fn input_backspace_and_delete() {
        let mut inp = InputWidget::new(10);
        inp.set_value("abc");
        inp.move_left();
        inp.backspace();
        assert_eq!(inp.value(), "ac");
        assert_eq!(inp.cursor, 1);
        inp.delete();
        assert_eq!(inp.value(), "a");
        // Backspace at 0 is a no-op.
        inp.move_left();
        inp.backspace();
        assert_eq!(inp.value(), "a");
    }

    #[test]
    fn select_move_highlight_and_wrap() {
        let mut s = SelectWidget::new(vec!["a".into(), "b".into(), "c".into()], 2);
        s.move_down();
        s.move_down();
        assert_eq!(s.selected, 2);
        // viewport of 2 scrolled so selection stays visible
        assert_eq!(s.visible(), vec![1, 2]);
        s.move_down(); // wraps
        assert_eq!(s.selected, 0);
        s.move_up(); // wraps to end
        assert_eq!(s.selected, 2);
        let mut g = Grid::new(6, 2);
        s.render(&mut g, 0, 0, 6);
        assert!(g.get(0, 1).unwrap().highlight);
    }

    #[test]
    fn tabselect_horizontal_render() {
        let mut s = SelectWidget::new(vec!["one".into(), "two".into()], 1);
        s.horizontal = true;
        s.move_right();
        assert_eq!(s.selected, 1);
        let mut g = Grid::new(12, 1);
        s.render(&mut g, 0, 0, 12);
        assert_eq!(g.row_string(0), " one  two   ");
        assert!(g.get(6, 0).unwrap().highlight);
        assert!(!g.get(1, 0).unwrap().highlight);
    }

    #[test]
    fn scrollbox_viewport_slice() {
        let lines: Vec<String> = (0..10).map(|i| format!("line{i}")).collect();
        let mut sb = ScrollBoxWidget::new(lines, 8, 3);
        sb.scroll_to(4);
        assert_eq!(sb.visible_lines(), &["line4".to_string(), "line5".to_string(), "line6".to_string()]);
        sb.scroll_by(100); // clamped to max
        assert_eq!(sb.scroll_y, 7);
        sb.scroll_by(-100);
        assert_eq!(sb.scroll_y, 0);
        let mut g = Grid::new(8, 3);
        sb.render(&mut g, 0, 0);
        assert_eq!(&g.row_string(0)[..5], "line0");
    }

    #[test]
    fn scrollbar_thumb_range() {
        let mut bar = ScrollBarWidget::new(true, 10);
        bar.scroll_size = 100;
        bar.viewport_size = 20;
        bar.set_position(80); // clamped to 100-20
        assert_eq!(bar.position, 80);
        bar.set_position(999);
        assert_eq!(bar.position, 80);
        let (start, len) = ScrollBarWidget { vertical: true, length: 10, scroll_size: 100, viewport_size: 50, position: 0 }.thumb_range();
        assert_eq!((start, len), (0, 5));
        let (s2, _) = ScrollBarWidget { vertical: true, length: 10, scroll_size: 100, viewport_size: 50, position: 50 }.thumb_range();
        assert_eq!(s2, 5);
    }

    #[test]
    fn table_widths_with_wide_chars() {
        // CJK char counts 2 wide: "日本" = 4 cols.
        let t = TextTableWidget::new(vec![
            vec!["a".into(), "日本".into()],
            vec!["xyz".into(), "b".into()],
        ]);
        assert_eq!(t.column_widths(), vec![3 + 2, 4 + 2]);
        // Fit shrinks largest-first down to min.
        let fit = t.fit_widths(8);
        assert!(fit.iter().sum::<usize>() + 1 <= 12);
        let mut g = Grid::new(20, 5);
        t.render(&mut g, 0, 0);
        assert_eq!(g.row_string(0), "┌─────┬──────┐      ");
    }

    #[test]
    fn slider_fill_ratio_and_clamp() {
        let mut s = SliderWidget::new(true, 10, 0.0, 100.0, 50.0);
        assert!((s.fill_ratio() - 0.5).abs() < 1e-9);
        assert_eq!(s.filled_cells(), 5);
        s.set_value(999.0);
        assert_eq!(s.value, 100.0);
        assert_eq!(s.filled_cells(), 10);
        s.set_value(-5.0);
        assert_eq!(s.filled_cells(), 0);
        let d = SliderWidget::new(true, 10, 5.0, 5.0, 5.0);
        assert_eq!(d.fill_ratio(), 0.0);
    }

    #[test]
    fn render_to_grid_dispatches_all_widgets() {
        let cases: Vec<Widget> = vec![
            Widget::Box(BoxWidget::new(4, 3, BorderStyle::Single)),
            Widget::Text(TextWidget::new(4, 2, "hi")),
            Widget::Input(InputWidget::new(4)),
            Widget::Select(SelectWidget::new(vec!["x".into()], 1)),
            Widget::Scroll(ScrollBoxWidget::new(vec!["y".into()], 4, 1)),
            Widget::ScrollBar(ScrollBarWidget::new(true, 2)),
            Widget::Table(TextTableWidget::new(vec![vec!["c".into()]])),
            Widget::Slider(SliderWidget::new(true, 4, 0.0, 1.0, 1.0)),
        ];
        for w in &cases {
            let g = render_to_grid(w, 8, 4);
            assert_eq!((g.width, g.height), (8, 4));
        }
        let g = render_to_grid(&Widget::Slider(SliderWidget::new(true, 4, 0.0, 1.0, 1.0)), 4, 1);
        assert_eq!(g.row_string(0), "████");
    }
}
