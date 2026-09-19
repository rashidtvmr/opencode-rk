#![forbid(unsafe_code)]
//! Derived abstract shapes over `CellBuffer` (no new FFI). Pure, bounded, no IO.

/// Max cells in a [`Bar`].
pub const MAX_BAR_LEN: usize = 256;
/// Max points in a [`Sparkline`].
pub const MAX_SPARKLINE_POINTS: usize = 128;
/// Max items in a [`Menu`].
pub const MAX_MENU_ITEMS: usize = 64;
/// Max chars per menu label (truncated).
pub const MAX_MENU_LABEL: usize = 64;
/// Max outer width of a [`Card`].
pub const MAX_CARD_WIDTH: usize = 128;
/// Max chars kept for card title (truncated).
pub const MAX_CARD_TITLE: usize = 96;
/// Max chars kept for card body (truncated).
pub const MAX_CARD_BODY: usize = 1024;

/// Bar direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarOrientation {
    Horizontal,
    Vertical,
}

/// Progress bar, fill ratio `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bar {
    pub orientation: BarOrientation,
    pub len: usize,
    pub ratio: f32,
}

impl Bar {
    /// Fail-closed: `None` on out-of-range ratio (incl. NaN), zero/over-bound len.
    #[must_use]
    pub fn new(orientation: BarOrientation, len: usize, ratio: f32) -> Option<Self> {
        if !(0.0..=1.0).contains(&ratio) || len == 0 || len > MAX_BAR_LEN {
            return None;
        }
        Some(Self { orientation, len, ratio })
    }

    /// Filled cells, rounded, clamped to `len`.
    #[must_use]
    pub fn filled(&self) -> usize {
        (self.ratio * self.len as f32).round() as usize % (self.len + 1)
    }

    /// `█` filled / `░` empty.
    #[must_use]
    pub fn render(&self) -> String {
        let n = self.filled();
        core::iter::repeat_n('█', n)
            .chain(core::iter::repeat_n('░', self.len - n))
            .collect()
    }
}

pub const SPARK_BLOCKS: [char; 8] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '█'];

/// Bounded point series, min/max normalized on render.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sparkline {
    points: Vec<f32>,
}

impl Sparkline {
    /// Fail-closed: `None` on empty or over-bound input.
    #[must_use]
    pub fn new(points: &[f32]) -> Option<Self> {
        render_slice(points).map(|_| Self { points: points.to_vec() })
    }

    /// One block char per point; `""` when empty. Flat series → mid block.
    #[must_use]
    pub fn render(&self) -> String {
        render_slice(&self.points).unwrap_or_default()
    }
}

/// Slice form: `None` on empty/over-bound/non-finite, else block string.
#[must_use]
pub fn render_slice(points: &[f32]) -> Option<String> {
    if points.is_empty() || points.len() > MAX_SPARKLINE_POINTS {
        return None;
    }
    let min = points.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max = points.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    if !(min.is_finite() && max.is_finite()) {
        return None;
    }
    if (max - min) < f32::EPSILON {
        return Some(core::iter::repeat_n(SPARK_BLOCKS[4], points.len()).collect());
    }
    Some(points.iter().map(|&v| SPARK_BLOCKS[((v - min) / (max - min) * 7.0).round() as usize % 8]).collect())
}

/// Item list with clamped selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu {
    items: Vec<String>,
    selected: usize,
}

impl Menu {
    /// Truncates count to [`MAX_MENU_ITEMS`], labels to [`MAX_MENU_LABEL`] chars.
    #[must_use]
    pub fn new(items: &[&str]) -> Self {
        Self {
            items: items.iter().take(MAX_MENU_ITEMS).map(|s| s.chars().take(MAX_MENU_LABEL).collect()).collect(),
            selected: 0,
        }
    }

    /// Clamp to last item; empty menu stays 0.
    pub fn select(&mut self, index: usize) {
        self.selected = index.min(self.items.len().saturating_sub(1));
    }

    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    #[must_use]
    pub fn selected_item(&self) -> Option<&str> {
        self.items.get(self.selected).map(String::as_str)
    }
}

/// Titled body rect with optional border.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    title: String,
    body: String,
    width: usize,
    bordered: bool,
}

impl Card {
    /// Fail-closed on zero/over-bound width; title/body truncated to fit.
    #[must_use]
    pub fn new(title: &str, body: &str, width: usize, bordered: bool) -> Option<Self> {
        if width == 0 || width > MAX_CARD_WIDTH {
            return None;
        }
        let inner = if bordered { width.saturating_sub(2) } else { width }.max(1);
        Some(Self {
            title: title.chars().take(MAX_CARD_TITLE).take(inner).collect(),
            body: body.chars().take(MAX_CARD_BODY).collect(),
            width,
            bordered,
        })
    }

    fn inner(&self) -> usize {
        if self.bordered { self.width.saturating_sub(2).max(1) } else { self.width }
    }

    fn pad_line(&self, text: &str) -> String {
        let mut line: String = text.chars().take(self.inner()).collect();
        while line.chars().count() < self.inner() {
            line.push(' ');
        }
        if self.bordered { format!("│{line}│") } else { line }
    }

    /// Lines incl. border when `bordered`; body wraps at inner width.
    #[must_use]
    pub fn render(&self) -> Vec<String> {
        let inner = self.inner().max(1);
        let mut out = Vec::new();
        if self.bordered {
            out.push(format!("┌{}┐", "─".repeat(inner)));
        }
        out.push(self.pad_line(&self.title));
        let chars: Vec<char> = self.body.chars().collect();
        if chars.is_empty() {
            out.push(self.pad_line(""));
        }
        for chunk in chars.chunks(inner) {
            out.push(self.pad_line(&chunk.iter().collect::<String>()));
        }
        if self.bordered {
            out.push(format!("└{}┘", "─".repeat(inner)));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_ratio_clamp() {
        assert!(Bar::new(BarOrientation::Horizontal, 8, -0.1).is_none());
        assert!(Bar::new(BarOrientation::Horizontal, 8, 1.1).is_none());
        assert!(Bar::new(BarOrientation::Horizontal, 8, f32::NAN).is_none());
        assert!(Bar::new(BarOrientation::Horizontal, 0, 0.5).is_none());
        assert!(Bar::new(BarOrientation::Horizontal, MAX_BAR_LEN + 1, 0.5).is_none());
        let bar = Bar::new(BarOrientation::Horizontal, 8, 0.5).unwrap();
        assert_eq!(bar.filled(), 4);
        assert_eq!(bar.render().chars().count(), 8);
        assert_eq!(Bar::new(BarOrientation::Vertical, 4, 1.0).unwrap().filled(), 4);
    }

    #[test]
    fn sparkline_empty() {
        assert_eq!(render_slice(&[]), None);
        assert_eq!(Sparkline::new(&[]), None);
        assert_eq!(Sparkline::default().render(), "");
        assert_eq!(render_slice(&[1.0, 2.0, 3.0]).unwrap(), " ▄█");
        assert_eq!(render_slice(&[5.0, 5.0]).unwrap(), "▄▄");
        assert!(Sparkline::new(&[0.0; MAX_SPARKLINE_POINTS + 1]).is_none());
    }

    #[test]
    fn menu_select_clamp() {
        let mut menu = Menu::new(&["a", "b", "c"]);
        menu.select(99);
        assert_eq!((menu.selected_index(), menu.selected_item()), (2, Some("c")));
        menu.select(1);
        assert_eq!(menu.selected_item(), Some("b"));
        let mut empty = Menu::new(&[]);
        empty.select(5);
        assert_eq!((empty.selected_index(), empty.selected_item()), (0, None));
    }

    #[test]
    fn card_truncation() {
        assert!(Card::new("t", "b", 0, true).is_none());
        assert!(Card::new("t", "b", MAX_CARD_WIDTH + 1, false).is_none());
        let lines = Card::new("longtitle", "abcdef", 5, true).unwrap().render();
        assert_eq!(lines[0], "┌───┐");
        assert!(lines.iter().all(|l| l.chars().count() == 5));
        assert_eq!(lines[1], "│lon│");
        assert_eq!(Card::new("hi", "xy", 4, false).unwrap().render(), vec!["hi  ", "xy  "]);
    }
}
