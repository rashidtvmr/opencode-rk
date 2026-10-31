#![forbid(unsafe_code)]
//! Native toast model and cell paint projection.
//!
//! Upstream authority (read-only checkout, pin
//! `450e7b92412c780a7b5a75226bb8d49aaca24e22`):
//! `packages/tui/src/ui/toast.tsx` and `packages/tui/src/ui/border.ts`.
//!
//! Behaviour copied from that source:
//! - one visible toast plus a FIFO queue; a new arrival replaces the current
//!   toast unless the current one is paused or something is already queued;
//! - default duration is 5000 ms; hover pauses the remaining lifetime and
//!   resume continues it;
//! - `activate()` dismisses first and runs the action afterwards;
//! - `dismiss()` promotes the queue head and leaves it paused when the paused
//!   flag is set;
//! - surface: absolute `top = 1`, `right = 2`, maximum width
//!   `min(60, cols - 6)`, raised-high background, left/right `┃` split
//!   borders coloured by the variant feedback role, horizontal padding 2 and
//!   vertical padding 1; an optional bold title has a one-row bottom gap; the
//!   action/close affordance is fixed at the right edge; a pending count adds
//!   a `+N more` row after a gap.
//!
//! Drawing is expressed as bounded [`CellRun`] spans carrying resolved RGBA
//! foreground/background plus OpenTUI text attributes, so the native painter
//! can issue its existing `fill_rect` + `draw_text_styled` calls without any
//! new dependency. Colours come from [`crate::native_theme_runtime::Palette`]
//! slots; the V2 palette has no cyan/info slot, so the info feedback colour is
//! the exact upstream `text.feedback.info.base` value (documented below).

use std::collections::VecDeque;

use crate::native_theme_runtime::{Palette, Slot, ThemeMode};
use crate::native_ui_style::ATTR_BOLD;
use crate::native_view_layout::visit_unpadded_cells;

/// Upstream `duration ?? 5000`.
pub const DEFAULT_DURATION_MS: u64 = 5_000;
/// Upstream paints exactly one toast surface; extra arrivals wait in a FIFO
/// queue (there is no multi-toast visual stack).
pub const MAX_VISIBLE_TOASTS: usize = 1;
/// `position="absolute" top={1}`.
pub const TOAST_TOP: u16 = 1;
/// `right={2}`.
pub const TOAST_RIGHT: u16 = 2;
/// `maxWidth={Math.min(60, dimensions().width - 6)}`.
pub const TOAST_MAX_WIDTH: u16 = 60;
/// `paddingLeft={2}` / `paddingRight={2}`.
pub const TOAST_HORIZONTAL_PADDING: u16 = 2;
/// `paddingTop={1}` / `paddingBottom={1}`.
pub const TOAST_VERTICAL_PADDING: u16 = 1;
/// `TextAttributes.BOLD` (`packages/core/src/types.ts`) from the shared
/// native attribute table.
pub const TEXT_ATTRIBUTE_BOLD: u32 = ATTR_BOLD;
/// `SplitBorder.customBorderChars.vertical` from `ui/border.ts`.
pub const SPLIT_BORDER_VERTICAL: char = '┃';
/// Horizontal chrome: two split borders plus both horizontal paddings.
pub const TOAST_CHROME: u16 = 2 + 2 * TOAST_HORIZONTAL_PADDING;
/// Upper bound on measured message cells (hostile plugin messages stay cheap).
pub const MAX_TEXT_CELLS: usize = 16_384;

/// Exact upstream dark `text.feedback.info.base` = `$hue.cyan.200` = #56b6c2.
pub const INFO_DARK: [u8; 4] = [0x56, 0xb6, 0xc2, 0xff];
/// Exact upstream light `text.feedback.info.base` = `$hue.cyan.200` = #318795.
pub const INFO_LIGHT: [u8; 4] = [0x31, 0x87, 0x95, 0xff];

/// Resolve the info feedback colour for a mode.
#[must_use]
pub fn info_color(mode: ThemeMode) -> [u8; 4] {
    match mode {
        ThemeMode::Dark => INFO_DARK,
        ThemeMode::Light => INFO_LIGHT,
    }
}

/// Toast variant; selects the feedback border colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastVariant {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastVariant {
    /// Every variant in declaration order.
    pub const ALL: [Self; 4] = [Self::Info, Self::Success, Self::Warning, Self::Error];
}

/// Resolved toast colours (RGBA bytes) from the runtime palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToastPalette {
    /// `text.feedback.info.base`; no cyan slot exists in [`Palette`].
    pub info: [u8; 4],
    /// `text.feedback.success.base` (palette `Success`).
    pub success: [u8; 4],
    /// `text.feedback.warning.base` (palette `Warning`).
    pub warning: [u8; 4],
    /// `text.feedback.error.base` (palette `Error`).
    pub error: [u8; 4],
    /// `text.base` (palette `Text`).
    pub text: [u8; 4],
    /// `text.muted` (palette `TextMuted`).
    pub muted: [u8; 4],
    /// `background.raised.high` (palette `BackgroundElement`).
    pub surface: [u8; 4],
}

impl ToastPalette {
    /// Resolve from the existing painter palette plus the runtime mode.
    #[must_use]
    pub fn from_palette(palette: &Palette, mode: ThemeMode) -> Self {
        Self {
            info: info_color(mode),
            success: palette.get(Slot::Success),
            warning: palette.get(Slot::Warning),
            error: palette.get(Slot::Error),
            text: palette.get(Slot::Text),
            muted: palette.get(Slot::TextMuted),
            surface: palette.get(Slot::BackgroundElement),
        }
    }

    /// Builtin upstream dark theme (`Palette::opencode_dark`).
    #[must_use]
    pub fn opencode_dark() -> Self {
        Self::from_palette(&Palette::opencode_dark(), ThemeMode::Dark)
    }

    /// Builtin upstream light theme (`Palette::opencode_light`).
    #[must_use]
    pub fn opencode_light() -> Self {
        Self::from_palette(&Palette::opencode_light(), ThemeMode::Light)
    }

    /// Border/feedback colour for a variant.
    #[must_use]
    pub fn variant_color(&self, variant: ToastVariant) -> [u8; 4] {
        match variant {
            ToastVariant::Info => self.info,
            ToastVariant::Success => self.success,
            ToastVariant::Warning => self.warning,
            ToastVariant::Error => self.error,
        }
    }
}

/// `action?: { label, run }` from upstream.
pub struct ToastAction {
    /// Affordance label (`› {label}`).
    pub label: String,
    run: Box<dyn FnMut()>,
}

impl ToastAction {
    /// Build an action from a label and callback.
    pub fn new(label: impl Into<String>, run: impl FnMut() + 'static) -> Self {
        Self {
            label: label.into(),
            run: Box::new(run),
        }
    }
}

/// `ToastOptions` from upstream (`title` and `action` optional).
pub struct ToastOptions {
    /// Optional bold title; when absent the message and affordance share a row.
    pub title: Option<String>,
    /// Message body (word wrapped).
    pub message: String,
    /// Variant feedback role.
    pub variant: ToastVariant,
    /// Auto-dismiss lifetime in milliseconds.
    pub duration: u64,
    /// Optional action affordance.
    pub action: Option<ToastAction>,
}

impl ToastOptions {
    /// New toast with the upstream default 5000 ms duration.
    pub fn new(variant: ToastVariant, message: impl Into<String>) -> Self {
        Self {
            title: None,
            message: message.into(),
            variant,
            duration: DEFAULT_DURATION_MS,
            action: None,
        }
    }

    /// Add a title.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Override the duration.
    #[must_use]
    pub fn with_duration(mut self, duration: u64) -> Self {
        self.duration = duration;
        self
    }

    /// Add an action.
    #[must_use]
    pub fn with_action(mut self, label: impl Into<String>, run: impl FnMut() + 'static) -> Self {
        self.action = Some(ToastAction::new(label, run));
        self
    }

    /// Affordance text: `› label` with an action, otherwise the close `x`.
    #[must_use]
    pub fn affordance(&self) -> String {
        match &self.action {
            Some(action) => format!("› {}", action.label),
            None => "x".to_owned(),
        }
    }
}

/// Toast controller mirroring `useToast()` from upstream.
///
/// Time is explicit: callers advance the logical clock with [`Toast::advance`],
/// which is the deterministic equivalent of the upstream `setTimeout`.
pub struct Toast {
    current: Option<ToastOptions>,
    queue: VecDeque<ToastOptions>,
    paused: bool,
    remaining: u64,
    started_at: u64,
    now: u64,
}

impl Default for Toast {
    fn default() -> Self {
        Self::new()
    }
}

impl Toast {
    /// Empty toast state at logical time 0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: None,
            queue: VecDeque::new(),
            paused: false,
            remaining: 0,
            started_at: 0,
            now: 0,
        }
    }

    /// Logical clock value in milliseconds.
    #[must_use]
    pub fn now(&self) -> u64 {
        self.now
    }

    /// Currently visible toast.
    #[must_use]
    pub fn current(&self) -> Option<&ToastOptions> {
        self.current.as_ref()
    }

    /// Number of queued toasts (`+N more`).
    #[must_use]
    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// True while hover has paused the lifetime.
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Remaining lifetime while paused (upstream `remaining`).
    #[must_use]
    pub fn remaining_ms(&self) -> u64 {
        self.remaining
    }

    /// `show(options)`: replace the current toast when idle, otherwise queue.
    pub fn show(&mut self, options: ToastOptions) {
        if self.current.is_some() && (self.paused || !self.queue.is_empty()) {
            self.queue.push_back(options);
            return;
        }
        self.current = Some(options);
        self.start_current();
    }

    /// Show an error toast (`toast.error`).
    pub fn error(&mut self, message: impl Into<String>) {
        self.show(ToastOptions::new(ToastVariant::Error, message));
    }

    /// Dismiss the current toast and promote the queue head.
    pub fn dismiss(&mut self) {
        let next = self.queue.pop_front();
        self.current = next;
        match &self.current {
            Some(toast) => {
                self.remaining = toast.duration;
                if !self.paused {
                    self.started_at = self.now;
                }
            }
            None => {
                self.remaining = 0;
                self.paused = false;
            }
        }
    }

    /// Pause the lifetime (hover in).
    pub fn pause(&mut self) {
        if self.current.is_none() || self.paused {
            return;
        }
        self.paused = true;
        self.remaining = self
            .remaining
            .saturating_sub(self.now.saturating_sub(self.started_at));
    }

    /// Resume the remaining lifetime (hover out).
    pub fn resume(&mut self) {
        if self.current.is_none() || !self.paused {
            return;
        }
        self.paused = false;
        self.started_at = self.now;
    }

    /// Activate the surface: dismiss first, then run the action.
    ///
    /// Returns the action label that ran, if any.
    pub fn activate(&mut self) -> Option<String> {
        let mut action = self.current.as_mut().and_then(|toast| toast.action.take());
        self.dismiss();
        let label = action.as_ref().map(|action| action.label.clone());
        if let Some(action) = action.as_mut() {
            (action.run)();
        }
        label
    }

    /// Advance the logical clock, firing every deadline crossed.
    pub fn advance(&mut self, ms: u64) {
        let target = self.now.saturating_add(ms);
        loop {
            if self.current.is_none() || self.paused {
                self.now = target;
                return;
            }
            let deadline = self.started_at.saturating_add(self.remaining);
            if target < deadline {
                self.now = target;
                return;
            }
            self.now = deadline;
            self.dismiss();
            if self.current.is_none() {
                self.now = target;
                return;
            }
        }
    }

    fn start_current(&mut self) {
        if let Some(toast) = &self.current {
            self.remaining = toast.duration;
            self.started_at = self.now;
        }
    }
}

/// One styled text span positioned in terminal cells.
///
/// The native painter draws this with `draw_text_styled(x, y, &text, fg,
/// Some(bg), attributes)` after its background fill.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellRun {
    /// Absolute terminal column of the first cell.
    pub x: u16,
    /// Text; its display width must equal the span width.
    pub text: String,
    /// Foreground RGBA.
    pub fg: [u8; 4],
    /// Background RGBA.
    pub bg: [u8; 4],
    /// OpenTUI text attributes (bit 0 = bold).
    pub attributes: u32,
}

/// One terminal row of styled spans, ordered left to right and tiling the
/// painted box exactly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellRow {
    /// Absolute terminal row.
    pub y: u16,
    /// Spans in painting order.
    pub runs: Vec<CellRun>,
}

impl CellRow {
    /// Concatenated span text (used by tests and selection bookkeeping).
    #[must_use]
    pub fn text(&self) -> String {
        self.runs.iter().map(|run| run.text.as_str()).collect()
    }
}

/// A fully positioned toast surface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToastFrame {
    /// Left column of the outer box.
    pub x: u16,
    /// Top row (`TOAST_TOP`, clipped by the viewport).
    pub y: u16,
    /// Outer width including both split borders.
    pub width: u16,
    /// Painted height (clipped by the viewport).
    pub height: u16,
    /// Surface background (raised high).
    pub surface: [u8; 4],
    /// Rows of spans; every run lies inside the box.
    pub rows: Vec<CellRow>,
}

impl ToastFrame {
    /// Render the current toast into bounded styled cells.
    ///
    /// Returns `None` when the viewport cannot host the minimum box (2 borders
    /// + 4 padding cells) or when the surface would start below the viewport.
    #[must_use]
    pub fn layout(
        toast: &ToastOptions,
        pending: usize,
        cols: u16,
        rows: u16,
        palette: &ToastPalette,
        hovered: bool,
    ) -> Option<Self> {
        let max_width = TOAST_MAX_WIDTH.min(cols.saturating_sub(6));
        if max_width < TOAST_CHROME || cols == 0 || rows <= TOAST_TOP {
            return None;
        }
        let content_max = usize::from(max_width).saturating_sub(usize::from(TOAST_CHROME));
        let has_title = toast.title.is_some();

        let affordance_full = toast.affordance();
        let affordance_natural = text_cells(&affordance_full);
        let message_natural = message_cells(&toast.message);
        let title_natural = toast.title.as_deref().map_or(0, text_cells);

        let desired = if has_title {
            title_natural
                .saturating_add(2)
                .saturating_add(affordance_natural)
                .max(message_natural)
        } else {
            message_natural
                .saturating_add(2)
                .saturating_add(affordance_natural)
        };
        let content_width = desired.min(content_max);
        let width = u16::try_from(content_width.saturating_add(usize::from(TOAST_CHROME))).ok()?;

        // Affordance is flexShrink 0 upstream; at hostile widths it is
        // truncated cluster-wise rather than overflowing the box.
        let affordance_budget = content_width.saturating_sub(3);
        let affordance_text = take_cells(&affordance_full, affordance_budget);
        let affordance_width = text_cells(&affordance_text);

        let (message_budget, title_text) = if has_title {
            let title_budget = content_width.saturating_sub(affordance_width.saturating_add(2));
            let title = toast
                .title
                .as_deref()
                .map(|title| take_cells(title, title_budget))
                .unwrap_or_default();
            (content_width, title)
        } else {
            (
                content_width.saturating_sub(affordance_width.saturating_add(2)),
                String::new(),
            )
        };

        let message_lines = wrap_cells(&toast.message, message_budget);
        let affordance_x = u16::try_from(
            usize::from(width)
                .saturating_sub(3)
                .saturating_sub(affordance_width),
        )
        .ok()?;

        let mut content_rows: Vec<Vec<Segment>> = Vec::new();
        if has_title {
            let mut title_row = vec![Segment {
                col: 3,
                text: title_text,
                fg: palette.text,
                bold: true,
            }];
            if !affordance_text.is_empty() {
                title_row.push(Segment {
                    col: affordance_x,
                    text: affordance_text.clone(),
                    fg: affordance_color(palette, hovered),
                    bold: hovered && toast.action.is_some(),
                });
            }
            content_rows.push(title_row);
            content_rows.push(Vec::new());
        }
        for (index, line) in message_lines.iter().enumerate() {
            let mut row = Vec::new();
            if !line.is_empty() {
                row.push(Segment {
                    col: 3,
                    text: line.clone(),
                    fg: palette.text,
                    bold: false,
                });
            }
            if !has_title && index == 0 && !affordance_text.is_empty() {
                row.push(Segment {
                    col: affordance_x,
                    text: affordance_text.clone(),
                    fg: affordance_color(palette, hovered),
                    bold: hovered && toast.action.is_some(),
                });
            }
            content_rows.push(row);
        }
        if pending > 0 {
            content_rows.push(Vec::new());
            content_rows.push(vec![Segment {
                col: 3,
                text: format!("+{pending} more"),
                fg: palette.muted,
                bold: false,
            }]);
        }

        let total_height = usize::from(TOAST_VERTICAL_PADDING)
            .saturating_mul(2)
            .saturating_add(content_rows.len());
        let x = cols.saturating_sub(TOAST_RIGHT).saturating_sub(width);
        let y = TOAST_TOP;
        let painted_height = total_height.min(usize::from(rows.saturating_sub(y)));
        if painted_height == 0 {
            return None;
        }
        let border_fg = palette.variant_color(toast.variant);
        let mut frame_rows = Vec::with_capacity(painted_height);
        for index in 0..painted_height {
            let row_y = y.saturating_add(u16::try_from(index).ok()?);
            let segments: &[Segment] = if index == 0 || index + 1 == total_height {
                &[]
            } else {
                content_rows
                    .get(index - usize::from(TOAST_VERTICAL_PADDING))
                    .map_or(&[], Vec::as_slice)
            };
            frame_rows.push(build_row(
                x,
                row_y,
                width,
                segments,
                palette.surface,
                border_fg,
                palette.text,
            ));
        }
        Some(Self {
            x,
            y,
            width,
            height: u16::try_from(painted_height).ok()?,
            surface: palette.surface,
            rows: frame_rows,
        })
    }

    /// Concatenated text of every row, top to bottom.
    #[must_use]
    pub fn text_rows(&self) -> Vec<String> {
        self.rows.iter().map(CellRow::text).collect()
    }
}

fn affordance_color(palette: &ToastPalette, hovered: bool) -> [u8; 4] {
    // `text.action.primary.base == text.base`; upstream uses it on hover.
    if hovered {
        palette.text
    } else {
        palette.muted
    }
}

struct Segment {
    col: u16,
    text: String,
    fg: [u8; 4],
    bold: bool,
}

/// Build one cell-exact box row: left border, interior padding/content, right
/// border. Runs never overlap and cover exactly `width` cells.
fn build_row(
    box_x: u16,
    y: u16,
    width: u16,
    segments: &[Segment],
    surface: [u8; 4],
    border_fg: [u8; 4],
    padding_fg: [u8; 4],
) -> CellRow {
    let mut runs = Vec::new();
    runs.push(CellRun {
        x: box_x,
        text: SPLIT_BORDER_VERTICAL.to_string(),
        fg: border_fg,
        bg: surface,
        attributes: 0,
    });
    let mut col = 1u16;
    let right_border = width.saturating_sub(1);
    for segment in segments {
        let start = segment.col.max(col).min(right_border);
        if start > col {
            push_spaces(&mut runs, box_x, col, start - col, padding_fg, surface);
            col = start;
        }
        let cells = u16::try_from(text_cells(&segment.text)).unwrap_or(u16::MAX);
        if col.saturating_add(cells) > right_border {
            break;
        }
        runs.push(CellRun {
            x: box_x.saturating_add(col),
            text: segment.text.clone(),
            fg: segment.fg,
            bg: surface,
            attributes: if segment.bold { TEXT_ATTRIBUTE_BOLD } else { 0 },
        });
        col = col.saturating_add(cells);
    }
    if col < right_border {
        push_spaces(
            &mut runs,
            box_x,
            col,
            right_border - col,
            padding_fg,
            surface,
        );
    }
    runs.push(CellRun {
        x: box_x.saturating_add(right_border),
        text: SPLIT_BORDER_VERTICAL.to_string(),
        fg: border_fg,
        bg: surface,
        attributes: 0,
    });
    CellRow { y, runs }
}

fn push_spaces(
    runs: &mut Vec<CellRun>,
    box_x: u16,
    col: u16,
    count: u16,
    fg: [u8; 4],
    bg: [u8; 4],
) {
    if count == 0 {
        return;
    }
    runs.push(CellRun {
        x: box_x.saturating_add(col),
        text: " ".repeat(usize::from(count)),
        fg,
        bg,
        attributes: 0,
    });
}

/// Display-cell width of a string using the shared native cell segmentation.
#[must_use]
pub fn text_cells(text: &str) -> usize {
    let mut width = 0;
    visit_unpadded_cells(text, MAX_TEXT_CELLS, |_, cells| width += cells);
    width
}

fn message_cells(message: &str) -> usize {
    message.split('\n').map(text_cells).max().unwrap_or(0)
}

/// Cluster-safe truncation to at most `budget` display cells.
#[must_use]
pub fn take_cells(text: &str, budget: usize) -> String {
    if budget == 0 {
        return String::new();
    }
    let mut out = String::new();
    visit_unpadded_cells(text, budget, |cluster, _| out.push_str(cluster));
    out
}

/// Greedy word wrap on display cells; words wider than the budget are
/// truncated on a cluster boundary. Explicit newlines start new lines.
#[must_use]
pub fn wrap_cells(text: &str, budget: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for raw in text.split('\n') {
        let mut line = String::new();
        let mut used = 0usize;
        for word in raw.split_whitespace() {
            let width = text_cells(word);
            if used == 0 {
                if width <= budget {
                    line.push_str(word);
                    used = width;
                } else {
                    line = take_cells(word, budget);
                    used = budget;
                }
                continue;
            }
            if used.saturating_add(1).saturating_add(width) <= budget {
                line.push(' ');
                line.push_str(word);
                used = used.saturating_add(1).saturating_add(width);
            } else {
                lines.push(std::mem::take(&mut line));
                if width <= budget {
                    line.push_str(word);
                    used = width;
                } else {
                    line = take_cells(word, budget);
                    used = budget;
                }
            }
        }
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    fn palette() -> ToastPalette {
        ToastPalette::opencode_dark()
    }

    fn row_span_width(row: &CellRow) -> u16 {
        row.runs
            .iter()
            .map(|run| u16::try_from(text_cells(&run.text)).unwrap())
            .sum()
    }

    #[test]
    fn variant_feedback_colors_match_upstream_slots() {
        let p = palette();
        assert_eq!(p.info, INFO_DARK);
        assert_eq!(p.success, [0x7f, 0xd8, 0x8f, 0xff]);
        assert_eq!(p.warning, [0xf5, 0xa7, 0x42, 0xff]);
        assert_eq!(p.error, [0xe0, 0x6c, 0x75, 0xff]);
        let colors: Vec<[u8; 4]> = ToastVariant::ALL
            .iter()
            .map(|variant| p.variant_color(*variant))
            .collect();
        for (index, color) in colors.iter().enumerate() {
            for other in colors.iter().skip(index + 1) {
                assert_ne!(color, other, "toast variants must be distinguishable");
            }
        }
        assert_eq!(
            ToastPalette::opencode_light().info,
            [0x31, 0x87, 0x95, 0xff]
        );
    }

    #[test]
    fn default_duration_and_idle_replace() {
        let mut toast = Toast::new();
        toast.show(ToastOptions::new(ToastVariant::Info, "first"));
        assert_eq!(toast.current().unwrap().duration, DEFAULT_DURATION_MS);
        toast.show(ToastOptions::new(ToastVariant::Success, "second"));
        assert_eq!(toast.current().unwrap().message, "second");
        assert_eq!(toast.pending(), 0);
    }

    #[test]
    fn paused_toast_queues_new_arrivals() {
        assert_eq!(MAX_VISIBLE_TOASTS, 1);
        let mut toast = Toast::new();
        toast.show(ToastOptions::new(ToastVariant::Info, "first"));
        toast.pause();
        toast.show(ToastOptions::new(ToastVariant::Info, "second"));
        toast.show(ToastOptions::new(ToastVariant::Info, "third"));
        assert_eq!(toast.current().unwrap().message, "first");
        assert_eq!(toast.pending(), 2);
    }

    #[test]
    fn replacement_restarts_the_lifetime_when_idle() {
        let mut toast = Toast::new();
        toast.show(ToastOptions::new(ToastVariant::Info, "first"));
        toast.advance(4_000);
        toast.show(ToastOptions::new(ToastVariant::Info, "second"));
        toast.advance(4_999);
        assert_eq!(toast.current().unwrap().message, "second");
        toast.advance(1);
        assert!(toast.current().is_none());
    }

    #[test]
    fn expires_at_exactly_five_seconds() {
        let mut toast = Toast::new();
        toast.show(ToastOptions::new(ToastVariant::Success, "copied"));
        toast.advance(4_999);
        assert!(toast.current().is_some());
        assert_eq!(toast.now(), 4_999);
        toast.advance(1);
        assert!(toast.current().is_none());
    }

    #[test]
    fn dismiss_advances_queue_fifo() {
        let mut toast = Toast::new();
        toast.show(ToastOptions::new(ToastVariant::Info, "first"));
        toast.pause();
        toast.show(ToastOptions::new(ToastVariant::Info, "second"));
        toast.show(ToastOptions::new(ToastVariant::Info, "third"));
        toast.dismiss();
        assert_eq!(toast.current().unwrap().message, "second");
        assert_eq!(toast.pending(), 1);
        toast.dismiss();
        assert_eq!(toast.current().unwrap().message, "third");
        assert_eq!(toast.pending(), 0);
        toast.dismiss();
        assert!(toast.current().is_none());
    }

    #[test]
    fn pause_preserves_remaining_lifetime_and_resume_expires() {
        let mut toast = Toast::new();
        toast.show(ToastOptions::new(ToastVariant::Info, "first"));
        toast.advance(2_000);
        toast.pause();
        assert_eq!(toast.remaining_ms(), 3_000);
        toast.advance(60_000);
        assert_eq!(toast.current().unwrap().message, "first");
        toast.resume();
        toast.advance(2_999);
        assert!(toast.current().is_some());
        toast.advance(1);
        assert!(toast.current().is_none());
    }

    #[test]
    fn activate_dismisses_before_running_action_and_keeps_queue_paused() {
        let calls = Rc::new(Cell::new(0));
        let observer = Rc::clone(&calls);
        let seen = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let seen_observer = Rc::clone(&seen);
        let mut toast = Toast::new();
        toast.show(
            ToastOptions::new(ToastVariant::Error, "Plugin failed").with_action(
                "Open plugins",
                move || {
                    observer.set(observer.get() + 1);
                    seen_observer.borrow_mut().push("ran".into());
                },
            ),
        );
        toast.pause();
        toast.show(ToastOptions::new(ToastVariant::Success, "Copied").with_duration(5));
        assert_eq!(toast.current().unwrap().message, "Plugin failed");
        assert_eq!(toast.pending(), 1);
        let label = toast.activate();
        assert_eq!(label.as_deref(), Some("Open plugins"));
        assert_eq!(calls.get(), 1);
        assert_eq!(toast.current().unwrap().message, "Copied");
        assert_eq!(toast.pending(), 0);
        assert!(toast.is_paused());
        toast.advance(100);
        assert!(toast.current().is_some());
        toast.resume();
        toast.advance(5);
        assert!(toast.current().is_none());
    }

    #[test]
    fn activate_without_action_only_dismisses() {
        let mut toast = Toast::new();
        toast.show(ToastOptions::new(ToastVariant::Success, "Copied").with_duration(5));
        assert!(toast.activate().is_none());
        assert!(toast.current().is_none());
    }

    #[test]
    fn surface_is_top_right_with_upstream_max_width() {
        let p = palette();
        let toast = ToastOptions::new(ToastVariant::Info, "a".repeat(100));
        let frame = ToastFrame::layout(&toast, 0, 120, 40, &p, false).unwrap();
        assert_eq!(frame.width, 60);
        assert_eq!(frame.x, 120 - 2 - 60);
        assert_eq!(frame.y, 1);
        let narrow = ToastFrame::layout(&toast, 0, 20, 40, &p, false).unwrap();
        assert_eq!(narrow.width, 14);
        assert_eq!(narrow.x, 20 - 2 - 14);
    }

    #[test]
    fn narrow_width_wraps_and_truncates_without_overflow() {
        let p = palette();
        let toast = ToastOptions::new(ToastVariant::Warning, "hello world oversized");
        let frame = ToastFrame::layout(&toast, 0, 20, 40, &p, false).unwrap();
        assert!(frame.width <= 14);
        for row in &frame.rows {
            assert_eq!(row_span_width(row), frame.width);
            for run in &row.runs {
                assert!(
                    run.x + u16::try_from(text_cells(&run.text)).unwrap() <= frame.x + frame.width
                );
            }
        }
        let text = frame.text_rows().join("\n");
        assert!(text.contains("hello"));
        assert!(text.contains('x'));
        assert!(!text.contains("oversized"));
    }

    #[test]
    fn actionable_affordance_is_fixed_and_hover_emphasizes_it() {
        let p = palette();
        let toast = ToastOptions::new(ToastVariant::Error, "Plugin failed")
            .with_action("Open plugins", || {});
        let idle = ToastFrame::layout(&toast, 0, 80, 24, &p, false).unwrap();
        let hovered = ToastFrame::layout(&toast, 0, 80, 24, &p, true).unwrap();
        let idle_run = idle.rows[1]
            .runs
            .iter()
            .find(|run| run.text.contains("›"))
            .unwrap();
        let hover_run = hovered.rows[1]
            .runs
            .iter()
            .find(|run| run.text.contains("›"))
            .unwrap();
        assert_eq!(idle_run.fg, p.muted);
        assert_eq!(idle_run.attributes, 0);
        assert_eq!(hover_run.fg, p.text);
        assert_eq!(hover_run.attributes, TEXT_ATTRIBUTE_BOLD);
        assert_eq!(
            idle_run.x + u16::try_from(text_cells(&idle_run.text)).unwrap(),
            idle.x + idle.width - 3
        );
    }

    #[test]
    fn title_has_bold_row_and_one_row_bottom_gap() {
        let p = palette();
        let toast = ToastOptions::new(ToastVariant::Info, "All good").with_title("Copied");
        let frame = ToastFrame::layout(&toast, 0, 80, 24, &p, false).unwrap();
        let title_run = frame.rows[1]
            .runs
            .iter()
            .find(|run| run.text == "Copied")
            .unwrap();
        assert_eq!(title_run.attributes, TEXT_ATTRIBUTE_BOLD);
        assert!(frame.rows[2]
            .text()
            .trim_matches(|ch| ch == ' ' || ch == SPLIT_BORDER_VERTICAL)
            .is_empty());
        assert!(frame.text_rows()[3].contains("All good"));
    }

    #[test]
    fn pending_count_adds_a_gap_and_more_row() {
        let p = palette();
        let toast = ToastOptions::new(ToastVariant::Info, "first");
        let frame = ToastFrame::layout(&toast, 3, 80, 24, &p, false).unwrap();
        let text = frame.text_rows();
        assert!(text.iter().any(|row| row.contains("+3 more")));
        let more_index = text.iter().position(|row| row.contains("+3 more")).unwrap();
        assert!(text[more_index - 1]
            .trim_matches(|ch| ch == ' ' || ch == SPLIT_BORDER_VERTICAL)
            .is_empty());
    }

    #[test]
    fn rows_tile_the_box_exactly() {
        let p = palette();
        for message in ["x", "hello world", "a".repeat(200).as_str()] {
            for cols in [14u16, 20, 40, 120] {
                let toast = ToastOptions::new(ToastVariant::Error, message).with_title("Title");
                let Some(frame) = ToastFrame::layout(&toast, 2, cols, 40, &p, false) else {
                    continue;
                };
                for (index, row) in frame.rows.iter().enumerate() {
                    assert_eq!(row.y, frame.y + u16::try_from(index).unwrap());
                    assert_eq!(row_span_width(row), frame.width);
                    let mut cursor = frame.x;
                    for run in &row.runs {
                        assert_eq!(run.x, cursor);
                        cursor += u16::try_from(text_cells(&run.text)).unwrap();
                    }
                    assert_eq!(cursor, frame.x + frame.width);
                }
                assert_eq!(frame.height, u16::try_from(frame.rows.len()).unwrap());
            }
        }
    }

    #[test]
    fn tiny_viewport_returns_no_surface() {
        let p = palette();
        let toast = ToastOptions::new(ToastVariant::Info, "hello");
        assert!(ToastFrame::layout(&toast, 0, 11, 40, &p, false).is_none());
        assert!(ToastFrame::layout(&toast, 0, 40, 1, &p, false).is_none());
    }

    #[test]
    fn viewport_clips_tall_toast_rows() {
        let p = palette();
        let toast = ToastOptions::new(ToastVariant::Info, "line ".repeat(40));
        let frame = ToastFrame::layout(&toast, 0, 30, 6, &p, false).unwrap();
        assert!(frame.y + frame.height <= 6);
        assert_eq!(frame.height, u16::try_from(frame.rows.len()).unwrap());
    }

    #[test]
    fn error_helper_uses_error_variant() {
        let mut toast = Toast::new();
        toast.error("boom");
        assert_eq!(toast.current().unwrap().variant, ToastVariant::Error);
        assert_eq!(toast.current().unwrap().duration, DEFAULT_DURATION_MS);
    }

    #[test]
    fn queue_survives_clock_advance_at_deadline_boundaries() {
        let mut toast = Toast::new();
        toast.show(ToastOptions::new(ToastVariant::Info, "first").with_duration(1_000));
        toast.pause();
        toast.show(ToastOptions::new(ToastVariant::Info, "second").with_duration(1_000));
        toast.resume();
        toast.advance(1_500);
        assert_eq!(toast.current().unwrap().message, "second");
        toast.advance(499);
        assert!(toast.current().is_some());
        toast.advance(1);
        assert!(toast.current().is_none());
    }

    #[test]
    fn wrap_and_take_use_display_cells() {
        assert_eq!(text_cells("ab"), 2);
        assert_eq!(text_cells("界"), 2);
        assert_eq!(take_cells("abcdef", 3), "abc");
        let lines = wrap_cells("aa bb cc", 5);
        assert_eq!(lines, vec!["aa bb".to_string(), "cc".to_string()]);
        assert_eq!(wrap_cells("", 5), vec![String::new()]);
    }
}
