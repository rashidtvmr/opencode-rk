#![forbid(unsafe_code)]
//! Bounded vertical session rail geometry and paint projection.
//! Upstream references: packages/tui/src/component/session-tabs.tsx:542-740
//! and packages/tui/src/component/session-tabs-rail.tsx:8-92.

use crate::native_tabs::TabFlags;
use crate::native_view_layout::{fit, visit_unpadded_cells};

/// Per-cell color role for the renderer, enabling multi-color rows without
/// guessing glyph text. A user title may contain the same glyphs as spinner,
/// close indicator, or New-session "+", so roles are assigned structurally.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellColor {
    /// Unstyled compact layout padding: native blank-cell default, never bold.
    CompactPadding,
    /// Background filler (leading pad spaces, trailing gap).
    Fill,
    /// Title text on a session row. Fg: Text (sel/hov), TextMuted (idle); bold if sel/att/busy.
    Title,
    /// Detail text on a session row. Fg: tint(textMuted, bg, 0.35); never bold.
    Detail,
    /// Busy spinner character. Fg: Primary; bold.
    Spinner,
    /// Close indicator (present only on hovered title row). Fg: TextMuted.
    Close,
    /// New-session "+" indicator char. Fg: idleNumber (not newTab), Text (newTab); bold only when newTab.
    NewIndicator,
    /// New-session "New session" text. Fg: TextMuted (not newTab), Text (newTab); bold only when newTab.
    NewText,
    /// Compact search glyph. Fg: TextMuted, never bold.
    Search,
    /// Compact search-control hover overlay row (`▀`). Upstream
    /// `session-tabs-rail.tsx:55-66`: fg = `tint(raised.base, raised.high,
    /// raised.high.a)`, bg = `raised.base`.
    SearchHover,
    CapTop,
    CapBottom,
}

/// Resolved paint parameters for a row, from palette via upstream theme mappings.
/// All colors are raw palette `[u8;4]`; the caller applies dim via local wrapper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowPaint {
    /// Background: panel (selected), element (hovered-only), or panel (idle).
    pub background: [u8; 4],
    /// Title fg: Text (selected/hovered), TextMuted (idle).
    pub title_fg: [u8; 4],
    /// Detail fg: tint(textMuted, bg, 0.35).
    pub detail_fg: [u8; 4],
    /// Title attribute: bold only when selected, matching the vertical indicator.
    pub title_attr: u32,
    /// Spinner fg: Primary slot.
    pub spinner_fg: [u8; 4],
    /// Close fg: always TextMuted.
    pub close_fg: [u8; 4],
    /// "+" fg: idleNumber (not newTab), Text (newTab).
    pub plus_fg: [u8; 4],
    /// "New session" fg: TextMuted (not newTab), Text (newTab).
    pub new_text_fg: [u8; 4],
    /// New "+"/"New session" attribute: 1 if newTab, else 0.
    pub new_attr: u32,
    pub cap_top_fg: [u8; 4],
    pub cap_bottom_fg: [u8; 4],
    /// Compact search-hover overlay fg: `tint(raised.base, raised.high,
    /// raised.high.a)` (`session-tabs-rail.tsx:17-19,61`).
    pub search_hover_fg: [u8; 4],
    /// Compact search-hover overlay bg: `raised.base` (`session-tabs-rail.tsx:62`).
    pub search_hover_bg: [u8; 4],
}

/// tint(base, overlay, alpha) = round(base + (overlay - base) * alpha), per color.ts:43.
#[inline]
pub fn tint(base: [u8; 4], overlay: [u8; 4], alpha: f64) -> [u8; 4] {
    let blend = |b: u8, o: u8| -> u8 {
        let result = f64::from(b) + (f64::from(o) - f64::from(b)) * alpha;
        result.round().clamp(0.0, 255.0) as u8
    };
    [
        blend(base[0], overlay[0]),
        blend(base[1], overlay[1]),
        blend(base[2], overlay[2]),
        base[3],
    ]
}

/// idleNumber() per session-tabs.tsx:567: tint(text.formfield.base, background(), 0.55).
/// text.formfield.base == text.base (Text slot) in both dark and light themes.
#[inline]
pub fn idle_number(palette: &crate::native_theme_runtime::Palette) -> [u8; 4] {
    use crate::native_theme_runtime::Slot;
    tint(
        palette.get(Slot::Text),
        palette.get(Slot::BackgroundPanel),
        0.55,
    )
}

/// `session-tabs-model.ts:14-22` `sessionTabDetail`: when the tab is a linked
/// worktree whose current branch differs from the default, show
/// `"<project> \u{2387} <branch>"`; otherwise fall back to the branch alone, then
/// the project. TAB-10.
#[must_use]
pub fn session_tab_detail(
    project: &str,
    current: Option<&str>,
    default_branch: Option<&str>,
    worktree: bool,
) -> String {
    let branch = if worktree && current != default_branch {
        current
    } else {
        None
    };
    match (branch, project.is_empty()) {
        (Some(branch), false) => format!("{project} \u{2387} {branch}"),
        (Some(branch), true) => branch.to_owned(),
        (None, _) => project.to_owned(),
    }
}

/// `session-tabs-rail.tsx:17-19`: the compact search control's hover tint,
/// `tint(background.raised.base, background.raised.high,
/// background.raised.high.a)`. `raised.base == backgroundPanel` and
/// `raised.high == backgroundElement` are the native slot mapping used
/// throughout this module.
#[inline]
fn search_hover_fg(palette: &crate::native_theme_runtime::Palette) -> [u8; 4] {
    use crate::native_theme_runtime::Slot;
    let base = palette.get(Slot::BackgroundPanel);
    let high = palette.get(Slot::BackgroundElement);
    tint(base, high, f64::from(high[3]) / 255.0)
}

impl RowPaint {
    pub fn compact_session_row(
        palette: &crate::native_theme_runtime::Palette,
        flags: &TabFlags,
        is_hovered: bool,
    ) -> Self {
        let mut paint = Self::session_row(palette, flags, is_hovered);
        use crate::native_theme_runtime::Slot;
        if flags.active {
            paint.background = palette.get(Slot::BackgroundElement);
            paint.detail_fg = tint(palette.get(Slot::TextMuted), paint.background, 0.35);
        }
        paint
    }

    pub fn cap_row(palette: &crate::native_theme_runtime::Palette, bottom: bool) -> Self {
        use crate::native_theme_runtime::Slot;
        let panel = palette.get(Slot::BackgroundPanel);
        let element = palette.get(Slot::BackgroundElement);
        Self {
            background: panel,
            title_fg: if bottom { element } else { panel },
            detail_fg: panel,
            title_attr: 0,
            spinner_fg: panel,
            close_fg: panel,
            plus_fg: panel,
            new_text_fg: panel,
            new_attr: 0,
            cap_top_fg: panel,
            cap_bottom_fg: element,
            search_hover_fg: search_hover_fg(palette),
            search_hover_bg: panel,
        }
    }

    pub fn compact_search_row(palette: &crate::native_theme_runtime::Palette) -> Self {
        use crate::native_theme_runtime::Slot;
        let background = palette.get(Slot::BackgroundPanel);
        let muted = palette.get(Slot::TextMuted);
        Self {
            background,
            title_fg: muted,
            detail_fg: muted,
            title_attr: 0,
            spinner_fg: muted,
            close_fg: muted,
            plus_fg: muted,
            new_text_fg: muted,
            new_attr: 0,
            cap_top_fg: background,
            cap_bottom_fg: background,
            search_hover_fg: search_hover_fg(palette),
            search_hover_bg: background,
        }
    }

    /// Compact search-control hover overlay row (`session-tabs-rail.tsx:54-66`):
    /// fg = `tint(raised.base, raised.high, raised.high.a)`; bg = that tint when
    /// the row below is highlighted, else `raised.base`.
    pub fn compact_search_hover_row(
        palette: &crate::native_theme_runtime::Palette,
        below_highlighted: bool,
    ) -> Self {
        let mut paint = Self::compact_search_row(palette);
        paint.title_fg = paint.search_hover_fg;
        paint.background = if below_highlighted {
            paint.search_hover_fg
        } else {
            paint.search_hover_bg
        };
        paint
    }

    /// Resolve paint for a session row. Upstream priority: attention > busy >
    /// hovered+selected > hovered > selected > idle for fg/attr, but bg follows
    /// tabBackground() logic (selected -> panel, hovered-only -> element, else panel).
    pub fn session_row(
        palette: &crate::native_theme_runtime::Palette,
        flags: &TabFlags,
        is_hovered: bool,
    ) -> Self {
        use crate::native_theme_runtime::Slot;
        let panel = palette.get(Slot::BackgroundPanel);
        let element = palette.get(Slot::BackgroundElement);
        let text = palette.get(Slot::Text);
        let text_muted = palette.get(Slot::TextMuted);
        let primary = palette.get(Slot::Primary);

        let is_attention = flags.needs_attention;
        let is_busy = flags.busy;
        let is_selected = flags.active;

        // Background (tabBackground 774-779): selected -> panel (actionSelected transparent);
        // hovered && !selected -> element; else -> panel
        let bg = if is_selected {
            panel
        } else if is_hovered {
            element
        } else {
            panel
        };

        // Title fg (foreground 794-797): hovered -> Text; selected -> Text; idle -> TextMuted
        let title_fg = if is_hovered || is_selected {
            text
        } else {
            text_muted
        };

        // Attr (bold): attention/busy/selected -> 1; idle/hovered -> 0
        let title_attr = if is_selected {
            1
        } else {
            0
        };

        // Detail fg (detailColor 814): tint(textMuted, pulseBackground, 0.35)
        // pulseBackground == bg when animations=false
        let detail_fg = tint(text_muted, bg, 0.35);

        // Close fg: always TextMuted (closeHovered is transient, false in fixtures)
        let close_fg = text_muted;

        RowPaint {
            background: bg,
            title_fg,
            detail_fg,
            title_attr,
            spinner_fg: primary,
            close_fg,
            plus_fg: [0, 0, 0, 0],
            new_text_fg: [0, 0, 0, 0],
            new_attr: 0,
            cap_top_fg: [0, 0, 0, 0],
            cap_bottom_fg: [0, 0, 0, 0],
            search_hover_fg: [0, 0, 0, 0],
            search_hover_bg: [0, 0, 0, 0],
        }
    }

    /// Resolve paint for the New-session control row.
    /// Upstream session-tabs.tsx:1197-1214:
    ///   "+" fg = newTab() || addHovered() ? Text : idleNumber(), bold if newTab()
    ///   "New session" fg = newTab() || addHovered() ? Text : TextMuted, bold if newTab()
    pub fn new_control_row(
        palette: &crate::native_theme_runtime::Palette,
        new_tab: bool,
        add_hovered: bool,
    ) -> Self {
        use crate::native_theme_runtime::Slot;
        let panel = palette.get(Slot::BackgroundPanel);
        let element = palette.get(Slot::BackgroundElement);
        let text = palette.get(Slot::Text);
        let text_muted = palette.get(Slot::TextMuted);

        // bg: newTab && !compact -> panel (actionSelected transparent); addHovered -> element; else panel
        let bg = if new_tab {
            panel
        } else if add_hovered {
            element
        } else {
            panel
        };

        let plus_fg = if new_tab || add_hovered {
            text
        } else {
            idle_number(palette)
        };
        let new_text_fg = if new_tab || add_hovered {
            text
        } else {
            text_muted
        };
        // Only newTab() (not addHovered) triggers bold per upstream: newTab? BOLD : undefined
        let new_attr = if new_tab { 1 } else { 0 };

        RowPaint {
            background: bg,
            title_fg: [0, 0, 0, 0],
            detail_fg: [0, 0, 0, 0],
            title_attr: 0,
            spinner_fg: [0, 0, 0, 0],
            close_fg: [0, 0, 0, 0],
            plus_fg,
            new_text_fg,
            new_attr,
            cap_top_fg: palette.get(Slot::BackgroundPanel),
            cap_bottom_fg: palette.get(Slot::BackgroundElement),
            search_hover_fg: search_hover_fg(palette),
            search_hover_bg: palette.get(Slot::BackgroundPanel),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailStyle {
    Idle,
    Active,
    Hovered,
    Busy,
    Attention,
    NewControl,
    Empty,
}

pub const RAIL_WIDTH: u16 = 42;
pub const TOP_PADDING: u16 = 1;
pub const TAB_HEIGHT: u16 = 2;
pub const TAB_GAP: u16 = 1;
pub const ADD_HEIGHT: u16 = 1;
pub const TAB_STRIDE: u16 = TAB_HEIGHT + TAB_GAP;
/// OpenCode switches the vertical rail to its compact renderer below 12 cells.
pub const COMPACT_BREAKPOINT: u16 = 12;
pub const COMPACT_STRIDE: u16 = 2;

/// Title cell budget for the expanded rail's first row. Mirrors
/// `render_expanded_cells`: `max(2, digits)` indicator cells plus a one-cell
/// gap, then the trailing pad. A hovered row wide enough for the close child
/// (width >= 4) reserves two trailing cells instead of one.
#[must_use]
pub fn expanded_title_width(width: u16, items: usize, hovered: bool) -> u16 {
    let indicator_width = items.to_string().len().max(2) as u16;
    let pad = width.min(indicator_width.saturating_add(1));
    let tail = if hovered && width >= 4 { 2 } else { 1 };
    width.saturating_sub(pad).saturating_sub(tail)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RailInput {
    pub session: crate::native_navigation::SessionRef,
    pub title: String,
    pub detail: String,
    pub flags: TabFlags,
    /// Marquee window start for the hovered title (0 paints the plain title).
    pub marquee_offset: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RailRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl RailRect {
    pub fn contains(self, x: u16, y: u16) -> bool {
        x >= self.x
            && y >= self.y
            && x < self.x.saturating_add(self.width)
            && y < self.y.saturating_add(self.height)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RailHit {
    Session(crate::native_navigation::SessionRef),
    New,
    Search,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RailRowKind {
    Padding,
    Search,
    /// Compact search-control hover overlay row (`session-tabs-rail.tsx:54-66`).
    SearchHover,
    CapTop,
    CapBottom,
    Session(crate::native_navigation::SessionRef),
    Gap,
    New,
    Empty,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RailRow {
    pub kind: RailRowKind,
    pub text: String,
    pub rect: RailRect,
    pub style: RailStyle,
    /// (column_offset, CellColor) breakpoints for per-cell paint.
    pub color_spans: Vec<(u16, CellColor)>,
    /// Original TabFlags that produced this row's style, needed for paint.
    pub flags: TabFlags,
    /// Whether this row's session was hovered when rendered.
    pub is_hovered: bool,
    /// Four-cell fade window for the title/detail text of this row.
    pub fade: Option<crate::native_tabs::TitleFade>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RailFrame {
    pub width: u16,
    pub height: u16,
    pub scroll_start: usize,
    pub scroll_cells: usize,
    pub rows: Vec<RailRow>,
    pub new_rect: Option<RailRect>,
    /// Whether the "New session" tab is the active/focused new tab.
    /// When true, the New row uses Text+bold for "+" and "New session".
    pub active_new: bool,
    visible_close: Option<(crate::native_navigation::SessionRef, RailRect)>,
}

impl RailFrame {
    pub fn render(
        inputs: &[RailInput],
        width: u16,
        height: u16,
        scroll_start: usize,
        hovered: Option<&crate::native_navigation::SessionRef>,
    ) -> Self {
        Self::render_with_active_new(inputs, width, height, scroll_start, hovered, false)
    }

    pub fn render_with_active_new(
        inputs: &[RailInput],
        width: u16,
        height: u16,
        scroll_start: usize,
        hovered: Option<&crate::native_navigation::SessionRef>,
        active_new: bool,
    ) -> Self {
        Self::render_expanded_cells(
            inputs,
            width,
            height,
            scroll_start.saturating_mul(usize::from(TAB_STRIDE)),
            hovered,
            active_new,
        )
    }

    /// Scroll and clip in physical cells, including partially visible sessions.
    /// Upstream session-tabs.tsx:658-667 reveals the selected child's exact bottom.
    pub fn render_cells(
        inputs: &[RailInput],
        width: u16,
        height: u16,
        requested_scroll: usize,
        hovered: Option<&crate::native_navigation::SessionRef>,
        active_new: bool,
    ) -> Self {
        Self::render_cells_with_search_hover(
            inputs,
            width,
            height,
            requested_scroll,
            hovered,
            active_new,
            false,
        )
    }

    /// TAB-23: as [`Self::render_cells`], plus the compact search-control
    /// hover fact (`session-tabs-rail.tsx:14-20`). Only the live vertical-rail
    /// render path needs the extra fact.
    pub fn render_cells_with_search_hover(
        inputs: &[RailInput],
        width: u16,
        height: u16,
        requested_scroll: usize,
        hovered: Option<&crate::native_navigation::SessionRef>,
        active_new: bool,
        search_hovered: bool,
    ) -> Self {
        // Five cells is the upstream compact minimum.  Sub-minimum test
        // viewports retain the bounded expanded projection so legacy close
        // hit geometry remains well-defined without exposing compact search.
        if width >= 5 && width < COMPACT_BREAKPOINT {
            return Self::render_compact_cells(inputs, width, height, requested_scroll, hovered, active_new, search_hovered);
        }
        Self::render_expanded_cells(inputs, width, height, requested_scroll, hovered, active_new)
    }

    fn render_expanded_cells(
        inputs: &[RailInput],
        width: u16,
        height: u16,
        requested_scroll: usize,
        hovered: Option<&crate::native_navigation::SessionRef>,
        active_new: bool,
    ) -> Self {
        let mut frame = Self {
            width,
            height,
            scroll_start: 0,
            scroll_cells: 0,
            rows: Vec::new(),
            new_rect: None,
            active_new,
            visible_close: None,
        };
        if width == 0 || height == 0 {
            return frame;
        }
        let top = TOP_PADDING.min(height);
        let usable = usize::from(height.saturating_sub(top));
        let stride = usize::from(TAB_STRIDE);
        let new_top = inputs.len().saturating_mul(stride);
        let maximum = new_top
            .saturating_add(usize::from(ADD_HEIGHT))
            .saturating_sub(usable);
        let mut scroll = requested_scroll.min(maximum);
        let reveal = if active_new {
            Some((new_top, usize::from(ADD_HEIGHT)))
        } else {
            inputs
                .iter()
                .position(|input| input.flags.active)
                .map(|index| (index.saturating_mul(stride), usize::from(TAB_HEIGHT)))
        };
        if usable > 0 {
            if let Some((target, target_height)) = reveal {
                if target < scroll {
                    scroll = target;
                } else if target.saturating_add(target_height) > scroll.saturating_add(usable) {
                    scroll = target.saturating_add(target_height).saturating_sub(usable);
                }
            }
        }
        frame.scroll_cells = scroll.min(maximum);
        frame.scroll_start = frame.scroll_cells / stride;
        // Upstream reserves max(2, decimal tab-count width) indicator cells
        // followed by a one-cell gap; both title and detail share that inset.
        let indicator_width = inputs.len().to_string().len().max(2);
        let pad = width.min(indicator_width.saturating_add(1).min(usize::from(u16::MAX)) as u16);
        for y in 0..height {
            let rect = RailRect {
                x: 0,
                y,
                width,
                height: 1,
            };
            if y < top {
                frame.rows.push(RailRow {
                    kind: RailRowKind::Padding,
                    text: fit("", width),
                    rect,
                    style: RailStyle::Empty,
                    color_spans: Vec::new(),
                    flags: TabFlags::default(),
                    is_hovered: false,
                    fade: None,
                });
                continue;
            }
            let logical = frame.scroll_cells.saturating_add(usize::from(y - top));
            let index = logical / stride;
            let part = logical % stride;
            if let Some(input) = inputs.get(index) {
                if part < usize::from(TAB_HEIGHT) {
                    let style = if input.flags.needs_attention {
                        RailStyle::Attention
                    } else if input.flags.busy {
                        RailStyle::Busy
                    } else if hovered == Some(&input.session) {
                        RailStyle::Hovered
                    } else if input.flags.active {
                        RailStyle::Active
                    } else {
                        RailStyle::Idle
                    };
                    let close = part == 0 && hovered == Some(&input.session) && width >= 4;
                    // The close child occupies the penultimate cell. An idle
                    // title reserves the right padding; details reserve two.
                    let tail = if close || part != 0 { 2 } else { 1 };
                    let row_pad = pad.min(width.saturating_sub(tail));
                    let indicator = if part == 0 && input.flags.busy && !input.flags.needs_attention
                    {
                        "⠋"
                    } else {
                        ""
                    };
                    let prefix = fit(&format!("{indicator:>indicator_width$} "), row_pad);
                    let available = if part == 0 {
                        expanded_title_width(width, inputs.len(), hovered == Some(&input.session))
                    } else {
                        width.saturating_sub(row_pad).saturating_sub(tail)
                    };
                    let source = if part == 0 { &input.title } else { &input.detail };
                    // `session-tabs.tsx:1590-1596`: only the hovered title
                    // takes the marquee window; the detail line never scrolls.
                    let displayed = if part == 0 && input.marquee_offset > 0 {
                        crate::native_tabs::marquee_text(source, available, input.marquee_offset)
                    } else {
                        crate::native_tabs::take_width(source, available)
                    };
                    let text = fit(&displayed, available);
                    let fade = (crate::native_tabs::marquee_overflows(source, available)
                        && usize::from(available) > crate::native_tabs::FADE_WIDTH)
                        .then(|| crate::native_tabs::TitleFade {
                            start: row_pad,
                            // `visibleTitleParts().length`: the clusters actually
                            // painted, excluding the fit's padding tail.
                            length: grapheme_count(&displayed).min(usize::from(u16::MAX)) as u16,
                            leading: part == 0 && input.marquee_offset > 0,
                        });
                    let text = if close {
                        frame.visible_close = Some((
                            input.session.clone(),
                            RailRect {
                                x: width - 2,
                                y,
                                width: 1,
                                height: 1,
                            },
                        ));
                        let body = fit(&format!("{prefix}{text}"), width.saturating_sub(2));
                        format!("{body}✕ ")
                    } else {
                        fit(&format!("{prefix}{text}"), width)
                    };
                    // Build per-cell color span metadata.
                    // Roles: Fill (leading pad, spinner spaces, trailing),
                    //        Title (session title text), Detail (detail text),
                    //        Spinner (busy indicator char), Close (hovered close).
                    // Spinner is right-aligned in indicator_width cells within the prefix,
                    // then a space; spinner sits at column row_pad - 2 when busy.
                    let mut color_spans = Vec::new();
                    color_spans.push((0, CellColor::Fill));
                    // Spinner position: row_pad - 2 (right-aligned in indicator_width, +1 space)
                    if input.flags.busy && !input.flags.needs_attention && part == 0 && row_pad >= 2
                    {
                        color_spans.push(((row_pad - 2) as u16, CellColor::Spinner));
                    }
                    color_spans.push((
                        row_pad,
                        if part == 0 {
                            CellColor::Title
                        } else {
                            CellColor::Detail
                        },
                    ));
                    if close {
                        // Close indicator at width-2, trailing space at width-1
                        color_spans.push(((width - 2), CellColor::Close));
                        color_spans.push((width - 1, CellColor::Fill));
                    } else if part == 0 {
                        // Title: 1-cell trailing pad
                        color_spans.push((width.saturating_sub(1), CellColor::Fill));
                    } else {
                        // Detail: 2-cell trailing pad
                        color_spans.push((width.saturating_sub(2), CellColor::Fill));
                    }
                    frame.rows.push(RailRow {
                        kind: RailRowKind::Session(input.session.clone()),
                        text,
                        rect,
                        style,
                        color_spans,
                        flags: input.flags,
                        is_hovered: hovered == Some(&input.session),
                        fade,
                    });
                } else {
                    frame.rows.push(RailRow {
                        kind: RailRowKind::Gap,
                        text: fit("", width),
                        rect,
                        style: RailStyle::Empty,
                        color_spans: Vec::new(),
                        flags: TabFlags::default(),
                        is_hovered: false,
                    fade: None,
                    });
                }
            } else if logical == new_top {
                frame.new_rect = Some(rect);
                // New session row: "+" is NewIndicator (col 1), "New session" is NewText (col 2+).
                let text = fit(" + New session", width);
                let mut color_spans = Vec::new();
                color_spans.push((0, CellColor::Fill)); // leading space
                color_spans.push((1, CellColor::NewIndicator)); // "+"
                color_spans.push((2, CellColor::NewText)); // "New session"
                color_spans.push((width, CellColor::Fill)); // trailing pad
                frame.rows.push(RailRow {
                    kind: RailRowKind::New,
                    text,
                    rect,
                    style: RailStyle::NewControl,
                    color_spans,
                    flags: TabFlags::default(),
                    is_hovered: false,
                    fade: None,
                });
            } else {
                frame.rows.push(RailRow {
                    kind: RailRowKind::Empty,
                    text: fit("", width),
                    rect,
                    style: RailStyle::Empty,
                    color_spans: Vec::new(),
                    flags: TabFlags::default(),
                    is_hovered: false,
                    fade: None,
                });
            }
        }
        frame
    }

    /// Compact vertical rail projection.  The controls are deliberately kept
    /// as rows in the same frame model as expanded tabs: row 0 is the search
    /// cap, row 1 is search, row 2 is its lower cap/scroll-box lead, and the
    /// first item starts at row 3.  This is the pinned width-5 geometry from
    /// session-tabs-rail.test.tsx; rows containing caps/search are inert.
    fn render_compact_cells(
        inputs: &[RailInput],
        width: u16,
        height: u16,
        requested_scroll: usize,
        hovered: Option<&crate::native_navigation::SessionRef>,
        active_new: bool,
        search_hovered: bool,
    ) -> Self {
        let mut frame = Self {
            width,
            height,
            scroll_start: 0,
            scroll_cells: 0,
            rows: Vec::new(),
            new_rect: None,
            active_new,
            visible_close: None,
        };
        if width == 0 || height == 0 {
            return frame;
        }

        let content_top = 2usize;
        let content_height = usize::from(height).saturating_sub(content_top);
        let logical_len = 1usize.saturating_add(inputs.len().saturating_mul(2)).saturating_add(1);
        let maximum = logical_len.saturating_sub(content_height);
        let mut scroll = requested_scroll.min(maximum);
        let reveal = if active_new {
            Some(1usize.saturating_add(inputs.len().saturating_mul(2)))
        } else {
            inputs
                .iter()
                .position(|input| input.flags.active)
                .map(|index| 1usize.saturating_add(index.saturating_mul(2)))
        };
        if let Some(target) = reveal {
            if target < scroll {
                scroll = target;
            } else if target.saturating_add(1) > scroll.saturating_add(content_height) {
                scroll = target.saturating_add(1).saturating_sub(content_height);
            }
        }
        frame.scroll_cells = scroll.min(maximum);
        frame.scroll_start = frame.scroll_cells / usize::from(COMPACT_STRIDE);

        let centered = |value: &str| -> String {
            // A wide grapheme (or a narrow rail) must be clipped before the
            // centering arithmetic.  Otherwise padding is computed from the
            // clipped width while the original glyph still escapes the row.
            let mut value_cells = String::new();
            visit_unpadded_cells(value, usize::from(width), |glyph, _| value_cells.push_str(glyph));
            let value = value_cells;
            let used = Self::display_width(&value).min(usize::from(width));
            let left = usize::from(width).saturating_sub(used) / 2;
            let right = usize::from(width).saturating_sub(used).saturating_sub(left);
            format!("{}{}{}", " ".repeat(left), value, " ".repeat(right))
        };
        let first_grapheme = |value: &str| -> String {
            let mut first = String::new();
            crate::native_view_layout::visit_unpadded_cells(value.trim_start(), 4, |glyph, _| {
                if first.is_empty() {
                    first.push_str(glyph);
                }
            });
            if first.is_empty() { "U".to_owned() } else { first }
        };
        let inert = |y: u16, text: String, kind: RailRowKind, role: CellColor| RailRow {
            kind,
            text,
            rect: RailRect { x: 0, y, width, height: 1 },
            style: RailStyle::Empty,
            color_spans: vec![(0, role)],
            flags: TabFlags::default(),
            is_hovered: false,
                    fade: None,
        };

        for y in 0..height {
            if y == 0 {
                frame.rows.push(inert(y, "▄".repeat(usize::from(width)), RailRowKind::CapTop, CellColor::CapTop));
                continue;
            }
            if y == 1 {
                let mut row = inert(y, centered("⌕"), RailRowKind::Search, CellColor::CompactPadding);
                let start = width.saturating_sub(1) / 2;
                row.color_spans.extend([(start, CellColor::Search), (start + 1, CellColor::CompactPadding)]);
                frame.rows.push(row);
                continue;
            }

            let logical = frame.scroll_cells.saturating_add(usize::from(y).saturating_sub(content_top));
            if logical == 0 {
                // The row directly under the search controls is the first
                // item's `▄` cap; the controls' own absolute overlay
                // (`session-tabs-rail.tsx:54-66`) wins that cell and paints
                // `▀` while the search is hovered, `▄` while the item below is
                // highlighted (`belowHighlighted`), and a blank cell otherwise.
                let highlighted = inputs.first().is_some_and(|input| {
                    input.flags.active || hovered == Some(&input.session)
                });
                if search_hovered {
                    // `session-tabs-rail.tsx:54-66`: the controls' absolute
                    // overlay wins the cell directly below the search glyph and
                    // paints `▀` (fg = tint(raised.base, raised.high,
                    // raised.high.a)) while the search control is hovered. The
                    // overlay bg is that same tint when the row below is
                    // highlighted (`belowHighlighted`), else `raised.base`;
                    // `inert` marks the row `is_hovered = false`, so carry the
                    // decision on `is_hovered` (unused for this row kind).
                    let mut overlay = inert(
                        y,
                        "▀".repeat(usize::from(width)),
                        RailRowKind::SearchHover,
                        CellColor::SearchHover,
                    );
                    overlay.is_hovered = highlighted;
                    frame.rows.push(overlay);
                } else if highlighted {
                    frame.rows.push(inert(
                        y,
                        "▄".repeat(usize::from(width)),
                        RailRowKind::CapBottom,
                        CellColor::CapBottom,
                    ));
                } else {
                    frame.rows.push(inert(
                        y,
                        " ".repeat(usize::from(width)),
                        RailRowKind::Gap,
                        CellColor::CompactPadding,
                    ));
                }
                continue;
            }
            let item = logical.saturating_sub(1) / 2;
            let item_row = logical.saturating_sub(1) % 2 == 0;
            if !item_row {
        let highlighted = inputs.get(item).is_some_and(|input| {
            input.flags.active || hovered == Some(&input.session)
        });
                let text = if highlighted {
                    "▀".repeat(usize::from(width))
                } else {
                    " ".repeat(usize::from(width))
                };
                frame.rows.push(inert(
                    y,
                    text,
                    if highlighted { RailRowKind::CapBottom } else { RailRowKind::Gap },
                    if highlighted { CellColor::CapBottom } else { CellColor::CompactPadding },
                ));
                continue;
            }
            if let Some(input) = inputs.get(item) {
                let is_hovered = hovered == Some(&input.session);
                let style = if input.flags.needs_attention {
                    RailStyle::Attention
                } else if input.flags.busy {
                    RailStyle::Busy
                } else if is_hovered {
                    RailStyle::Hovered
                } else if input.flags.active {
                    RailStyle::Active
                } else {
                    RailStyle::Idle
                };
                let glyph = if input.flags.needs_attention {
                    "!".to_owned()
                } else if input.flags.busy {
                    "⠋".to_owned()
                } else {
                    first_grapheme(&input.title)
                };
                let close = is_hovered && width >= 4;
                let text_width = width.saturating_sub(if close { 2 } else { 0 });
                let mut glyph_cells = String::new();
                visit_unpadded_cells(&glyph, usize::from(text_width), |part, _| glyph_cells.push_str(part));
                let used = Self::display_width(&glyph_cells).min(usize::from(text_width));
                let left = usize::from(text_width).saturating_sub(used) / 2;
                let right = usize::from(text_width).saturating_sub(used).saturating_sub(left);
                let text = if close {
                    format!("{}{}{}✕ ", " ".repeat(left), glyph_cells, " ".repeat(right))
                } else {
                    format!("{}{}{}", " ".repeat(left), glyph_cells, " ".repeat(right))
                };
                if close {
                    frame.visible_close = Some((
                        input.session.clone(),
                        RailRect { x: width - 2, y, width: 1, height: 1 },
                    ));
                }
                let start = left as u16;
                let mut color_spans = vec![(0, CellColor::CompactPadding), (start, CellColor::Title), (start + used as u16, CellColor::CompactPadding)];
                if close {
                    color_spans.push((width - 2, CellColor::Close));
                    color_spans.push((width - 1, CellColor::CompactPadding));
                }
                frame.rows.push(RailRow {
                    kind: RailRowKind::Session(input.session.clone()),
                    text,
                    rect: RailRect { x: 0, y, width, height: 1 },
                    style,
                    color_spans,
                    flags: input.flags,
                    is_hovered,
                    fade: None,
                });
            } else if item == inputs.len() {
                frame.new_rect = Some(RailRect { x: 0, y, width, height: 1 });
                let text = centered("+");
                let start = (usize::from(width).saturating_sub(1) / 2) as u16;
                frame.rows.push(RailRow {
                    kind: RailRowKind::New,
                    text,
                    rect: RailRect { x: 0, y, width, height: 1 },
                    style: RailStyle::NewControl,
                    color_spans: vec![(0, CellColor::CompactPadding), (start, CellColor::NewIndicator), (start + 1, CellColor::CompactPadding)],
                    flags: TabFlags::default(),
                    is_hovered: false,
                    fade: None,
                });
            } else {
                frame.rows.push(inert(y, " ".repeat(usize::from(width)), RailRowKind::Gap, CellColor::CompactPadding));
            }
        }
        frame
    }

    pub fn hit(&self, x: u16, y: u16) -> Option<RailHit> {
        self.rows
            .iter()
            .find(|row| row.rect.contains(x, y))
            .and_then(|row| match &row.kind {
                RailRowKind::Session(id) => Some(RailHit::Session(id.clone())),
                RailRowKind::New => Some(RailHit::New),
                RailRowKind::Search => Some(RailHit::Search),
                _ => None,
            })
    }

    /// Resolve the exact painted session row for a captured mouse press.
    pub fn session_press_target(
        &self,
        x: u16,
        y: u16,
    ) -> Option<(crate::native_navigation::SessionRef, RailRect, bool)> {
        let RailHit::Session(session) = self.hit(x, y)? else {
            return None;
        };
        self.rows
            .iter()
            .find(|row| {
                row.rect.contains(x, y)
                    && matches!(&row.kind, RailRowKind::Session(id) if id == &session)
            })
            .map(|row| {
                (
                    session.clone(),
                    row.rect,
                    self.close_hit(x, y) == Some(&session),
                )
            })
    }

    pub fn search_hit(&self, x: u16, y: u16) -> bool {
        matches!(self.hit(x, y), Some(RailHit::Search))
    }

    /// Target slot for a captured drag at screen row `y`, mirroring upstream
    /// `drag()` (`session-tabs.tsx:685-702`): `floor((y - contentTop +
    /// scrollTop) / stride)`, clamped to `0..=tab_len-1`. Rows above the rail
    /// clamp to the first slot; rows on the New control or below clamp to the
    /// last. Uses the same stride and scroll facts the painted rows were
    /// built with, so no separate scroll arithmetic is needed.
    #[must_use]
    pub fn drag_slot(&self, y: u16, tab_len: usize) -> Option<usize> {
        if tab_len == 0 {
            return None;
        }
        let compact = self.width >= 5 && self.width < COMPACT_BREAKPOINT;
        let (content_top, stride) = if compact {
            (2u16, usize::from(COMPACT_STRIDE))
        } else {
            (TOP_PADDING, usize::from(TAB_STRIDE))
        };
        if y < content_top {
            return Some(0);
        }
        let logical = self
            .scroll_cells
            .saturating_add(usize::from(y - content_top));
        let slot = if compact {
            if logical == 0 {
                0
            } else {
                logical.saturating_sub(1) / stride
            }
        } else {
            logical / stride
        };
        Some(slot.min(tab_len - 1))
    }

    pub fn compact_cap_role(&self, y: u16) -> Option<CellColor> {
        self.rows.iter().find(|row| row.rect.contains(0, y)).and_then(|row| {
            row.color_spans.first().map(|(_, role)| *role)
        }).filter(|role| matches!(role, CellColor::CapTop | CellColor::CapBottom))
    }

    pub fn close_hit(&self, x: u16, y: u16) -> Option<&crate::native_navigation::SessionRef> {
        self.visible_close
            .as_ref()
            .filter(|(_, rect)| rect.contains(x, y))
            .map(|(session, _)| session)
    }

    pub fn display_width(text: &str) -> usize {
        visit_unpadded_cells(text, usize::MAX, |_, _| {})
    }
}

/// Visible grapheme clusters (the fade's `visibleTitleParts().length`).
fn grapheme_count(text: &str) -> usize {
    let mut count = 0usize;
    visit_unpadded_cells(text, usize::MAX, |_, _| count += 1);
    count
}

// ---------------------------------------------------------------------------
// Compact rail hover tooltip.
//
// Upstream `packages/tui/src/component/session-tabs.tsx:1349-1387`: while the
// vertical rail is compact and no drag/menu is active, a hovered tab shows a
// 4-row tooltip at `left = rail width`, width `min(54, cols - railWidth)`,
// `top = clamp(hoverY - 1, 0, rows - 4)` with title/detail truncated to
// `tooltipWidth - 2` cells.
// ---------------------------------------------------------------------------

/// `session-tabs.tsx:574-575` `tooltipWidth = min(54, dimensions.width - width)`.
pub const COMPACT_TOOLTIP_MAX_WIDTH: u16 = 54;
/// Upstream box height in `session-tabs.tsx:1352`.
pub const COMPACT_TOOLTIP_ROWS: u16 = 4;
/// `session-tabs.tsx:1356-1368` two text lines inside `paddingY={1}`.
pub const COMPACT_TOOLTIP_TEXT_INSET: u16 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactTooltip {
    pub rect: RailRect,
    pub title: String,
    pub detail: String,
}

/// Resolve the compact hover tooltip. `hover_y` is the pointer row inside the
/// rail's own coordinate space (the caller subtracts `screenY` as upstream
/// does at `session-tabs.tsx:1354`).
#[must_use]
pub fn compact_tooltip(
    rail_width: u16,
    cols: u16,
    rows: u16,
    hover_y: u16,
    hovered: bool,
    dragging: bool,
    menu_open: bool,
    title: &str,
    detail: &str,
) -> Option<CompactTooltip> {
    // `Show when={compact() && !dragging() && !contextMenu() && hovered()}`
    // at `session-tabs.tsx:1349`.
    if !hovered || dragging || menu_open || rail_width == 0 || rows == 0 {
        return None;
    }
    let width = COMPACT_TOOLTIP_MAX_WIDTH.min(cols.saturating_sub(rail_width));
    if width == 0 {
        return None;
    }
    let height = COMPACT_TOOLTIP_ROWS.min(rows);
    let top = hover_y
        .saturating_sub(1)
        .min(rows.saturating_sub(height));
    let text_width = width.saturating_sub(COMPACT_TOOLTIP_TEXT_INSET);
    Some(CompactTooltip {
        rect: RailRect {
            x: rail_width,
            y: top,
            width,
            height,
        },
        title: truncate_cells(title, text_width),
        detail: truncate_cells(detail, text_width),
    })
}

fn truncate_cells(value: &str, width: u16) -> String {
    let mut out = String::new();
    let mut used = 0usize;
    visit_unpadded_cells(value, usize::from(width), |glyph, cells| {
        if used + cells <= usize::from(width) {
            out.push_str(glyph);
            used += cells;
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rail_text_and_busy_indicator_match_pinned_reference_frame() {
        // Pinned OpenTUI component, 42x40, three sessions, active New,
        // animations=false: rail-status-home.txt (2026-10-28 capture).
        let mut inputs = sessions(3);
        for (index, input) in inputs.iter_mut().enumerate() {
            input.title = format!("Session {}", index + 1);
            input.flags.active = false;
        }
        inputs[1].flags.busy = true;
        let frame = RailFrame::render_cells(&inputs, 42, 40, 0, None, true);
        assert_eq!(frame.rows[1].text.trim_end(), "   Session 1");
        assert_eq!(frame.rows[2].text.trim_end(), "   project");
        assert_eq!(frame.rows[4].text.trim_end(), " ⠋ Session 2");
        assert_eq!(frame.rows[10].text.trim_end(), " + New session");
    }

    #[test]
    fn close_child_is_visible_and_actionable_only_on_hovered_title_row() {
        let inputs = sessions(2);
        let resting = RailFrame::render(&inputs, 42, 10, 0, None);
        assert_eq!(resting.close_hit(40, 1), None);
        assert!(!resting.rows.iter().any(|row| row.text.contains('✕')));
        let hovered = RailFrame::render(&inputs, 42, 10, 0, Some(&inputs[0].session));
        assert_eq!(hovered.close_hit(40, 1), Some(&inputs[0].session));
        assert_eq!(hovered.close_hit(40, 2), None, "detail row never closes");
        assert_eq!(hovered.close_hit(41, 1), None, "right padding never closes");
        let title = hovered
            .rows
            .iter()
            .find(|row| row.kind == RailRowKind::Session(inputs[0].session.clone()))
            .unwrap();
        assert_eq!(
            crate::native_view_layout::slice_cells(title.text.lines().next().unwrap(), 40, 1),
            "✕"
        );
        let moved = RailFrame::render(&inputs, 42, 10, 0, Some(&inputs[1].session));
        assert_eq!(
            moved.close_hit(40, 1),
            None,
            "old hover loses close authority"
        );
        for width in 0..4 {
            let tiny = RailFrame::render(&inputs, width, 10, 0, Some(&inputs[0].session));
            assert!(tiny.visible_close.is_none());
        }
    }

    #[test]
    fn expanded_title_width_matches_the_painted_first_row() {
        let inputs = sessions(2);
        let width = 30;
        let indicator = inputs.len().to_string().len().max(2) as u16;
        let pad = indicator + 1;
        assert_eq!(expanded_title_width(width, 2, false), width - pad - 1);
        assert_eq!(expanded_title_width(width, 2, true), width - pad - 2);
        let hovered = RailFrame::render(&inputs, width, 10, 0, Some(&inputs[0].session));
        let row = hovered
            .rows
            .iter()
            .find(|row| row.kind == RailRowKind::Session(inputs[0].session.clone()))
            .unwrap();
        let line = row.text.lines().next().unwrap();
        assert_eq!(RailFrame::display_width(line), usize::from(width));
        // prefix + title budget + "✕ " consumes the full rail width.
        assert_eq!(
            RailFrame::display_width(line),
            usize::from(pad + expanded_title_width(width, 2, true) + 2)
        );
        // Below the 4-column close threshold the hovered budget matches resting.
        assert_eq!(expanded_title_width(3, 2, true), expanded_title_width(3, 2, false));
    }

    #[test]
    fn expanded_rows_carry_the_four_cell_fade_window() {
        // session-tabs.tsx:754-766,812-818: the title fades against exactly
        // the visible clusters of the painted budget; the detail line fades
        // on its own overflow and never scrolls.
        let inputs = sessions(2);
        let frame = RailFrame::render(&inputs, 20, 10, 0, None);
        let title = frame
            .rows
            .iter()
            .find(|row| row.kind == RailRowKind::Session(inputs[0].session.clone()))
            .unwrap();
        let fade = title.fade.expect("17 cells overflow a 16-cell budget");
        assert_eq!((fade.start, fade.length), (3, 13), "界面😀 + 9 ASCII clusters fit 16 cells");
        assert!(!fade.leading);
        assert_eq!(fade.weight(0), 0.0, "at rest the leading ramp is disabled");
        assert_eq!(fade.weight(8), 0.0);
        assert!((fade.weight(12) - 0.92).abs() < 1e-6);
        let detail = frame
            .rows
            .iter()
            .filter(|row| row.kind == RailRowKind::Session(inputs[0].session.clone()))
            .nth(1)
            .unwrap();
        assert_eq!(detail.fade, None, "the short detail never fades");

        let hovered = RailFrame::render(&inputs, 20, 10, 0, Some(&inputs[0].session));
        let title = hovered
            .rows
            .iter()
            .find(|row| row.kind == RailRowKind::Session(inputs[0].session.clone()))
            .unwrap();
        let fade = title.fade.expect("hovered close shrinks the budget to 15 cells");
        assert_eq!((fade.start, fade.length), (3, 12));

        let mut scrolling = sessions(2);
        scrolling[0].marquee_offset = 4;
        let frame = RailFrame::render(&scrolling, 20, 10, 0, None);
        let title = frame
            .rows
            .iter()
            .find(|row| row.kind == RailRowKind::Session(scrolling[0].session.clone()))
            .unwrap();
        let fade = title.fade.expect("the marquee window still fades");
        assert_eq!((fade.start, fade.length), (3, 15), "12 title clusters + the · gap");
        assert!(fade.leading, "a scrolling title ramps its leading edge");
        assert!((fade.weight(0) - 0.92).abs() < 1e-6);
    }

    #[test]
    fn hovered_close_fits_narrow_rails_without_overrunning() {
        let inputs = sessions(1);
        for width in 0..=6 {
            let frame = RailFrame::render(&inputs, width, 6, 0, Some(&inputs[0].session));
            for row in &frame.rows {
                assert!(RailFrame::display_width(&row.text) <= usize::from(width));
            }
            if width >= 4 {
                assert_eq!(frame.close_hit(width - 2, 1), Some(&inputs[0].session));
            }
        }
    }

    #[test]
    fn mouse_press_captures_title_detail_and_clipped_detail_rows() {
        let inputs = sessions(2);
        let frame = RailFrame::render(&inputs, 42, 10, 0, None);
        for y in [1, 2] {
            let (session, rect, close) = frame
                .session_press_target(0, y)
                .expect("painted session row is actionable");
            assert_eq!(session, inputs[0].session);
            assert!(
                rect.contains(0, y),
                "captured row must contain the press coordinate"
            );
            assert!(!close);
        }

        let mut many = sessions(32);
        for input in &mut many {
            input.flags.active = false;
        }
        let clipped = RailFrame::render_cells(&many, 42, 40, 0, None, true);
        assert_eq!(clipped.scroll_cells, 58);
        let (session, rect, close) = clipped
            .session_press_target(0, 1)
            .expect("clipped detail remains selectable");
        assert_eq!(session, many[19].session);
        assert!(
            rect.contains(0, 1),
            "clipped-detail press must capture its painted row"
        );
        assert!(!close, "clipped detail never exposes close action");
    }

    #[test]
    fn tab_15_drag_slot_maps_rows_scroll_and_clamping() {
        // session-tabs.tsx:685-702: floor((y - contentTop + scrollTop)/stride)
        // clamped to the open-tab range, sharing the painted stride/scroll.
        let inputs = sessions(3);
        let frame = RailFrame::render_cells(&inputs, 42, 10, 0, None, false);
        assert_eq!(frame.drag_slot(0, 3), Some(0), "above the rail clamps to the first slot");
        assert_eq!(frame.drag_slot(1, 3), Some(0));
        assert_eq!(frame.drag_slot(3, 3), Some(0));
        assert_eq!(frame.drag_slot(4, 3), Some(1));
        assert_eq!(frame.drag_slot(6, 3), Some(1), "gap rows stay in the upper slot");
        assert_eq!(frame.drag_slot(7, 3), Some(2));
        assert_eq!(frame.drag_slot(9, 3), Some(2), "New/below clamps to the last slot");
        assert_eq!(frame.drag_slot(1, 0), None, "no open tabs has no target");

        let clipped = RailFrame::render_cells(&sessions(32), 42, 40, 0, None, true);
        assert_eq!(clipped.scroll_cells, 58);
        assert_eq!(clipped.drag_slot(1, 32), Some(19), "scrolled rows use painted cells");

        let compact = RailFrame::render_cells(&inputs, 6, 10, 0, None, false);
        assert_eq!(compact.drag_slot(1, 3), Some(0));
        assert_eq!(compact.drag_slot(2, 3), Some(0));
        assert_eq!(compact.drag_slot(4, 3), Some(0));
        assert_eq!(compact.drag_slot(5, 3), Some(1));
        assert_eq!(compact.drag_slot(9, 3), Some(2));
    }

    fn sessions(count: usize) -> Vec<RailInput> {
        (0..count)
            .map(|index| RailInput {
                session: crate::native_navigation::SessionRef(format!("tab-{index}")),
                title: "界面😀 long title".into(),
                detail: "project".into(),
                flags: TabFlags {
                    active: index + 1 == count,
                    busy: false,
                    needs_attention: false,
                },
                marquee_offset: 0,
            })
            .collect()
    }

    #[test]
    fn every_painted_cell_is_bounded_and_unambiguous() {
        for width in [0, 1, 2, 42] {
            for height in [0, 1, 2, 3, 6, 20] {
                for count in [0, 1, 32] {
                    let frame =
                        RailFrame::render(&sessions(count), width, height, usize::MAX, None);
                    if width == 0 || height == 0 {
                        assert!(frame.rows.is_empty());
                        continue;
                    }
                    let mut covered = vec![false; usize::from(height)];
                    for row in &frame.rows {
                        assert!(row.rect.y + row.rect.height <= height);
                        for line in row.text.split('\n') {
                            assert!(RailFrame::display_width(line) <= usize::from(width));
                        }
                        for y in row.rect.y..row.rect.y + row.rect.height {
                            assert!(!covered[usize::from(y)], "overlapping row {y}");
                            covered[usize::from(y)] = true;
                        }
                    }
                    assert!(covered.into_iter().all(|cell| cell));
                    if let Some(rect) = frame.new_rect {
                        assert_eq!(frame.hit(rect.x, rect.y), Some(RailHit::New));
                    }
                }
            }
        }
    }

    #[test]
    fn active_tail_reveals_new_when_complete_logical_tail_fits() {
        let inputs = sessions(32);
        let frame = RailFrame::render(&inputs, 42, 11, 0, None);
        assert_eq!(
            frame.scroll_cells, 85,
            "active session follows upstream exact-cell reveal"
        );
        assert_eq!(
            frame.scroll_start, 28,
            "item offset is display metadata only"
        );
        assert!(frame.rows.iter().any(|row| row.kind
            == RailRowKind::Session(inputs[31].session.clone())
            && row.rect.y == 9));
        assert_eq!(
            frame.hit(0, 9),
            Some(RailHit::Session(inputs[31].session.clone()))
        );
        assert_eq!(
            frame.hit(0, 10),
            Some(RailHit::Session(inputs[31].session.clone()))
        );
        assert!(
            frame.new_rect.is_none(),
            "inactive New child remains below the viewport"
        );
    }

    #[test]
    fn active_home_reveals_new_after_retained_sessions() {
        let mut inputs = sessions(32);
        for input in &mut inputs {
            input.flags.active = false;
        }
        let inactive = RailFrame::render_with_active_new(&inputs, 42, 6, 0, None, false);
        assert_eq!(inactive.scroll_start, 0);
        assert!(inactive.new_rect.is_none());
        let active = RailFrame::render_with_active_new(&inputs, 42, 6, 0, None, true);
        let rect = active
            .new_rect
            .expect("active Home reveals final New child");
        assert_eq!(active.hit(rect.x, rect.y), Some(RailHit::New));
        assert!(active.scroll_start > 0);
    }

    #[test]
    fn active_home_keeps_cell_scroll_and_clipped_detail_at_exact_bottom() {
        let mut inputs = sessions(32);
        for input in &mut inputs {
            input.flags.active = false;
        }
        let frame = RailFrame::render_cells(&inputs, 42, 40, 0, Some(&inputs[19].session), true);
        assert_eq!(frame.scroll_cells, 58);
        assert_eq!(
            frame.new_rect,
            Some(RailRect {
                x: 0,
                y: 39,
                width: 42,
                height: 1
            })
        );
        assert_eq!(
            frame.hit(0, 1),
            Some(RailHit::Session(inputs[19].session.clone()))
        );
        assert_eq!(frame.rows[1].text.trim(), "project");
        assert_eq!(
            frame.close_hit(40, 1),
            None,
            "clipped title has no close authority"
        );
        assert_eq!(frame.hit(0, 39), Some(RailHit::New));
        let repaint = RailFrame::render_cells(&inputs, 42, 40, frame.scroll_cells, None, true);
        assert_eq!(repaint.scroll_cells, 58);
        assert_eq!(repaint.new_rect, frame.new_rect);
    }

    #[test]
    fn active_home_reveal_is_bounded_for_short_viewports() {
        let mut inputs = sessions(8);
        for input in &mut inputs {
            input.flags.active = false;
        }
        for height in 0..=12 {
            let frame = RailFrame::render_with_active_new(&inputs, 42, height, 0, None, true);
            assert!(frame
                .rows
                .iter()
                .all(|row| row.rect.y.saturating_add(row.rect.height) <= height));
            if height > TOP_PADDING {
                let rect = frame.new_rect.expect("New fits after fixed top padding");
                assert_eq!(frame.hit(rect.x, rect.y), Some(RailHit::New));
            } else {
                assert!(frame.new_rect.is_none());
            }
        }
    }

    #[test]
    fn active_reveal_does_not_pin_new_when_logical_tail_is_offscreen() {
        let inputs = sessions(32);
        let frame = RailFrame::render(&inputs, 42, 6, 0, None);
        assert_eq!(
            frame.scroll_start, 30,
            "active session follows upstream reveal rule"
        );
        assert!(frame.rows.iter().any(|row| row.kind
            == RailRowKind::Session(inputs[31].session.clone())
            && row.rect.y == 4));
        assert_eq!(
            frame.hit(0, 4),
            Some(RailHit::Session(inputs[31].session.clone()))
        );
        assert!(frame.new_rect.is_none());
        assert_eq!(frame.hit(0, 6), None, "outside viewport must not hit");
    }

    #[test]
    fn new_uses_complete_logical_list_not_visible_slice() {
        let mut inputs = sessions(8);
        for input in &mut inputs {
            input.flags.active = false;
        }
        let top = RailFrame::render(&inputs, 42, 6, 0, None);
        assert!(
            top.new_rect.is_none(),
            "top window must not append New after visible slice"
        );
        let bottom = RailFrame::render(&inputs, 42, 6, 7, None);
        assert_eq!(bottom.scroll_cells, 20);
        assert_eq!(bottom.scroll_start, 6);
        assert!(bottom.rows.iter().any(|row| row.kind
            == RailRowKind::Session(inputs[7].session.clone())
            && row.rect.y == 2));
        assert_eq!(
            bottom.hit(0, 2),
            Some(RailHit::Session(inputs[7].session.clone()))
        );
        assert_eq!(
            bottom.hit(0, 3),
            Some(RailHit::Session(inputs[7].session.clone()))
        );
        assert!(bottom
            .rows
            .iter()
            .any(|row| row.kind == RailRowKind::Gap && row.rect.y == 4));
        assert_eq!(
            bottom.new_rect,
            Some(RailRect {
                x: 0,
                y: 5,
                width: 42,
                height: 1
            })
        );
        assert_eq!(bottom.hit(0, 5), Some(RailHit::New));
    }

    #[test]
    fn requested_max_scroll_reveals_new_without_hiding_active_session() {
        let inputs = sessions(8);
        let frame = RailFrame::render(&inputs, 42, 6, 7, None);
        assert_eq!(frame.scroll_cells, 20);
        assert_eq!(frame.scroll_start, 6);
        assert!(frame.rows.iter().any(|row| row.kind
            == RailRowKind::Session(inputs[7].session.clone())
            && row.rect.y == 2));
        assert_eq!(
            frame.hit(0, 2),
            Some(RailHit::Session(inputs[7].session.clone()))
        );
        assert_eq!(
            frame.hit(0, 3),
            Some(RailHit::Session(inputs[7].session.clone()))
        );
        assert!(frame
            .rows
            .iter()
            .any(|row| row.kind == RailRowKind::Gap && row.rect.y == 4));
        assert_eq!(
            frame.new_rect,
            Some(RailRect {
                x: 0,
                y: 5,
                width: 42,
                height: 1
            })
        );
        assert_eq!(frame.hit(0, 5), Some(RailHit::New));
    }

    #[test]
    fn zero_items_new_is_after_padding_and_short_heights_are_bounded() {
        let empty = RailFrame::render(&[], 42, 6, 0, None);
        assert_eq!(
            empty.new_rect,
            Some(RailRect {
                x: 0,
                y: 1,
                width: 42,
                height: 1
            })
        );
        assert_eq!(empty.hit(41, 1), Some(RailHit::New));
        for height in 0..=4 {
            let frame = RailFrame::render(&[], 42, height, usize::MAX, None);
            assert!(frame
                .rows
                .iter()
                .all(|row| row.rect.y.saturating_add(row.rect.height) <= height));
            match frame.new_rect {
                Some(rect) => {
                    assert_eq!(frame.hit(rect.x, rect.y), Some(RailHit::New));
                    assert!(rect.y < height);
                    assert!(
                        frame.hit(0, height).is_none(),
                        "outside viewport must not hit"
                    );
                }
                None => {
                    assert!(frame.hit(0, height.saturating_sub(1)).is_none());
                }
            }
        }
    }

    #[test]
    fn new_exact_geometry_for_short_list_and_gap() {
        let inputs = sessions(2);
        let frame = RailFrame::render(&inputs, 42, 10, 0, None);
        assert_eq!(
            frame.new_rect,
            Some(RailRect {
                x: 0,
                y: 7,
                width: 42,
                height: 1
            })
        );
        assert_eq!(frame.hit(41, 7), Some(RailHit::New));
        assert_eq!(frame.hit(41, 8), None);
    }

    #[test]
    fn short_heights_hit_only_painted_controls() {
        let inputs = sessions(8);
        for height in 0..=4 {
            let frame = RailFrame::render(&inputs, 42, height, 0, None);
            assert!(frame
                .rows
                .iter()
                .all(|row| row.rect.y.saturating_add(row.rect.height) <= height));
            for y in 0..height {
                let painted = frame
                    .rows
                    .iter()
                    .find(|row| row.rect.contains(0, y))
                    .and_then(|row| match &row.kind {
                        RailRowKind::Session(id) => Some(RailHit::Session(id.clone())),
                        RailRowKind::New => Some(RailHit::New),
                        RailRowKind::Search | RailRowKind::CapTop | RailRowKind::CapBottom => None,
                        _ => None,
                    });
                assert_eq!(frame.hit(0, y), painted, "height={height} y={y}");
                assert_eq!(frame.hit(42, y), None, "outside rail width must not hit");
            }
            assert_eq!(frame.hit(0, height), None, "outside viewport must not hit");
        }
    }

    // ── Paint / color-span regression tests ────────────────────────────────

    #[test]
    fn color_spans_distinguish_title_detail_spinner_close_and_fill() {
        // Three sessions, active New, session 1 hovered (idle+hovered -> element bg).
        let mut inputs = sessions(3);
        for (i, input) in inputs.iter_mut().enumerate() {
            input.title = format!("Session {}", i + 1);
            input.flags.active = false;
        }
        let hover = &inputs[0].session;
        let frame = RailFrame::render_cells(&inputs, 42, 40, 0, Some(hover), true);

        // Row 1: Session 1 title (hovered -> Hovered style)
        let title_row = &frame.rows[1];
        assert_eq!(title_row.style, RailStyle::Hovered);
        // indicator_width = max(2, 1) = 2; pad = 3
        assert!(title_row
            .color_spans
            .iter()
            .any(|(c, r)| *c == 0 && *r == CellColor::Fill));
        assert!(title_row
            .color_spans
            .iter()
            .any(|(c, r)| *c == 3 && *r == CellColor::Title));
        assert!(
            title_row
                .color_spans
                .iter()
                .any(|(_, r)| *r == CellColor::Close),
            "hovered title has Close role"
        );
        let close_col = title_row
            .color_spans
            .iter()
            .find(|(_, r)| *r == CellColor::Close)
            .unwrap()
            .0;
        assert_eq!(close_col, 40, "close indicator at width-2");

        // Row 2: Session 1 detail (Idle style, Detail role)
        let detail_row = &frame.rows[2];
        assert_eq!(detail_row.style, RailStyle::Hovered);
        assert!(detail_row
            .color_spans
            .iter()
            .any(|(c, r)| *c == 3 && *r == CellColor::Detail));
    }

    #[test]
    fn row_paint_session_row_matches_fixture_colors() {
        use crate::native_theme_runtime::{Palette, Slot};
        let dark = Palette::opencode_dark();

        // Idle: bg=panel, title_fg=TextMuted, title_attr=0
        let idle_flags = TabFlags {
            active: false,
            busy: false,
            needs_attention: false,
        };
        let idle = RowPaint::session_row(&dark, &idle_flags, false);
        assert_eq!(idle.background, dark.get(Slot::BackgroundPanel));
        assert_eq!(idle.title_fg, dark.get(Slot::TextMuted));
        assert_eq!(idle.title_attr, 0);

        // Selected: bg=panel (actionSelected transparent), title_fg=Text, title_attr=1
        let sel_flags = TabFlags {
            active: true,
            busy: false,
            needs_attention: false,
        };
        let sel = RowPaint::session_row(&dark, &sel_flags, false);
        assert_eq!(sel.background, dark.get(Slot::BackgroundPanel));
        assert_eq!(sel.title_fg, dark.get(Slot::Text));
        assert_eq!(sel.title_attr, 1);

        // Hovered-only: bg=element, title_fg=Text, title_attr=0
        let hover = RowPaint::session_row(&dark, &idle_flags, true);
        assert_eq!(hover.background, dark.get(Slot::BackgroundElement));
        assert_eq!(hover.title_fg, dark.get(Slot::Text));
        assert_eq!(hover.title_attr, 0);

        // Selected+hovered: bg=panel (selected wins), title_fg=Text, title_attr=1
        let sel_hover = RowPaint::session_row(&dark, &sel_flags, true);
        assert_eq!(sel_hover.background, dark.get(Slot::BackgroundPanel));
        assert_eq!(sel_hover.title_fg, dark.get(Slot::Text));
        assert_eq!(sel_hover.title_attr, 1);

        // Attention: bg=panel, title_fg=TextMuted (idle), bold
        let att_flags = TabFlags {
            active: false,
            busy: false,
            needs_attention: true,
        };
        let att = RowPaint::session_row(&dark, &att_flags, false);
        assert_eq!(att.title_attr, 0, "attention alone is not bold");
        assert_eq!(att.title_fg, dark.get(Slot::TextMuted));

        // Attention + hovered: bg=element, fg=Text, bold
        let att_hover = RowPaint::session_row(&dark, &att_flags, true);
        assert_eq!(att_hover.background, dark.get(Slot::BackgroundElement));
        assert_eq!(att_hover.title_fg, dark.get(Slot::Text));
        assert_eq!(att_hover.title_attr, 0);

        // Busy (not selected): bg=panel, title_fg=TextMuted, has_busy=true, spinner=Primary
        let busy_flags = TabFlags {
            active: false,
            busy: true,
            needs_attention: false,
        };
        let busy = RowPaint::session_row(&dark, &busy_flags, false);
        assert_eq!(busy.background, dark.get(Slot::BackgroundPanel));
        assert_eq!(busy.title_attr, 0);
        assert_eq!(busy.spinner_fg, dark.get(Slot::Primary));

        // Busy + selected: bg=panel, fg=Text, bold, has_busy=true
        let busy_sel = RowPaint::session_row(
            &dark,
            &TabFlags {
                active: true,
                busy: true,
                needs_attention: false,
            },
            false,
        );
        assert_eq!(busy_sel.title_fg, dark.get(Slot::Text));
        assert_eq!(busy_sel.title_attr, 1);
        assert_eq!(busy_sel.spinner_fg, dark.get(Slot::Primary));

        // Close indicator always TextMuted
        assert_eq!(idle.close_fg, dark.get(Slot::TextMuted));
        assert_eq!(sel.close_fg, dark.get(Slot::TextMuted));
    }

    #[test]
    fn row_paint_detail_fg_uses_tint_textmuted_bg_035() {
        use crate::native_theme_runtime::{Palette, Slot};
        let dark = Palette::opencode_dark();
        let flags = TabFlags {
            active: true,
            busy: false,
            needs_attention: false,
        };
        let paint = RowPaint::session_row(&dark, &flags, false);
        // detail_fg = tint(textMuted, bg, 0.35) = tint([128,128,128], [20,20,20], 0.35)
        // = 128 + (20-128)*0.35 = 128 - 37.8 = 90.2 -> 90
        assert_eq!(paint.detail_fg, [90, 90, 90, 255]);
    }

    #[test]
    fn tint_matches_upstream_formula() {
        // tint(base, overlay, alpha) = round(base + (overlay - base) * alpha)
        let result = tint([238, 238, 238, 255], [20, 20, 20, 255], 0.55);
        assert_eq!(result, [118, 118, 118, 255]);
        let light = tint([26, 26, 26, 255], [250, 250, 250, 255], 0.55);
        assert_eq!(light, [149, 149, 149, 255]);
        // Alpha channel preserved from base
        let with_alpha = tint([100, 100, 100, 200], [50, 50, 50, 255], 0.5);
        assert_eq!(with_alpha[3], 200);
    }

    #[test]
    fn idle_number_matches_upstream_tint() {
        use crate::native_theme_runtime::Palette;
        let dark = Palette::opencode_dark();
        assert_eq!(idle_number(&dark), [118, 118, 118, 255]);
        let light = Palette::opencode_light();
        assert_eq!(idle_number(&light), [149, 149, 149, 255]);
    }

    #[test]
    fn new_control_row_colors_match_fixtures() {
        use crate::native_theme_runtime::{Palette, Slot};
        let dark = Palette::opencode_dark();

        // newTab=false, addHovered=false
        let paint = RowPaint::new_control_row(&dark, false, false);
        assert_eq!(paint.plus_fg, [118, 118, 118, 255]); // idleNumber
        assert_eq!(paint.new_text_fg, dark.get(Slot::TextMuted));
        assert_eq!(paint.background, dark.get(Slot::BackgroundPanel));
        assert_eq!(paint.new_attr, 0, "not newTab -> not bold");

        // newTab=true: both use Text, bold
        let paint_new = RowPaint::new_control_row(&dark, true, false);
        assert_eq!(paint_new.plus_fg, dark.get(Slot::Text));
        assert_eq!(paint_new.new_text_fg, dark.get(Slot::Text));
        assert_eq!(paint_new.new_attr, 1, "newTab -> bold");

        // addHovered=true: both use Text, not bold
        let paint_hover = RowPaint::new_control_row(&dark, false, true);
        assert_eq!(paint_hover.plus_fg, dark.get(Slot::Text));
        assert_eq!(paint_hover.new_text_fg, dark.get(Slot::Text));
        assert_eq!(paint_hover.new_attr, 0, "addHovered does not bold");

        let light = Palette::opencode_light();
        let paint_light = RowPaint::new_control_row(&light, false, false);
        assert_eq!(paint_light.plus_fg, [149, 149, 149, 255]);
        assert_eq!(paint_light.new_text_fg, light.get(Slot::TextMuted));
    }

    #[test]
    fn new_session_row_has_correct_color_spans() {
        let inputs: Vec<RailInput> = (0..1)
            .map(|index| RailInput {
                session: crate::native_navigation::SessionRef(format!("tab-{index}")),
                title: "Session 1".into(),
                detail: "project".into(),
                flags: TabFlags {
                    active: false,
                    busy: false,
                    needs_attention: false,
                },
                marquee_offset: 0,
            })
            .collect();
        let frame = RailFrame::render_with_active_new(&inputs, 42, 10, 0, None, true);
        let new_row = frame
            .rows
            .iter()
            .find(|row| matches!(row.kind, RailRowKind::New))
            .expect("New row present");
        assert_eq!(new_row.style, RailStyle::NewControl);
        assert!(new_row
            .color_spans
            .iter()
            .any(|(c, r)| *c == 0 && *r == CellColor::Fill));
        assert!(new_row
            .color_spans
            .iter()
            .any(|(c, r)| *c == 1 && *r == CellColor::NewIndicator));
        assert!(new_row
            .color_spans
            .iter()
            .any(|(c, r)| *c == 2 && *r == CellColor::NewText));
    }

    #[test]
    fn busy_row_has_spinner_role_in_color_spans() {
        let mut inputs = sessions(1);
        inputs[0].flags.busy = true;
        inputs[0].flags.active = false;
        let frame = RailFrame::render_cells(&inputs, 42, 10, 0, None, true);
        let title_row = &frame.rows[1];
        assert_eq!(title_row.style, RailStyle::Busy);
        assert!(
            title_row.text.contains('\u{280B}'),
            "busy row has spinner char"
        );
        assert!(
            title_row
                .color_spans
                .iter()
                .any(|(_, r)| *r == CellColor::Spinner),
            "busy row has Spinner cell role"
        );
    }

    #[test]
    fn close_indicator_has_close_role_when_hovered() {
        let inputs = sessions(1);
        let frame = RailFrame::render(&inputs, 42, 10, 0, Some(&inputs[0].session));
        let title_row = frame
            .rows
            .iter()
            .find(|row| {
                row.rect.y == 1
                    && matches!(&row.kind, RailRowKind::Session(id) if id == &inputs[0].session)
            })
            .expect("hovered title row");
        assert!(
            title_row.text.contains('\u{2715}'),
            "hovered title has close indicator"
        );
        let close_span = title_row
            .color_spans
            .iter()
            .find(|(_, r)| *r == CellColor::Close);
        assert!(close_span.is_some(), "hovered row has Close cell role");
        let close_col = close_span.unwrap().0;
        assert!(
            close_col > 0 && close_col < 42,
            "close column is within bounds"
        );
    }

    #[test]
    fn titles_with_spinner_glyph_use_title_role_not_spinner() {
        let mut inputs = sessions(1);
        inputs[0].title = "\u{280B} Session with braille".to_owned();
        inputs[0].flags.busy = false;
        let frame = RailFrame::render_cells(&inputs, 42, 10, 0, None, true);
        let title_row = &frame.rows[1];
        assert!(
            !title_row
                .color_spans
                .iter()
                .any(|(_, r)| *r == CellColor::Spinner),
            "title containing spinner glyph is not classified as Spinner"
        );
    }

    #[test]
    fn narrow_rail_preserves_close_role() {
        let inputs = sessions(1);
        let frame = RailFrame::render(&inputs, 10, 10, 0, Some(&inputs[0].session));
        if frame.visible_close.is_some() {
            let title_row = frame.rows.iter().find(|row| row.rect.y == 1).unwrap();
            assert!(
                title_row
                    .color_spans
                    .iter()
                    .any(|(_, r)| *r == CellColor::Close),
                "narrow rail still has Close role"
            );
        }
    }

    #[test]
    fn row_paint_returns_raw_palette_values() {
        use crate::native_theme_runtime::Palette;
        let palette = Palette::opencode_dark();
        let flags = TabFlags {
            active: true,
            busy: false,
            needs_attention: false,
        };
        let paint = RowPaint::session_row(&palette, &flags, false);
        assert_eq!(paint.background[3], 255, "palette always opaque");
        assert_eq!(paint.title_fg[3], 255, "foreground always opaque");
    }

    #[test]
    fn compact_width_five_matches_pinned_row_geometry_and_hit_targets() {
        let inputs = vec![
            RailInput {
                session: crate::native_navigation::SessionRef("first".into()),
                title: "First session".into(),
                detail: "project-alpha".into(),
                flags: TabFlags { active: true, busy: false, needs_attention: false },
                marquee_offset: 0,
            },
            RailInput {
                session: crate::native_navigation::SessionRef("second".into()),
                title: "Second session".into(),
                detail: "project-alpha".into(),
                flags: TabFlags::default(),
                marquee_offset: 0,
            },
            RailInput {
                session: crate::native_navigation::SessionRef("third".into()),
                title: "Third session".into(),
                detail: "project-alpha".into(),
                flags: TabFlags::default(),
                marquee_offset: 0,
            },
        ];
        let frame = RailFrame::render_cells(&inputs, 5, 24, 0, None, false);
        assert_eq!(frame.rows[1].text, "  ⌕  ");
        assert_eq!(frame.rows[3].text, "  F  ");
        assert_eq!(frame.rows[5].text, "  S  ");
        assert_eq!(frame.rows[7].text, "  T  ");
        assert_eq!(frame.rows[9].text, "  +  ");
        assert_eq!(frame.hit(2, 3), Some(RailHit::Session(inputs[0].session.clone())));
        assert_eq!(frame.hit(2, 5), Some(RailHit::Session(inputs[1].session.clone())));
        assert_eq!(frame.hit(2, 7), Some(RailHit::Session(inputs[2].session.clone())));
        assert_eq!(frame.hit(2, 9), Some(RailHit::New));
        assert_eq!(frame.hit(2, 1), Some(RailHit::Search));
    }

    // TAB-11: the controls' absolute overlay row (`session-tabs-rail.tsx:54-66`)
    // is `▀` while the search is hovered, `▄` while the item directly below is
    // highlighted (`belowHighlighted`), and a blank cell otherwise. The Rust
    // model derives the "search hovered" fact from the first item being hovered
    // (the closest honest native equivalent) and must never paint `▀▀▀▀▀` over
    // a non-highlighted neighbour.
    #[test]
    fn compact_controls_overlay_row_uses_below_highlighted_half_block() {
        let inputs = vec![
            RailInput {
                session: crate::native_navigation::SessionRef("first".into()),
                title: "First session".into(),
                detail: "project-alpha".into(),
                flags: TabFlags { active: true, busy: false, needs_attention: false },
                marquee_offset: 0,
            },
            RailInput {
                session: crate::native_navigation::SessionRef("second".into()),
                title: "Second session".into(),
                detail: "project-alpha".into(),
                flags: TabFlags::default(),
                marquee_offset: 0,
            },
        ];
        // Nothing highlighted: the overlay cell is blank (not a cap glyph).
        let idle_inputs = vec![
            RailInput {
                session: crate::native_navigation::SessionRef("first".into()),
                title: "First session".into(),
                detail: "project-alpha".into(),
                flags: TabFlags::default(),
                marquee_offset: 0,
            },
            RailInput {
                session: crate::native_navigation::SessionRef("second".into()),
                title: "Second session".into(),
                detail: "project-alpha".into(),
                flags: TabFlags::default(),
                marquee_offset: 0,
            },
        ];
        let idle = RailFrame::render_cells(&idle_inputs, 5, 24, 0, None, false);
        assert_eq!(idle.rows[2].text, "     ");
        assert_eq!(idle.rows[2].kind, RailRowKind::Gap);
        assert_eq!(idle.compact_cap_role(2), None);

        // Hovering a *non-active* first item still highlights the cell directly
        // under the controls: `▄`, the bottom half, same as the active case.
        let hovered = RailFrame::render_cells(
            &idle_inputs,
            5,
            24,
            0,
            Some(&idle_inputs[0].session),
            false,
        );
        assert_eq!(hovered.rows[2].text, "▄▄▄▄▄");
        assert_eq!(hovered.rows[2].kind, RailRowKind::CapBottom);
        assert_eq!(hovered.compact_cap_role(2), Some(CellColor::CapBottom));

        // An active first item also paints the bottom half (its own cap).
        let active = RailFrame::render_cells(&inputs, 5, 24, 0, None, false);
        assert_eq!(active.rows[2].text, "▄▄▄▄▄");
        assert_eq!(active.rows[2].kind, RailRowKind::CapBottom);

        // Only the far end of the rail paints the top half
        // (`SessionTabHalfRow edge="top"`), never this row.
        assert_ne!(idle.rows[0].text, "     ");
    }

    #[test]
    fn compact_status_precedence_and_width_boundary_preserve_expanded_geometry() {
        let input = RailInput {
            session: crate::native_navigation::SessionRef("status".into()),
            title: "Status".into(),
            detail: "project".into(),
            flags: TabFlags { active: false, busy: true, needs_attention: true },
            marquee_offset: 0,
        };
        let compact = RailFrame::render_cells(std::slice::from_ref(&input), 11, 10, 0, None, false);
        assert_eq!(compact.rows[3].text.trim(), "!");
        assert_eq!(compact.rows[3].rect.height, 1);
        let expanded = RailFrame::render_cells(std::slice::from_ref(&input), 12, 10, 0, None, false);
        assert_eq!(expanded.rows[1].kind, RailRowKind::Session(input.session));
        assert_eq!(expanded.rows[2].kind, RailRowKind::Session(crate::native_navigation::SessionRef("status".into())));
        assert_eq!(expanded.rows[3].kind, RailRowKind::Gap);
    }

    #[test]
    fn compact_search_hover_paints_upstream_overlay_glyph() {
        // `session-tabs-rail.tsx:54-66`: while the compact search control is
        // hovered the row directly below the `⌕` glyph paints `▀` at the hover
        // tint (fg), with the tint as bg only when the row below is highlighted.
        let inputs = vec![RailInput {
            session: crate::native_navigation::SessionRef("s".into()),
            title: "S".into(),
            detail: "p".into(),
            flags: TabFlags::default(),
            marquee_offset: 0,
        }];
        let plain = RailFrame::render_cells_with_search_hover(&inputs, 5, 8, 0, None, false, false);
        let overlay = RailFrame::render_cells_with_search_hover(&inputs, 5, 8, 0, None, false, true);
        assert_eq!(plain.rows[2].kind, RailRowKind::Gap);
        assert_eq!(overlay.rows[2].kind, RailRowKind::SearchHover);
        assert_eq!(overlay.rows[2].text, "▀▀▀▀▀");
        assert_eq!(overlay.rows[2].color_spans, vec![(0, CellColor::SearchHover)]);
        // `belowHighlighted` (the row-below cap) is carried on `is_hovered`.
        let below = RailFrame::render_cells_with_search_hover(
            &inputs, 5, 8, 0, Some(&inputs[0].session), false, true,
        );
        assert!(below.rows[2].is_hovered, "highlighted row below raises the overlay bg");
        assert!(!overlay.rows[2].is_hovered);
        // The overlay never appears in the expanded rail.
        let expanded = RailFrame::render_cells_with_search_hover(&inputs, 12, 8, 0, None, false, true);
        assert!(!expanded.rows.iter().any(|row| row.kind == RailRowKind::SearchHover));
    }

    #[test]
    fn compact_hover_close_is_painted_and_exact_for_widths_five_through_eleven() {
        let input = sessions(1).remove(0);
        for width in 5..=11 {
            let frame = RailFrame::render_cells(
                std::slice::from_ref(&input),
                width,
                8,
                0,
                Some(&input.session),
                false,
            );
            let row = frame.rows.iter().find(|row| row.rect.y == 3).expect("compact title row");
            assert!(RailFrame::display_width(&row.text) <= usize::from(width));
            assert_eq!(row.text.chars().last(), Some(' '));
            assert_eq!(crate::native_view_layout::slice_cells(&row.text, width - 2, 1), "✕");
            assert!(row.color_spans.contains(&(width - 2, CellColor::Close)));
            assert_eq!(frame.close_hit(width - 2, 3), Some(&input.session));
            assert_eq!(frame.close_hit(width - 1, 3), None);
            assert_eq!(frame.close_hit(width - 2, 4), None);
            assert_eq!(frame.close_hit(width - 2, 1), None);
        }
    }

    #[test]
    fn tab_12_compact_tooltip_is_four_rows_capped_and_clamped() {
        // session-tabs.tsx:574-575 tooltipWidth; :1349-1387 placement.
        let tooltip = compact_tooltip(
            11,
            31,
            40,
            10,
            true,
            false,
            false,
            "A very long session title that will not fit",
            "project ⎇ branch",
        )
        .expect("hovered compact tooltip");
        assert_eq!(tooltip.rect.x, 11, "left = rail width");
        assert_eq!(tooltip.rect.width, 20, "cols - rail width");
        assert_eq!(tooltip.rect.height, 4, "4 rows including the two caps");
        assert_eq!(tooltip.rect.y, 9, "hover row - 1");
        assert!(RailFrame::display_width(&tooltip.title) <= 18);
        assert!(RailFrame::display_width(&tooltip.detail) <= 18);
        assert!(tooltip.title.len() < "A very long session title that will not fit".len());
        // A wide terminal caps the tooltip at 54 columns.
        assert_eq!(
            compact_tooltip(11, 120, 40, 10, true, false, false, "t", "d")
                .unwrap()
                .rect
                .width,
            COMPACT_TOOLTIP_MAX_WIDTH
        );

        // The tooltip narrows with the viewport and clamps to the bottom edge.
        let narrow = compact_tooltip(5, 40, 24, 30, true, false, false, "t", "d").unwrap();
        assert_eq!(narrow.rect.width, 35);
        assert_eq!(narrow.rect.y, 20, "rows - height");
        assert_eq!(
            compact_tooltip(5, 24, 24, 23, true, false, false, "t", "d")
                .unwrap()
                .rect
                .y,
            20
        );
        // A viewport narrower than the rail never invents a negative width.
        assert_eq!(
            compact_tooltip(30, 24, 24, 3, true, false, false, "t", "d"),
            None
        );
        // Hidden while dragging or while the context menu owns the pointer.
        for (hovered, dragging, menu) in [(false, false, false), (true, true, false), (true, false, true)] {
            assert_eq!(
                compact_tooltip(10, 80, 24, 4, hovered, dragging, menu, "t", "d"),
                None
            );
        }
    }

    #[test]
    fn tab_17_active_reveal_uses_new_layout_and_respects_viewport() {
        // session-tabs.tsx:658-667 reveals the active child after layout:
        // `top + height` must stay inside the scroll viewport for long lists.
        let mut inputs = sessions(30);
        for input in &mut inputs {
            input.flags.active = false;
        }
        inputs[27].flags.active = true;
        let frame = RailFrame::render_cells(&inputs, 42, 10, 0, None, false);
        let active = frame
            .rows
            .iter()
            .find(|row| matches!(&row.kind, RailRowKind::Session(id) if *id == inputs[27].session))
            .expect("active title row is painted");
        assert!(active.rect.y < frame.height, "active row inside the viewport");
        assert_eq!(active.rect.height, 1);
        // The reveal scrolls minimally: the active title and its detail row
        // end exactly at the viewport bottom.
        assert_eq!(active.rect.y + 2, frame.height);
        assert!(frame.scroll_cells > 0, "a long list scrolls to the active tab");
        // A taller viewport needs no scroll and paints the same row in place.
        let roomy = RailFrame::render_cells(&inputs, 42, 100, 0, None, false);
        assert_eq!(roomy.scroll_cells, 0);
        assert!(roomy
            .rows
            .iter()
            .any(|row| matches!(&row.kind, RailRowKind::Session(id) if *id == inputs[27].session)));
    }

    #[test]
    fn tab_10_expanded_uses_two_row_entries_with_gap_and_compact_uses_one() {
        // Expanded rows are title+detail with a one-row gap between entries
        // (`session-tabs.tsx:1462-1475`, rail `TAB_HEIGHT=2`, `TAB_GAP=1`);
        // compact entries are one row plus one gap (`session-tabs.tsx:1652-1675`).
        let inputs = sessions(2);
        let expanded = RailFrame::render(&inputs, RAIL_WIDTH, 12, 0, None);
        let title = &expanded.rows[1];
        let detail = &expanded.rows[2];
        let gap = &expanded.rows[3];
        assert_eq!(title.kind, RailRowKind::Session(inputs[0].session.clone()));
        assert_eq!(detail.kind, RailRowKind::Session(inputs[0].session.clone()));
        assert_eq!(gap.kind, RailRowKind::Gap);
        assert_eq!(detail.rect.y, title.rect.y + 1);
        assert_eq!(gap.rect.y, detail.rect.y + 1);
        assert_eq!((TAB_HEIGHT, TAB_GAP, TAB_STRIDE), (2, 1, 3));

        let compact = RailFrame::render_cells(&inputs, 5, 12, 0, None, false);
        assert_eq!(compact.rows[1].kind, RailRowKind::Search);
        assert_eq!(
            compact.rows[3].kind,
            RailRowKind::Session(inputs[0].session.clone())
        );
        assert_eq!(compact.rows[4].kind, RailRowKind::Gap);
        assert_eq!(
            compact.rows[5].kind,
            RailRowKind::Session(inputs[1].session.clone())
        );
        assert_eq!(compact.rows[3].rect.height, 1);
        assert_eq!(compact.rows[5].rect.y, compact.rows[3].rect.y + 2);
    }

    #[test]
    fn tab_10_detail_matches_project_branch_worktree_rules() {
        // session-tabs-model.ts:14-22: `branch = worktree && current !==
        // defaultBranch ? current : undefined`; result is
        // `branch && project ? "<project> \u{2387} <branch>" : branch ?? project`.
        assert_eq!(session_tab_detail("proj", Some("feat"), Some("main"), true), "proj \u{2387} feat");
        // current == default under a worktree drops the branch.
        assert_eq!(session_tab_detail("proj", Some("main"), Some("main"), true), "proj");
        // a plain checkout never shows a branch even when it differs.
        assert_eq!(session_tab_detail("proj", Some("feat"), Some("main"), false), "proj");
        // an empty project keeps the branch alone.
        assert_eq!(session_tab_detail("", Some("feat"), Some("main"), true), "feat");
        // no branch info falls back to the project.
        assert_eq!(session_tab_detail("proj", None, None, true), "proj");
        assert_eq!(session_tab_detail("proj", None, None, false), "proj");
    }

    #[test]
    fn tab_11_five_column_compact_golden_rows_are_pinned() {
        // Five-column compact golden (`session-tabs.tsx:1652-1675`): cap row 0,
        // search row 1, items 3/5/7, plus row 9, all in a 5-cell rail.
        let inputs = vec![
            RailInput {
                session: crate::native_navigation::SessionRef("first".into()),
                title: "First session".into(),
                detail: "project-alpha".into(),
                flags: TabFlags { active: true, busy: false, needs_attention: false },
                marquee_offset: 0,
            },
            RailInput {
                session: crate::native_navigation::SessionRef("second".into()),
                title: "Second session".into(),
                detail: "project-alpha".into(),
                flags: TabFlags::default(),
                marquee_offset: 0,
            },
            RailInput {
                session: crate::native_navigation::SessionRef("third".into()),
                title: "Third session".into(),
                detail: "project-alpha".into(),
                flags: TabFlags::default(),
                marquee_offset: 0,
            },
        ];
        let frame = RailFrame::render_cells(&inputs, 5, 24, 0, None, false);
        assert_eq!(frame.width, 5);
        assert_eq!(frame.rows[0].text, "▄▄▄▄▄");
        assert_eq!(frame.rows[1].text, "  ⌕  ");
        assert_eq!(frame.rows[3].text, "  F  ");
        assert_eq!(frame.rows[5].text, "  S  ");
        assert_eq!(frame.rows[7].text, "  T  ");
        assert_eq!(frame.rows[9].text, "  +  ");
        assert_eq!(frame.rows[1].rect.y, 1);
        for (row, session) in [(3, 0), (5, 1), (7, 2)] {
            assert_eq!(
                frame.rows[row].kind,
                RailRowKind::Session(inputs[session].session.clone())
            );
            assert_eq!(frame.rows[row].rect.width, 5);
        }
        assert_eq!(frame.rows[9].kind, RailRowKind::New);
        // The rail owns exactly five columns; the transcript starts at x=5.
        assert_eq!(frame.hit(4, 3), Some(RailHit::Session(inputs[0].session.clone())));
        assert_eq!(frame.hit(5, 3), None);
    }
}
