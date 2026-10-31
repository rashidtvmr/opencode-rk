#![forbid(unsafe_code)]
//! Session-frame row composition (work package P03).
//!
//! Upstream pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`
//! (`packages/tui/src/`):
//!
//! * `component/session-frame.tsx:107-140` — `wide`, `sidebarVisible` and the
//!   `rightPane()` precedence;
//! * `component/session-frame.tsx:261-398` — a growing `session-pane` column
//!   plus at most one right box: `SESSION_SIDEBAR_WIDTH` for the sidebar,
//!   `paneResize.size()` for a terminal/panel, and an absolute right-aligned
//!   sidebar overlay when the viewport is not wide;
//! * `routes/session/index.tsx:1510-1592` — the session column insets 1 (below
//!   44 terminal columns) or 2, reserves `paddingBottom={1}`, an always-present
//!   `height={1}` action row between the transcript scrollbox and the
//!   composer, and a transcript scrollbar that reserves the viewport
//!   `paddingRight` plus a one-column track;
//! * `routes/session/sidebar.tsx:20-76` — the 42-column sidebar pads 1/1/2/2,
//!   pins its title, scrolls its middle against an ABSOLUTE one-column
//!   scrollbar over the content's reserved `paddingRight={1}`, and pins its
//!   footer.
//!
//! Geometry lives in [`crate::native_layout::SessionRegions`]; width math and
//! predicates live in [`crate::native_view_layout`]. This module only turns
//! caller-owned lines into rows whose display width is exactly `cols`, so
//! painting the frame clears the previous region cell-exactly.
//!
//! Text styling (colors, the hovered jump label, the shimmer title) is owned
//! by the styling packages; this module returns plain text cells. Terminal and
//! panel content is owned by P04/P11, so their dock is reserved as blank cells
//! rather than painted by a second pane.

use crate::native_layout::{FrameSpec, Rect, SessionRegions};
use crate::native_view_layout::{
    RightPane, SidebarMode, fit, right_pane, session_wide, sidebar_visible, slice_cells,
    visit_unpadded_cells,
};

/// Jump/loading label rendered in the reserved action row
/// (`routes/session/index.tsx:1572`).
pub const LOADING_LABEL: &str = "Loading session history\u{2026}";
/// Jump label rendered in the reserved action row
/// (`routes/session/index.tsx:1585`).
pub const JUMP_LABEL: &str = "Jump to latest \u{2193}";
/// Bounded frame cells (`cols * rows`), so a hostile viewport cannot allocate
/// unbounded memory.
const MAX_FRAME_CELLS: u32 = 1 << 20;

/// Sidebar facts plus caller-owned sidebar lines.
#[derive(Clone, Copy, Debug)]
pub struct SidebarInput<'a> {
    /// `sidebarOpen()` — the explicit toggle signal.
    pub open: bool,
    /// `config.session.sidebar` (`auto` by default).
    pub mode: SidebarMode,
    /// `data.session.get(id)?.parentID` — child sessions never show it.
    pub child_session: bool,
    /// Session title rendered in the pinned title box.
    pub title: &'a str,
    /// `sidebar.content` rows, already wrapped by the sidebar owner.
    pub content: &'a [String],
    /// `sidebar.footer` rows.
    pub footer: &'a [String],
    /// First visible content row (the sidebar scrolls independently).
    pub scroll: usize,
}

impl<'a> SidebarInput<'a> {
    /// An empty, closed sidebar.
    #[must_use]
    pub const fn closed() -> SidebarInput<'a> {
        SidebarInput {
            open: false,
            mode: SidebarMode::Auto,
            child_session: false,
            title: "",
            content: &[],
            footer: &[],
            scroll: 0,
        }
    }
}

/// State of the always-reserved action row below the transcript
/// (`routes/session/index.tsx:1570-1589`). The row itself is reserved in every
/// state; only its contents change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JumpAction {
    /// Nothing to show: the row stays blank but reserved.
    None,
    /// `firstJump()` — "Loading session history…".
    Loading,
    /// `awayFromBottom()` — "Jump to latest ↓".
    AwayFromBottom,
}

impl JumpAction {
    /// The label drawn in the row, if any.
    #[must_use]
    pub const fn label(self) -> Option<&'static str> {
        match self {
            JumpAction::None => None,
            JumpAction::Loading => Some(LOADING_LABEL),
            JumpAction::AwayFromBottom => Some(JUMP_LABEL),
        }
    }
}

/// Everything the session frame paints, as caller-owned snapshots.
#[derive(Clone, Copy, Debug)]
pub struct ShellFrameInput<'a> {
    /// Terminal width in cells.
    pub cols: u16,
    /// Terminal height in rows.
    pub rows: u16,
    /// Vertical tab rail width; 0 when tabs are horizontal or hidden.
    pub tabs_width: u16,
    /// Horizontal tab strip height; 0 when vertical or hidden.
    pub tabs_height: u16,
    /// An open panel takes the right pane (`fullscreen` panels own the frame
    /// and are not composed here).
    pub panel_open: bool,
    /// A terminal is selected for the right pane.
    pub terminal_selected: bool,
    /// Docked terminal/panel width (`paneResize.size()`).
    pub pane_width: u16,
    /// Sidebar state and content.
    pub sidebar: SidebarInput<'a>,
    /// Transcript rows, already wrapped to the content width.
    pub transcript: &'a [String],
    /// First visible transcript row.
    pub transcript_scroll: usize,
    /// Composer block rows, pinned at the bottom of the session column.
    pub composer: &'a [String],
    /// Reserved action row state.
    pub jump: JumpAction,
    /// `config.session.scrollbar`.
    pub scrollbar: bool,
}

impl<'a> ShellFrameInput<'a> {
    /// A frame with no tabs, no panes, no composer and no transcript.
    #[must_use]
    pub const fn new(cols: u16, rows: u16) -> ShellFrameInput<'a> {
        ShellFrameInput {
            cols,
            rows,
            tabs_width: 0,
            tabs_height: 0,
            panel_open: false,
            terminal_selected: false,
            pane_width: 0,
            sidebar: SidebarInput::closed(),
            transcript: &[],
            transcript_scroll: 0,
            composer: &[],
            jump: JumpAction::None,
            scrollbar: false,
        }
    }
}

/// A composed frame: `cols` cells per row and at most `rows` rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellFrame {
    /// Painted rows, each exactly the terminal width in display cells.
    pub rows: Vec<String>,
    /// The single frame descriptor that produced the rows.
    pub descriptor: FrameDescriptor,
    /// The geometry that produced the rows.
    pub regions: SessionRegions,
    /// `dimensions().width - verticalTabsWidth > 120`.
    pub wide: bool,
    /// The single right pane in use.
    pub right: RightPane,
    /// Resolved `sidebarVisible()`.
    pub sidebar_visible: bool,
}

/// The single frame descriptor: every geometry fact the painter, the resize
/// handle and the chrome layers need, computed exactly once per frame.
///
/// Upstream derives these from flex layout rather than arithmetic, so the
/// numbers follow the same source of truth:
///
/// * the rail reservation is a sibling of the session frame in a row flex box
///   (`app.tsx:1422-1434`), so [`FrameDescriptor::available`] is the frame box
///   the resize handle and the right dock are placed in;
/// * the right dock is one box of `SESSION_SIDEBAR_WIDTH` for the sidebar and
///   `paneResize.size()` for a terminal/panel (`session-frame.tsx:323-331`);
/// * the session content insets by 1 (<44 terminal) or 2 and reserves one
///   bottom padding row plus an always-present action row
///   (`routes/session/index.tsx:1514-1520,1570`);
/// * the transcript scrollbar reserves the viewport `paddingRight` plus a
///   one-column track while shown (`routes/session/index.tsx:1527-1537`);
/// * the sidebar pads 1/1/2/2 and its content reserves one right column under
///   the ABSOLUTE scrollbar (`routes/session/sidebar.tsx:26-29,56-69`).
///
/// Consumers must read these fields instead of subtracting rectangles again.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameDescriptor {
    /// Terminal width in cells.
    pub cols: u16,
    /// Rows the frame emits.
    pub rows: u16,
    /// `dimensions().width - verticalTabsWidth > 120`.
    pub wide: bool,
    /// The single resolved right pane.
    pub pane: RightPane,
    /// Resolved `sidebarVisible()`.
    pub sidebar_visible: bool,
    /// Tab reservation: the vertical rail or the horizontal strip.
    pub rail: Rect,
    /// Frame box after the rail: the resize handle spans it and the right dock
    /// sits on its right edge (`session-frame.tsx:381-384`).
    pub available: Rect,
    /// Docked right box; zero when there is no docked pane.
    pub right: Rect,
    /// Sidebar box (docked or absolute overlay).
    pub sidebar: Rect,
    /// True when the sidebar is painted as an absolute overlay.
    pub sidebar_overlay: bool,
    /// Left inset of the session content (`index.tsx:1515-1516`).
    pub inset_left: u16,
    /// Right inset of the session content.
    pub inset_right: u16,
    /// Painted content width (transcript viewport, scrollbar cells included).
    pub content_width: u16,
    /// Cells the transcript scrollbar reserves while it is shown.
    pub scrollbar_cells: u16,
    /// Sidebar outer left padding (`sidebar.tsx:28`).
    pub sidebar_pad_left: u16,
    /// Sidebar content box width, padding excluded.
    pub sidebar_inner_width: u16,
    /// Sidebar text width under the reserved scrollbar column.
    pub sidebar_text_width: u16,
    /// Sidebar outer right padding (`sidebar.tsx:29`).
    pub sidebar_pad_right: u16,
    /// Rows of the always-reserved action strip (`index.tsx:1570`).
    pub action_rows: u16,
    /// Rows of the reserved `paddingBottom={1}` strip (`index.tsx:1514`).
    pub bottom_pad_rows: u16,
    /// The full region model these scalars were derived from.
    pub regions: SessionRegions,
}

impl FrameDescriptor {
    /// Resolve the whole frame geometry from one [`ShellFrameInput`].
    #[must_use]
    pub fn new(input: &ShellFrameInput<'_>) -> FrameDescriptor {
        let cols = input.cols;
        let rows = input.rows;
        let wide = session_wide(cols, input.tabs_width);
        let sidebar_visible = sidebar_visible(
            input.sidebar.child_session,
            input.sidebar.open,
            input.sidebar.mode,
            wide,
        );
        let pane = right_pane(
            input.panel_open,
            input.sidebar.open,
            sidebar_visible,
            input.terminal_selected,
        );
        let spec = FrameSpec {
            cols,
            rows,
            tabs_width: input.tabs_width,
            tabs_height: input.tabs_height,
            right: pane,
            pane_width: input.pane_width,
            wide,
            composer_height: u16::try_from(input.composer.len()).unwrap_or(u16::MAX),
            sidebar_title_height: u16::from(!input.sidebar.title.is_empty()),
            sidebar_footer_height: u16::try_from(input.sidebar.footer.len()).unwrap_or(u16::MAX),
            scrollbar: input.scrollbar,
        };
        let regions = SessionRegions::compute(&spec);
        let inset_left = regions
            .transcript_text
            .x
            .saturating_sub(regions.session.x)
            .min(regions.session.w);
        let content_width = regions.transcript.w.min(regions.session.w);
        let inset_right = regions
            .session
            .w
            .saturating_sub(inset_left)
            .saturating_sub(content_width);
        // The sidebar content box is pinned by the title/footer rects
        // (`sidebar.tsx:32,74`); derive its pads once so no row recomputes.
        let sidebar_pad_left = regions
            .sidebar_title
            .x
            .saturating_sub(regions.sidebar.x)
            .min(regions.sidebar.w);
        let sidebar_inner_width = regions
            .sidebar_title
            .w
            .min(regions.sidebar.w.saturating_sub(sidebar_pad_left));
        let sidebar_text_width = regions.sidebar_text_width().min(sidebar_inner_width);
        let sidebar_pad_right = regions
            .sidebar
            .w
            .saturating_sub(sidebar_pad_left)
            .saturating_sub(sidebar_inner_width);
        FrameDescriptor {
            cols,
            rows,
            wide,
            pane,
            sidebar_visible,
            rail: regions.tabs,
            available: regions.available,
            right: regions.right,
            sidebar: regions.sidebar,
            sidebar_overlay: regions.sidebar_overlay,
            inset_left,
            inset_right,
            content_width,
            scrollbar_cells: regions.transcript.w.saturating_sub(regions.transcript_text.w),
            sidebar_pad_left,
            sidebar_inner_width,
            sidebar_text_width,
            sidebar_pad_right,
            action_rows: regions.jump.h,
            bottom_pad_rows: regions.bottom_pad.h,
            regions,
        }
    }

    /// Painted content columns: `inset_left + content_width + inset_right`
    /// always equals the session column width.
    #[must_use]
    pub fn content_rect(&self) -> Rect {
        self.regions.transcript_text
    }
}

/// Append-only row builder that tracks its current cell position.
struct Row {
    cells: String,
    x: u16,
}

impl Row {
    fn new() -> Row {
        Row {
            cells: String::new(),
            x: 0,
        }
    }

    /// Pad with blank cells up to column `x`.
    fn pad_to(&mut self, x: u16) {
        while self.x < x {
            self.cells.push(' ');
            self.x += 1;
        }
    }

    /// Append `width` cells of `text` (truncated/padded on cluster boundaries).
    fn text(&mut self, text: &str, width: u16) {
        if width == 0 {
            return;
        }
        self.cells.push_str(&fit(text, width));
        self.x = self.x.saturating_add(width);
    }

    fn blank(&mut self, width: u16) {
        self.text("", width);
    }
}

/// Display cells of `line`, bounded by the module's own scanner.
fn cells(line: &str) -> u16 {
    u16::try_from(visit_unpadded_cells(line, usize::MAX, |_, _| {})).unwrap_or(u16::MAX)
}

/// Compose the session frame.
///
/// The paint order matches upstream: tab reservation, session column, docked
/// right pane, then (when not wide) the absolute sidebar overlay. Every row is
/// normalized to exactly `cols` cells, so no pane can overflow the viewport.
/// All geometry comes from one [`FrameDescriptor`]; nothing is recomputed here.
#[must_use]
pub fn compose_shell(input: &ShellFrameInput<'_>) -> ShellFrame {
    let descriptor = FrameDescriptor::new(input);
    let drawn = bounded_rows(descriptor.cols, descriptor.rows);
    let mut out = Vec::with_capacity(usize::from(drawn));
    for y in 0..drawn {
        let session_segment = session_row(&descriptor, input, y);
        let mut row = Row::new();
        row.pad_to(descriptor.regions.session.x);
        if descriptor.sidebar_overlay
            && y >= descriptor.sidebar.y
            && y < descriptor.sidebar.y + descriptor.sidebar.h
        {
            // Absolute overlay (`session-frame.tsx:385-397`): it covers the
            // rightmost sidebar columns of the session column.
            let cut = descriptor
                .sidebar
                .x
                .saturating_sub(descriptor.regions.session.x)
                .min(descriptor.regions.session.w);
            row.text(&slice_cells(&session_segment, 0, cut), cut);
            row.text(&sidebar_row(&descriptor, input, y), descriptor.sidebar.w);
        } else {
            row.text(&session_segment, descriptor.regions.session.w);
            if descriptor.right.w > 0 {
                row.text(&right_row(&descriptor, input, y), descriptor.right.w);
            }
        }
        // Trust-boundary normalization: any arithmetic slip still yields an
        // exactly-wide row instead of overlapping the next pane.
        out.push(fit(&row.cells, descriptor.cols));
    }
    ShellFrame {
        rows: out,
        descriptor,
        regions: descriptor.regions,
        wide: descriptor.wide,
        right: descriptor.pane,
        sidebar_visible: descriptor.sidebar_visible,
    }
}

/// Rows actually emitted for a `cols x rows` viewport, capped by
/// [`MAX_FRAME_CELLS`].
fn bounded_rows(cols: u16, rows: u16) -> u16 {
    if cols == 0 {
        return rows;
    }
    let budget = MAX_FRAME_CELLS / u32::from(cols);
    let budget = u16::try_from(budget).unwrap_or(u16::MAX);
    rows.min(budget.max(1))
}

/// One session-column row, exactly `regions.session.w` cells wide:
/// `inset_left + content + inset_right`.
fn session_row(descriptor: &FrameDescriptor, input: &ShellFrameInput<'_>, y: u16) -> String {
    let content = content_row(descriptor, input, y);
    let mut row = Row::new();
    row.blank(descriptor.inset_left);
    row.text(&content, descriptor.content_width);
    row.blank(descriptor.inset_right);
    row.cells
}

/// The content segment for one row: transcript, reserved action row, composer
/// or blank.
fn content_row(descriptor: &FrameDescriptor, input: &ShellFrameInput<'_>, y: u16) -> String {
    let regions = &descriptor.regions;
    let width = descriptor.content_width;
    if regions.transcript.h > 0 && y >= regions.transcript.y && y < regions.transcript.y + regions.transcript.h {
        let index = usize::from(y - regions.transcript.y).saturating_add(input.transcript_scroll);
        let line = input.transcript.get(index).map_or("", String::as_str);
        let text_w = regions.transcript_text.w.min(width);
        let mut row = Row::new();
        row.text(line, text_w);
        row.blank(width.saturating_sub(text_w));
        return row.cells;
    }
    if regions.jump.h > 0 && y >= regions.jump.y && y < regions.jump.y + regions.jump.h {
        return jump_row(input.jump, width);
    }
    if regions.composer.h > 0 && y >= regions.composer.y && y < regions.composer.y + regions.composer.h {
        let index = usize::from(y - regions.composer.y);
        return fit(input.composer.get(index).map_or("", String::as_str), width);
    }
    " ".repeat(usize::from(width))
}

/// The reserved action row (`routes/session/index.tsx:1570-1589`).
///
/// The row is a `flexDirection="row" justifyContent="flex-end"` box, so every
/// state is RIGHT aligned: the loading text is flush right on its own, and the
/// jump box adds `paddingLeft={1}` (`index.tsx:1577`) so its label keeps one
/// blank cell to its left. An empty state stays reserved but blank.
fn jump_row(action: JumpAction, width: u16) -> String {
    let Some(label) = action.label() else {
        return " ".repeat(usize::from(width));
    };
    let lead = match action {
        JumpAction::AwayFromBottom => 1,
        _ => 0,
    };
    let label_cells = cells(label);
    let mut row = Row::new();
    let room = width.saturating_sub(lead);
    if label_cells >= room {
        row.blank(lead);
        row.text(label, room);
        return row.cells;
    }
    row.blank(room - label_cells);
    row.blank(lead);
    row.text(label, label_cells);
    row.cells
}

/// The docked right pane row. Sidebar content is composed here; terminal and
/// panel content is owned by P04/P11, so their dock stays blank (one pane).
fn right_row(descriptor: &FrameDescriptor, input: &ShellFrameInput<'_>, y: u16) -> String {
    match descriptor.pane {
        RightPane::Sidebar => sidebar_row(descriptor, input, y),
        RightPane::Terminal | RightPane::Panel | RightPane::None => {
            " ".repeat(usize::from(descriptor.right.w))
        }
    }
}

/// One sidebar box row, exactly `descriptor.sidebar.w` cells wide:
/// `pad_left + content + pad_right`, with the content box and text width taken
/// from the descriptor rather than recomputed.
fn sidebar_row(descriptor: &FrameDescriptor, input: &ShellFrameInput<'_>, y: u16) -> String {
    let regions = &descriptor.regions;
    if descriptor.sidebar.w == 0 {
        return fit("", descriptor.sidebar.w);
    }
    let inner_w = descriptor.sidebar_inner_width;
    let mut row = Row::new();
    row.blank(descriptor.sidebar_pad_left);
    if regions.sidebar_title.h > 0 && y >= regions.sidebar_title.y && y < regions.sidebar_title.y + regions.sidebar_title.h {
        // Title box: text width minus its `paddingRight={2}` (`sidebar.tsx:32`).
        row.text(input.sidebar.title, descriptor.sidebar_text_width);
        row.blank(inner_w.saturating_sub(descriptor.sidebar_text_width));
    } else if regions.sidebar_scroll.h > 0
        && y >= regions.sidebar_scroll.y
        && y < regions.sidebar_scroll.y + regions.sidebar_scroll.h
    {
        let index = usize::from(y - regions.sidebar_scroll.y).saturating_add(input.sidebar.scroll);
        let line = input.sidebar.content.get(index).map_or("", String::as_str);
        // Constant reserved right padding under the ABSOLUTE bar
        // (`sidebar.tsx:56-69`): headings never reflow when it toggles.
        row.text(line, descriptor.sidebar_text_width);
        row.blank(inner_w.saturating_sub(descriptor.sidebar_text_width));
    } else if regions.sidebar_footer.h > 0
        && y >= regions.sidebar_footer.y
        && y < regions.sidebar_footer.y + regions.sidebar_footer.h
    {
        let index = usize::from(y - regions.sidebar_footer.y);
        let line = input.sidebar.footer.get(index).map_or("", String::as_str);
        row.text(line, inner_w);
    } else {
        row.blank(inner_w);
    }
    row.blank(descriptor.sidebar_pad_right);
    row.cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_view_layout::{
        SESSION_SIDEBAR_MAX_WIDTH, SESSION_SIDEBAR_WIDTH, SESSION_TABS_COMPACT_WIDTH,
        clamp_session_tabs_width, session_tabs_compact, session_tabs_fit_vertically,
        sidebar_scrollbar_reservation,
    };

    fn s(text: &str) -> String {
        text.to_owned()
    }

    fn cells_of(row: &str) -> usize {
        visit_unpadded_cells(row, usize::MAX, |_, w| {
            assert!(w > 0, "no zero-width cluster escapes the composer");
        })
    }

    fn lines(count: usize, prefix: &str) -> Vec<String> {
        (0..count).map(|i| s(&format!("{prefix} {i}"))).collect()
    }

    /// Cell-exact session-column cells of one composed row.
    fn session_segment(frame: &ShellFrame, y: u16) -> String {
        let regions = frame.regions;
        match frame.rows.get(usize::from(y)) {
            Some(row) if regions.session.w > 0 => {
                slice_cells(row, regions.session.x, regions.session.w)
            }
            _ => String::new(),
        }
    }

    /// Cell-exact sidebar cells of one composed row.
    fn sidebar_segment(frame: &ShellFrame, y: u16) -> String {
        let regions = frame.regions;
        match frame.rows.get(usize::from(y)) {
            Some(row) if regions.sidebar.w > 0 => slice_cells(row, regions.sidebar.x, regions.sidebar.w),
            _ => String::new(),
        }
    }

    /// Sidebar shown or hidden explicitly via the config mode, so the auto
    /// rule does not mask the case under test.
    fn mode(show: bool) -> SidebarMode {
        if show { SidebarMode::Auto } else { SidebarMode::Hide }
    }

    /// LAY-01. At 80x24, 120x40, 160x48 and 200x60 every composed row is
    /// exactly the terminal width, the frame is exactly the terminal height,
    /// and the root never overflows.
    #[test]
    fn lay_01_frame_fills_standard_viewports() {
        let composer = [s("> prompt"), s("[hints]")];
        for (cols, rows) in [(80u16, 24u16), (120, 40), (160, 48), (200, 60)] {
            let transcript = lines(usize::from(rows), "row");
            for show_sidebar in [false, true] {
                for (tabs_w, tabs_h) in [(0u16, 0u16), (42, 0), (0, 1)] {
                    let mut input = ShellFrameInput::new(cols, rows);
                    input.tabs_width = tabs_w;
                    input.tabs_height = tabs_h;
                    input.transcript = &transcript;
                    input.composer = &composer;
                    input.sidebar.mode = mode(show_sidebar);
                    input.sidebar.title = "Session";
                    let frame = compose_shell(&input);
                    assert_eq!(frame.rows.len(), usize::from(rows), "{cols}x{rows}: height");
                    for (index, row) in frame.rows.iter().enumerate() {
                        assert_eq!(
                            cells_of(row),
                            usize::from(cols),
                            "{cols}x{rows} tabs {tabs_w}x{tabs_h} row {index} must span the viewport"
                        );
                        assert!(row.len() <= usize::from(cols) * 4, "row {index}: bounded bytes");
                    }
                }
            }
        }
    }

    /// LAY-01 (regression). No pane border is ever drawn: upstream composes
    /// columns by flex, not by separator cells (`session-frame.tsx:325-398`).
    #[test]
    fn lay_01_frame_draws_no_pane_borders() {
        let transcript = lines(20, "line");
        let sidebar_content = lines(10, "side");
        let footer = [s("[close]")];
        let mut input = ShellFrameInput::new(160, 24);
        input.sidebar.open = true;
        input.sidebar.title = "Session title";
        input.sidebar.content = &sidebar_content;
        input.sidebar.footer = &footer;
        input.transcript = &transcript;
        let frame = compose_shell(&input);
        assert!(frame.sidebar_visible && frame.wide);
        for row in &frame.rows {
            assert!(!row.contains('\u{2502}'), "no vertical separators");
            assert!(!row.contains('\u{2500}'), "no horizontal rules");
        }
    }

    /// LAY-02. `routes/session/index.tsx:1515-1516`: at 43 columns the content
    /// gaps are 1 cell per side, at 44 they are 2, and a longer-than-viewport
    /// prompt is clipped inside the frame instead of overflowing.
    #[test]
    fn lay_02_prompt_stays_inside_the_viewport_at_43_and_44() {
        let long = s(&"> ".repeat(60));
        let composer = [long];
        for (cols, inset) in [(43u16, 1u16), (44, 2)] {
            let mut input = ShellFrameInput::new(cols, 10);
            input.composer = &composer;
            let frame = compose_shell(&input);
            let regions = frame.regions;
            assert_eq!(
                regions.transcript_text.x - regions.session.x,
                inset,
                "{cols}: left inset"
            );
            assert_eq!(
                regions.session.w - (regions.transcript_text.x - regions.session.x) - regions.transcript_text.w,
                inset,
                "{cols}: right inset"
            );
            assert!(regions.composer.y + regions.composer.h <= 10);
            for row in &frame.rows {
                assert_eq!(cells_of(row), usize::from(cols), "{cols}: prompt/footer fit the viewport");
            }
            let prompt_row = &frame.rows[usize::from(regions.composer.y)];
            let leading = prompt_row.len() - prompt_row.trim_start().len();
            assert_eq!(leading, usize::from(inset), "{cols}: prompt starts at the inset");
            assert!(prompt_row.trim_start().starts_with("> >"), "{cols}: prompt painted");
            assert!(prompt_row.ends_with(' '), "{cols}: right inset kept clear");
            assert!(prompt_row.len() <= usize::from(cols), "{cols}: prompt truncated");
        }
    }

    /// LAY-03 / LAY-05. With a vertical rail the frame subtracts the rail, and
    /// eligibility is decided by `sessionTabsFitVertically`.
    #[test]
    fn lay_03_vertical_rail_is_reserved_before_the_session_column() {
        for (cols, rail, vertical) in [(105u16, 42u16, false), (106, 42, true), (68, 5, false), (69, 5, true)] {
            let fits = session_tabs_fit_vertically(cols, rail);
            assert_eq!(fits, vertical, "{cols}/{rail}: vertical eligibility");
            let mut input = ShellFrameInput::new(cols, 20);
            input.tabs_width = if fits { rail } else { 0 };
            input.sidebar.open = true;
            let frame = compose_shell(&input);
            assert_eq!(frame.regions.session.x, if fits { rail } else { 0 });
            assert_eq!(frame.regions.session.w, cols - if fits { rail } else { 0 });
            for row in &frame.rows {
                assert_eq!(cells_of(row), usize::from(cols));
            }
        }
    }

    /// LAY-04. Rail clamp bounds (5/72/min-content 44) and the 11/12 compact
    /// transition, observed through the reservation width the frame uses.
    #[test]
    fn lay_04_rail_clamp_and_compact_transition() {
        for (width, total, expected) in [
            (2u16, 200u16, SESSION_TABS_COMPACT_WIDTH),
            (11, 200, 11),
            (12, 200, 12),
            (80, 200, SESSION_SIDEBAR_MAX_WIDTH),
            (40, 60, 16),
        ] {
            let clamped = clamp_session_tabs_width(width, total);
            assert_eq!(clamped, expected, "{width}/{total}");
            let mut input = ShellFrameInput::new(total, 24);
            input.tabs_width = clamped;
            input.sidebar.mode = SidebarMode::Hide;
            let frame = compose_shell(&input);
            assert_eq!(frame.regions.session.x, clamped, "{width}/{total}: rail reserved");
        }
        assert!(session_tabs_compact(11));
        assert!(!session_tabs_compact(12));
    }

    /// LAY-06 / LAY-07. Auto sidebar docking: 120/121 without a rail and
    /// 162/163 with the default 42-column rail; below the threshold an
    /// explicitly open sidebar becomes a right overlay instead of a dock.
    #[test]
    fn lay_06_lay_07_auto_sidebar_thresholds_and_overlay() {
        for (cols, rail, expected) in [
            (120u16, 0u16, false),
            (121, 0, true),
            (162, 42, false),
            (163, 42, true),
        ] {
            let mut input = ShellFrameInput::new(cols, 30);
            input.tabs_width = rail;
            let frame = compose_shell(&input);
            assert_eq!(frame.wide, expected, "{cols}/{rail}: wide");
            assert_eq!(frame.sidebar_visible, expected, "{cols}/{rail}: auto visibility");
            if expected {
                assert_eq!(frame.right, RightPane::Sidebar);
                assert_eq!(frame.regions.right.w, 42, "{cols}/{rail}: fixed 42 dock");
                assert_eq!(frame.regions.session.w, cols - rail - 42);
            } else {
                assert_eq!(frame.right, RightPane::None);
                assert_eq!(frame.regions.session.w, cols - rail);
            }
        }
        // Explicitly open below the threshold: absolute overlay at the right
        // edge, session column unchanged (`session-frame.tsx:385-397`).
        let mut input = ShellFrameInput::new(120, 24);
        input.sidebar.open = true;
        input.sidebar.title = "overlay";
        let frame = compose_shell(&input);
        assert!(!frame.wide && frame.sidebar_visible);
        assert!(frame.regions.sidebar_overlay && !frame.regions.sidebar_docked);
        assert_eq!(frame.regions.right.w, 0);
        assert_eq!(frame.regions.session.w, 120);
        assert_eq!(frame.regions.sidebar.x, 78, "120 - 42");
        let title_row = &frame.rows[usize::from(frame.regions.sidebar_title.y)];
        assert!(title_row.contains("overlay"), "overlay sidebar painted");
        assert_eq!(title_row.find("overlay"), Some(80), "at x = 78 + 2 padding");
    }

    /// LAY-11. Exactly one right pane: panel beats sidebar, sidebar beats
    /// terminal, terminal beats the automatic sidebar, and a docked pane is
    /// reserved at its own width.
    #[test]
    fn lay_11_one_right_pane_at_a_time() {
        let mut input = ShellFrameInput::new(160, 30);
        input.sidebar.open = true;
        input.sidebar.title = "side";
        input.terminal_selected = true;
        input.pane_width = 50;
        input.panel_open = true;
        let frame = compose_shell(&input);
        assert_eq!(frame.right, RightPane::Panel);
        assert_eq!(frame.regions.right.w, 50);
        assert_eq!(frame.regions.sidebar.w, 0, "panel suppresses the sidebar box");
        assert!(frame.rows.iter().all(|row| !row.contains("side")));

        input.panel_open = false;
        let frame = compose_shell(&input);
        assert_eq!(frame.right, RightPane::Sidebar, "open sidebar beats terminal");
        assert_eq!(frame.regions.right.w, 42);
        assert!(frame.rows.iter().any(|row| row.contains("side")));

        input.sidebar.open = false;
        let frame = compose_shell(&input);
        assert_eq!(frame.right, RightPane::Terminal, "terminal beats auto sidebar");
        assert_eq!(frame.regions.right.w, 50);

        input.terminal_selected = false;
        let frame = compose_shell(&input);
        assert_eq!(frame.right, RightPane::Sidebar, "auto sidebar on a wide view");

        input.sidebar.child_session = true;
        let frame = compose_shell(&input);
        assert_eq!(frame.right, RightPane::None, "child sessions hide it");
        assert_eq!(frame.regions.right.w, 0);
    }

    /// LAY-14. The transcript and the sidebar scroll independently: scrolling
    /// one leaves the other's rows and both pinned headings untouched.
    #[test]
    fn lay_14_transcript_and_sidebar_scroll_independently() {
        let transcript = lines(60, "t");
        let sidebar_content = lines(60, "s");
        let footer = [s("[footer]")];
        let mut input = ShellFrameInput::new(160, 24);
        input.sidebar.open = true;
        input.sidebar.title = "Title row";
        input.sidebar.content = &sidebar_content;
        input.sidebar.footer = &footer;
        input.transcript = &transcript;
        let baseline = compose_shell(&input);

        let title_row = baseline.regions.sidebar_title.y;
        let footer_row = baseline.regions.sidebar_footer.y;
        assert!(baseline.rows[usize::from(title_row)].contains("Title row"));
        assert!(baseline.rows[usize::from(footer_row)].contains("[footer]"));

        // Scroll only the transcript.
        let mut scrolled = input;
        scrolled.transcript_scroll = 10;
        let frame = compose_shell(&scrolled);
        assert_eq!(
            sidebar_segment(&frame, title_row),
            sidebar_segment(&baseline, title_row),
            "pinned title unchanged"
        );
        assert_eq!(
            sidebar_segment(&frame, footer_row),
            sidebar_segment(&baseline, footer_row),
            "pinned footer unchanged"
        );
        assert_eq!(
            sidebar_segment(&frame, frame.regions.sidebar_scroll.y),
            sidebar_segment(&baseline, baseline.regions.sidebar_scroll.y),
            "sidebar rows are unaffected by the transcript scroll"
        );
        assert!(
            frame.rows[usize::from(frame.regions.transcript.y)].contains("t 10"),
            "transcript scrolled by 10 rows"
        );
        assert_ne!(
            session_segment(&frame, frame.regions.transcript.y),
            session_segment(&baseline, baseline.regions.transcript.y),
            "the session segment did move with the transcript"
        );

        // Scroll only the sidebar.
        let mut scrolled = input;
        scrolled.sidebar.scroll = 7;
        let frame = compose_shell(&scrolled);
        assert_eq!(
            session_segment(&frame, frame.regions.transcript.y),
            session_segment(&baseline, baseline.regions.transcript.y),
            "transcript unaffected by the sidebar scroll"
        );
        assert!(frame.rows[usize::from(frame.regions.sidebar_scroll.y)].contains("s 7"));
        assert_eq!(
            sidebar_segment(&frame, title_row),
            sidebar_segment(&baseline, title_row),
            "pinned title unchanged"
        );
        assert_eq!(
            sidebar_segment(&frame, footer_row),
            sidebar_segment(&baseline, footer_row),
            "pinned footer unchanged"
        );
    }

    /// LAY-15. The row between the transcript and the composer exists in every
    /// state; only its contents change (`index.tsx:1570-1589`).
    #[test]
    fn lay_15_action_row_is_reserved_without_a_jump_action() {
        let transcript = lines(10, "row");
        let composer = [s("> prompt")];
        for action in [JumpAction::None, JumpAction::Loading, JumpAction::AwayFromBottom] {
            let mut input = ShellFrameInput::new(80, 20);
            input.transcript = &transcript;
            input.composer = &composer;
            input.jump = action;
            let frame = compose_shell(&input);
            let regions = frame.regions;
            assert_eq!(regions.jump.h, 1, "{action:?}: one row is always reserved");
            assert_eq!(regions.jump.y + 1, regions.composer.y, "{action:?}: above the composer");
            let row = &frame.rows[usize::from(regions.jump.y)];
            let cell = slice_cells(row, regions.transcript_text.x, regions.transcript_text.w);
            match action {
                JumpAction::None => assert_eq!(cell.trim(), "", "{action:?}: blank but reserved"),
                JumpAction::Loading => {
                    assert_eq!(cell.trim(), LOADING_LABEL, "right aligned label");
                    assert!(cell.ends_with(LOADING_LABEL), "flush right, no jump padding");
                }
                JumpAction::AwayFromBottom => {
                    assert_eq!(cell.trim(), JUMP_LABEL, "right aligned label");
                    assert!(cell.ends_with(JUMP_LABEL), "flush right");
                    let label_at = cell.len() - JUMP_LABEL.len();
                    assert_eq!(cell.as_bytes()[label_at - 1], b' ', "paddingLeft 1 before the label");
                }
            }
            assert!(frame.rows[usize::from(regions.composer.y)].contains("> prompt"));
        }
    }

    /// LAY-16. Showing the transcript scrollbar reserves two columns (viewport
    /// padding + track) so content narrows without moving the left edge, while
    /// the sidebar's absolute bar reserves nothing and its headings keep both
    /// origin and width (`index.tsx:1527-1537`, `sidebar.tsx:56-69`).
    #[test]
    fn lay_16_scrollbar_reservation_cells() {
        let transcript = [s(&"x".repeat(200))];
        let sidebar_content = [s(&"y".repeat(200))];
        let mut input = ShellFrameInput::new(160, 12);
        input.sidebar.open = true;
        input.sidebar.title = "Heading";
        input.sidebar.content = &sidebar_content;
        input.transcript = &transcript;

        let without = compose_shell(&input);
        input.scrollbar = true;
        let with = compose_shell(&input);

        assert_eq!(without.regions.transcript_text.x, with.regions.transcript_text.x);
        assert_eq!(
            without.regions.transcript_text.w - with.regions.transcript_text.w,
            2,
            "paddingRight 1 + track 1"
        );
        let row = &with.rows[usize::from(with.regions.transcript.y)];
        let text_end = usize::from(with.regions.transcript_text.x + with.regions.transcript_text.w);
        assert_eq!(&row[text_end..text_end + 2], "  ", "two reserved cells stay blank");
        assert_eq!(cells_of(row), 160);

        // Sidebar: absolute bar, constant content geometry.
        assert_eq!(without.regions.sidebar_title, with.regions.sidebar_title);
        assert_eq!(without.regions.sidebar_scroll, with.regions.sidebar_scroll);
        assert_eq!(sidebar_scrollbar_reservation(true), 0);
        let heading_row = with.regions.sidebar_title.y;
        assert_eq!(
            with.rows[usize::from(heading_row)],
            without.rows[usize::from(heading_row)],
            "heading does not reflow when the sidebar bar toggles"
        );
        assert_eq!(with.regions.sidebar_text_width(), 37);
    }

    /// Degenerate viewports (zero and tiny) compose safely: no panic, exact
    /// width, bounded rows, and every region stays inside the viewport.
    #[test]
    fn degenerate_viewports_compose_safely() {
        let transcript = [s("row")];
        let composer = [s(">")];
        for (cols, rows) in [(0u16, 0u16), (0, 5), (5, 0), (1, 1), (3, 2), (43, 3)] {
            for sidebar in [false, true] {
                let mut input = ShellFrameInput::new(cols, rows);
                input.sidebar.open = sidebar;
                input.sidebar.title = "t";
                input.transcript = &transcript;
                input.composer = &composer;
                let frame = compose_shell(&input);
                assert!(frame.rows.len() <= usize::from(rows));
                for row in &frame.rows {
                    assert_eq!(cells_of(row), usize::from(cols), "{cols}x{rows}");
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // LAY-13: one frame descriptor (rail, insets, reservations, dock).
    // -----------------------------------------------------------------------

    /// LAY-13. `session-frame.tsx:323-331,381-384` +
    /// `routes/session/index.tsx:1514-1520`: the descriptor carries every
    /// geometry fact once — rail, available frame box, dock, insets and the
    /// reserved action/bottom rows — and the bands sum to the viewport, so the
    /// painter and the resize handle never re-derive them.
    #[test]
    fn lay13_descriptor_is_the_single_geometry_source() {
        let mut input = ShellFrameInput::new(160, 24);
        input.panel_open = true;
        input.pane_width = 50;
        let frame = compose_shell(&input);
        let d = frame.descriptor;
        assert_eq!(d.cols, 160);
        assert_eq!(d.rows, 24);
        assert_eq!(d.pane, RightPane::Panel);
        assert_eq!(d.rail, frame.regions.tabs);
        assert_eq!(d.rail.w, 0, "no rail without vertical tabs");
        assert_eq!(d.available.x, 0);
        assert_eq!(d.available.w, 160);
        assert_eq!(d.right.w, 50, "paneResize.size()");
        assert_eq!(
            d.available.x + d.available.w,
            d.right.x + d.right.w,
            "the dock ends at the frame's right edge"
        );
        assert_eq!(d.inset_left, 2);
        assert_eq!(d.inset_right, 2);
        assert_eq!(
            d.inset_left + d.content_width + d.inset_right,
            d.regions.session.w,
            "insets + content fill the session column"
        );
        assert_eq!(d.content_width, frame.regions.transcript.w);
        assert_eq!(d.action_rows, 1, "action strip is always reserved");
        assert_eq!(d.bottom_pad_rows, 1, "paddingBottom 1");
        assert_eq!(d.scrollbar_cells, 0);

        // A vertical rail moves the frame box right without changing its math.
        let mut railed = ShellFrameInput::new(200, 24);
        railed.tabs_width = 42;
        railed.terminal_selected = true;
        railed.pane_width = 50;
        let frame = compose_shell(&railed);
        let d = frame.descriptor;
        assert_eq!(d.rail.w, 42, "the rail is a sibling reservation");
        assert_eq!(d.rail.h, 24);
        assert_eq!(d.available.x, 42);
        assert_eq!(d.available.w, 158);
        assert_eq!(d.right.w, 50);
        assert_eq!(d.right.x + d.right.w, 200);
        assert_eq!(
            d.inset_left + d.content_width + d.inset_right,
            d.regions.session.w
        );
        for row in &frame.rows {
            assert_eq!(cells_of(row), 200);
        }
    }

    /// LAY-13 / LAY-16. The sidebar pads and the scrollbar reservation are
    /// resolved once in the descriptor (`sidebar.tsx:26-29,56-69`,
    /// `routes/session/index.tsx:1527-1537`) instead of per row.
    #[test]
    fn lay13_descriptor_resolves_sidebar_pads_and_scrollbar_cells() {
        let content = [s(&"y".repeat(200))];
        let mut input = ShellFrameInput::new(160, 12);
        input.sidebar.open = true;
        input.sidebar.title = "Heading";
        input.sidebar.content = &content;
        let plain = compose_shell(&input).descriptor;
        assert_eq!(plain.sidebar.w, SESSION_SIDEBAR_WIDTH);
        assert_eq!(plain.sidebar_pad_left, 2, "paddingLeft 2");
        assert_eq!(plain.sidebar_pad_right, 2, "paddingRight 2");
        assert_eq!(plain.sidebar_inner_width, 38);
        assert_eq!(plain.sidebar_text_width, 37, "one reserved scrollbar column");
        assert_eq!(
            plain.sidebar_pad_left + plain.sidebar_inner_width + plain.sidebar_pad_right,
            plain.sidebar.w
        );
        assert_eq!(plain.scrollbar_cells, 0);

        input.scrollbar = true;
        let scrolled = compose_shell(&input).descriptor;
        assert_eq!(scrolled.scrollbar_cells, 2, "viewport padding 1 + track 1");
        assert_eq!(
            scrolled.content_width, plain.content_width,
            "the reservation comes out of the content, not the session column"
        );
        assert_eq!(
            scrolled.sidebar_text_width, plain.sidebar_text_width,
            "the absolute sidebar bar never reflows headings"
        );
    }
}
