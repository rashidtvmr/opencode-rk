#![forbid(unsafe_code)]
//! Native shell layout (TUI-003, slice: layout types) and the pinned
//! session-frame region model (P03, LAY-01/LAY-14..LAY-16).
//!
//! Pure geometry only: viewport size, compact flag, sidebar width,
//! stacked regions, overlap checker. No rendering, no IO, no clock.
//!
//! Commit: 5af7884. Evidence: `crates/cli/src/native_app.rs:120-244`
//! (Rect/ShellLayout/compute/has_overlap semantics); card TUI-003-T03
//! (tiny/resized terminals: no overlap, no panic, focus visible).
//!
//! [`ShellLayout`] is the legacy fixed shell (commits 5af7884 / bf32742d) and
//! is kept unchanged for its existing consumers and tests. The pinned
//! session-frame geometry is [`SessionRegions`], derived from
//! `packages/tui/src/component/session-frame.tsx:107-140,261-398` and
//! `packages/tui/src/routes/session/index.tsx:1510-1592` at pin
//! `450e7b92412c780a7b5a75226bb8d49aaca24e22`: a growing session column plus
//! at most one right pane, an always-present 1-row action strip between the
//! transcript and the composer, a reserved bottom padding row, and a sidebar
//! whose title and footer are pinned outside its scroll region.

use crate::native_view_layout::{
    SESSION_SIDEBAR_PAD_HORIZONTAL, SESSION_SIDEBAR_PAD_VERTICAL, SESSION_SIDEBAR_SCROLL_RESERVE,
    SESSION_SIDEBAR_WIDTH, SESSION_TRANSCRIPT_SCROLLBAR_CELLS,
    session_horizontal_inset,
};

/// Minimum viewport for the full multi-column shell.
pub const MIN_FULL_WIDTH: u16 = 80;
/// Minimum viewport height for the full shell.
pub const MIN_FULL_HEIGHT: u16 = 24;
/// Preferred sidebar width on full viewports.
pub const SIDEBAR_WIDTH: u16 = 30;
/// Minimum sidebar width clamped into narrow full viewports.
pub const SIDEBAR_MIN_WIDTH: u16 = 20;

/// Viewport size in cells.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Size {
    pub w: u16,
    pub h: u16,
}

/// Integer viewport cell rectangle (origin top-left).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl Rect {
    #[must_use]
    pub const fn area(self) -> u32 {
        self.w as u32 * self.h as u32
    }

    /// True when interiors intersect (touching edges are fine).
    /// Zero-area rects never overlap.
    #[must_use]
    pub const fn overlaps(self, other: Rect) -> bool {
        if self.w == 0 || self.h == 0 || other.w == 0 || other.h == 0 {
            return false;
        }
        self.x < other.x.saturating_add(other.w)
            && other.x < self.x.saturating_add(self.w)
            && self.y < other.y.saturating_add(other.h)
            && other.y < self.y.saturating_add(self.h)
    }
}

/// Sidebar width for a full (non-compact) viewport of `width` cells.
/// Clamped to `[SIDEBAR_MIN_WIDTH, SIDEBAR_WIDTH]` and never above `width`.
#[must_use]
pub fn sidebar_width(width: u16) -> u16 {
    let third = width / 3;
    let capped = if third < SIDEBAR_WIDTH {
        third
    } else {
        SIDEBAR_WIDTH
    };
    let floor = if SIDEBAR_MIN_WIDTH < width {
        SIDEBAR_MIN_WIDTH
    } else {
        width
    };
    let floored = if capped < floor { floor } else { capped };
    if floored > width {
        width
    } else {
        floored
    }
}

/// Shell regions in paint order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShellLayout {
    pub transcript: Rect,
    pub composer: Rect,
    pub sidebar: Rect,
    pub status: Rect,
    /// True on tiny viewports (<80 or <24): single column, sidebar
    /// collapsed to zero so regions cannot overlap.
    pub compact: bool,
}

impl ShellLayout {
    /// Stacked non-overlapping layout. `sidebar_visible` is honored only
    /// when the viewport fits the full shell; tiny terminals force compact
    /// with a zero-area sidebar.
    #[must_use]
    pub fn compute(width: u16, height: u16, sidebar_visible: bool) -> ShellLayout {
        let compact = width < MIN_FULL_WIDTH || height < MIN_FULL_HEIGHT;
        let status_h: u16 = 1;
        let composer_h: u16 = if compact { 3 } else { 5 };
        let body_h = height.saturating_sub(status_h + composer_h);
        let status = Rect {
            x: 0,
            y: height.saturating_sub(status_h),
            w: width,
            h: status_h.min(height),
        };
        let composer = Rect {
            x: 0,
            y: height.saturating_sub(status_h + composer_h),
            w: width,
            h: composer_h.min(height.saturating_sub(status_h)),
        };
        if compact || !sidebar_visible {
            return ShellLayout {
                transcript: Rect {
                    x: 0,
                    y: 0,
                    w: width,
                    h: body_h,
                },
                composer,
                sidebar: Rect {
                    x: 0,
                    y: 0,
                    w: 0,
                    h: 0,
                },
                status,
                compact: true,
            };
        }
        let side_w = sidebar_width(width).min(width);
        ShellLayout {
            transcript: Rect {
                x: 0,
                y: 0,
                w: width.saturating_sub(side_w),
                h: body_h,
            },
            composer,
            sidebar: Rect {
                x: width.saturating_sub(side_w),
                y: 0,
                w: side_w,
                h: body_h,
            },
            status,
            compact: false,
        }
    }

    /// True when any two non-empty regions intersect.
    #[must_use]
    pub fn has_overlap(self) -> bool {
        let rs = [self.transcript, self.composer, self.sidebar, self.status];
        let mut i = 0;
        while i < rs.len() {
            if rs[i].area() != 0 {
                let mut j = i + 1;
                while j < rs.len() {
                    if rs[j].area() != 0 && rs[i].overlaps(rs[j]) {
                        return true;
                    }
                    j += 1;
                }
            }
            i += 1;
        }
        false
    }
}

/// Inputs for [`SessionRegions::compute`].
///
/// Every length is in cells/rows and is caller-owned: the tab reservation
/// comes from `native_tabs::TabReservation`, the right-pane precedence from
/// `native_view_layout::right_pane`, and the composer/heading heights from
/// the content owners (P06/P08/P07).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameSpec {
    /// Terminal width in cells (`useTerminalDimensions().width`).
    pub cols: u16,
    /// Terminal height in rows.
    pub rows: u16,
    /// Vertical tab rail width; 0 when tabs are horizontal or hidden
    /// (`component/session-frame.tsx:329`; the session route receives
    /// `verticalTabsWidth` from `app.tsx:622-623`).
    pub tabs_width: u16,
    /// Horizontal tab strip height; 0 when vertical or hidden.
    pub tabs_height: u16,
    /// The single resolved right pane (`session-frame.tsx:114-120`).
    pub right: crate::native_view_layout::RightPane,
    /// Dock width for a terminal/panel (`paneResize.size()`).
    pub pane_width: u16,
    /// Sidebar docks only when `wide`; otherwise it is an absolute overlay
    /// (`session-frame.tsx:325,385`).
    pub wide: bool,
    /// Rows owned by the composer block at the bottom of the session column
    /// (queued dock, prompt, border, hints). `flexShrink={0}`.
    pub composer_height: u16,
    /// Rendered title rows inside the sidebar (pinned, `sidebar.tsx:32`).
    pub sidebar_title_height: u16,
    /// Rendered footer rows inside the sidebar (pinned, `sidebar.tsx:74`).
    pub sidebar_footer_height: u16,
    /// `config.session.scrollbar` (`session/index.tsx:243`).
    pub scrollbar: bool,
}

impl FrameSpec {
    /// A spec with no tabs, no composer, one sidebar title row and no footer.
    #[must_use]
    pub const fn new(cols: u16, rows: u16) -> FrameSpec {
        FrameSpec {
            cols,
            rows,
            tabs_width: 0,
            tabs_height: 0,
            right: crate::native_view_layout::RightPane::None,
            pane_width: 0,
            wide: false,
            composer_height: 0,
            sidebar_title_height: 1,
            sidebar_footer_height: 0,
            scrollbar: false,
        }
    }
}

/// Rectangles of the pinned session frame.
///
/// All regions are disjoint and inside the terminal. `transcript` is the
/// scroll viewport (including the scrollbar column), `transcript_text` is the
/// narrower text area, `jump` is the always-reserved action row, `composer`
/// and `bottom_pad` close the session column, and the sidebar is split into a
/// pinned title, an independently scrolling middle and a pinned footer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionRegions {
    /// Area left after the tab reservation.
    pub available: Rect,
    /// Tab reservation box (rail left of the session column or strip above).
    pub tabs: Rect,
    /// The growing session column (`session-frame.tsx:272-286`).
    pub session: Rect,
    /// Transcript scroll viewport: the padded content column, including the
    /// scrollbar column (`index.tsx:1511-1520`).
    pub transcript: Rect,
    /// Transcript text columns (`transcript` minus the two scrollbar cells
    /// while the bar is shown).
    pub transcript_text: Rect,
    /// The one-column transcript scrollbar track on the right edge when
    /// shown, else zero-area (`index.tsx:1530-1537`).
    pub transcript_track: Rect,
    /// The always-present 1-row action strip (`index.tsx:1570`).
    pub jump: Rect,
    /// Composer block pinned at the bottom of the session column.
    pub composer: Rect,
    /// The reserved `paddingBottom={1}` row under the composer.
    pub bottom_pad: Rect,
    /// Docked right pane; zero-area when there is none or the sidebar overlays.
    pub right: Rect,
    /// Sidebar box: docked on the right when wide, absolute right overlay
    /// otherwise (`session-frame.tsx:325-397`).
    pub sidebar: Rect,
    /// Pinned sidebar title content box.
    pub sidebar_title: Rect,
    /// Sidebar scroll region (independently scrolled from the transcript).
    pub sidebar_scroll: Rect,
    /// Pinned sidebar footer box.
    pub sidebar_footer: Rect,
    /// The sidebar scrollbar column (absolute, overlays reserved padding).
    pub sidebar_track: Rect,
    /// True when the sidebar column is docked rather than overlaid.
    pub sidebar_docked: bool,
    /// True when the sidebar is painted as an absolute overlay.
    pub sidebar_overlay: bool,
    /// True when the transcript scrollbar reserves cells.
    pub scrollbar: bool,
}

impl SessionRegions {
    /// Compute the session-frame regions for `spec`.
    ///
    /// Follows the pinned upstream composition exactly:
    ///
    /// * tab reservation first (`app.tsx:622-623`, `session-frame.tsx:286`);
    /// * one right pane at a time, docked `SESSION_SIDEBAR_WIDTH` for the
    ///   sidebar and `paneResize.size()` for a terminal/panel
    ///   (`session-frame.tsx:329-331`);
    /// * a non-wide sidebar is absolutely positioned over the session column
    ///   (`session-frame.tsx:385-397`), so it reserves no docked width;
    /// * the session column insets left/right by 1 (<44 terminal) or 2
    ///   (`index.tsx:1515-1516`) and reserves one bottom padding row
    ///   (`index.tsx:1514`);
    /// * the action strip is always one row above the composer
    ///   (`index.tsx:1570`), the composer hangs at the bottom
    ///   (`index.tsx:1590`);
    /// * the sidebar pads 1/1/2/2, pins the title, scrolls the middle and
    ///   pins the footer (`sidebar.tsx:20-76`).
    ///
    /// Any viewport, including zero-area and absurd values, yields saturating
    /// in-range rectangles; regions may collapse but never overlap.
    #[must_use]
    pub fn compute(spec: &FrameSpec) -> SessionRegions {
        use crate::native_view_layout::{RightPane, right_pane_width};

        let cols = spec.cols;
        let rows = spec.rows;
        let tabs_w = spec.tabs_width.min(cols);
        let tabs_h = spec.tabs_height.min(rows);
        let available = Rect {
            x: tabs_w,
            y: tabs_h,
            w: cols - tabs_w,
            h: rows - tabs_h,
        };
        let tabs = if tabs_w > 0 {
            Rect { x: 0, y: 0, w: tabs_w, h: rows }
        } else if tabs_h > 0 {
            Rect { x: 0, y: 0, w: cols, h: tabs_h }
        } else {
            Rect { x: 0, y: 0, w: 0, h: 0 }
        };

        let sidebar_pane = spec.right == RightPane::Sidebar;
        let sidebar_docked = sidebar_pane && spec.wide;
        let sidebar_overlay = sidebar_pane && !spec.wide;
        let dock = if sidebar_docked {
            right_pane_width(RightPane::Sidebar, spec.pane_width).min(available.w)
        } else if matches!(spec.right, RightPane::Terminal | RightPane::Panel) {
            right_pane_width(spec.right, spec.pane_width).min(available.w)
        } else {
            0
        };

        let session = Rect {
            x: available.x,
            y: available.y,
            w: available.w - dock,
            h: available.h,
        };
        let right = Rect {
            x: session.x + session.w,
            y: available.y,
            w: dock,
            h: available.h,
        };

        // Horizontal inset uses the TERMINAL width, not the session width
        // (`index.tsx:1515-1516`). Saturate so tiny columns stay in range.
        let inset = session_horizontal_inset(cols).min(session.w / 2);
        let content_x = session.x + inset;
        let content_w = session.w - inset * 2;

        // Bottom padding row, then the composer above it, then the action row.
        let pad_rows = u16::from(session.h > 0).min(session.h);
        let bottom_pad = Rect {
            x: session.x,
            y: session.y + session.h - pad_rows,
            w: session.w,
            h: pad_rows,
        };
        let composer_room = session.h.saturating_sub(pad_rows);
        let composer_h = spec.composer_height.min(composer_room);
        let composer = Rect {
            x: session.x,
            y: session.y + composer_room - composer_h,
            w: session.w,
            h: composer_h,
        };
        let action_h = u16::from(composer.y > session.y).min(composer.y - session.y);
        let jump = Rect {
            x: session.x,
            y: composer.y - action_h,
            w: session.w,
            h: action_h,
        };
        // The transcript scrollbox fills the padded content column
        // (`index.tsx:1511-1520`); its insets are outside the scroll region.
        let transcript_h = jump.y - session.y;
        let transcript = Rect {
            x: content_x,
            y: session.y,
            w: content_w,
            h: transcript_h,
        };

        // Transcript scrollbar: viewport paddingRight(1) + 1-column track on
        // the right edge (`index.tsx:1527-1537`); hidden reserves nothing.
        let scrollbar = spec.scrollbar && transcript.h > 0 && content_w > SESSION_TRANSCRIPT_SCROLLBAR_CELLS;
        let reserve = if scrollbar { SESSION_TRANSCRIPT_SCROLLBAR_CELLS } else { 0 };
        let transcript_text = Rect {
            x: content_x,
            y: transcript.y,
            w: content_w - reserve,
            h: transcript.h,
        };
        let transcript_track = if scrollbar && transcript.h > 0 {
            Rect {
                x: content_x + content_w - 1,
                y: transcript.y,
                w: 1,
                h: transcript.h,
            }
        } else {
            Rect { x: content_x, y: transcript.y, w: 0, h: 0 }
        };

        // Sidebar: docked on the right edge, or an absolute right overlay.
        let sidebar_w = SESSION_SIDEBAR_WIDTH.min(available.w);
        let sidebar_x = available.x + available.w - sidebar_w;
        let sidebar = if sidebar_pane {
            Rect {
                x: sidebar_x,
                y: available.y,
                w: sidebar_w,
                h: available.h,
            }
        } else {
            Rect { x: sidebar_x, y: available.y, w: 0, h: 0 }
        };

        let inner_x = sidebar.x + SESSION_SIDEBAR_PAD_HORIZONTAL.min(sidebar.w);
        let inner_w = sidebar.w.saturating_sub(SESSION_SIDEBAR_PAD_HORIZONTAL * 2);
        let top = sidebar.y + SESSION_SIDEBAR_PAD_VERTICAL.min(sidebar.h);
        let bottom = sidebar.y + sidebar.h.saturating_sub(SESSION_SIDEBAR_PAD_VERTICAL);
        let title_h = spec.sidebar_title_height.min(bottom.saturating_sub(top));
        let title = Rect {
            x: inner_x,
            y: top,
            w: inner_w.min(sidebar.w.saturating_sub(SESSION_SIDEBAR_PAD_HORIZONTAL)),
            h: title_h,
        };
        // The title box reserves `paddingBottom={1}` (`sidebar.tsx:32`) and the
        // footer box reserves `paddingTop={1}` (`sidebar.tsx:74`); the footer
        // text is pinned to the bottom padding row of the 42-column box.
        let title_end = title.y + title.h;
        let title_gap = if title.h > 0 {
            1.min(bottom.saturating_sub(title_end))
        } else {
            0
        };
        let scroll_top = title_end + title_gap;
        let footer_text_h = spec.sidebar_footer_height.min(bottom.saturating_sub(scroll_top));
        let footer_y = bottom - footer_text_h;
        let footer_gap = if footer_text_h > 0 || spec.sidebar_footer_height > 0 {
            1.min(footer_y.saturating_sub(scroll_top))
        } else {
            0
        };
        let sidebar_footer = Rect {
            x: inner_x,
            y: footer_y,
            w: inner_w,
            h: footer_text_h,
        };
        let scroll_bottom = footer_y.saturating_sub(footer_gap);
        let sidebar_scroll = Rect {
            x: inner_x,
            y: scroll_top,
            w: inner_w,
            h: scroll_bottom.saturating_sub(scroll_top),
        };
        let sidebar_track = if sidebar_pane && sidebar_scroll.h > 0 && sidebar_scroll.w > 0 {
            Rect {
                x: inner_x + inner_w - 1,
                y: sidebar_scroll.y,
                w: 1,
                h: sidebar_scroll.h,
            }
        } else {
            Rect { x: inner_x, y: sidebar_scroll.y, w: 0, h: 0 }
        };

        SessionRegions {
            available,
            tabs,
            session,
            transcript,
            transcript_text,
            transcript_track,
            jump,
            composer,
            bottom_pad,
            right,
            sidebar,
            sidebar_title: title,
            sidebar_scroll,
            sidebar_footer,
            sidebar_track,
            sidebar_docked,
            sidebar_overlay,
            scrollbar,
        }
    }

    /// Sidebar text width inside the scroll region: the outer 42-column box
    /// minus 2+2 padding minus the constant 1-column scroll reserve
    /// (`sidebar.tsx:28-29,69`).
    #[must_use]
    pub const fn sidebar_text_width(&self) -> u16 {
        self.sidebar_scroll
            .w
            .saturating_sub(SESSION_SIDEBAR_SCROLL_RESERVE)
    }

    /// Sidebar title text width: the inner box minus its `paddingRight={2}`
    /// (`sidebar.tsx:32`).
    #[must_use]
    pub const fn sidebar_title_width(&self) -> u16 {
        self.sidebar_title.w.saturating_sub(2)
    }

    /// True when any two non-empty LEAF regions intersect. Container rects
    /// (`available`, `session`, `sidebar`, `right`) are excluded because they
    /// enclose their children by construction; an overlay sidebar is likewise
    /// painted on top of the session column.
    #[must_use]
    pub fn has_overlap(&self) -> bool {
        let rs = [
            self.tabs,
            self.transcript,
            self.jump,
            self.composer,
            self.bottom_pad,
            self.sidebar_title,
            self.sidebar_scroll,
            self.sidebar_footer,
        ];
        let mut i = 0;
        while i < rs.len() {
            if rs[i].area() != 0 {
                let mut j = i + 1;
                while j < rs.len() {
                    if rs[j].area() != 0 && rs[i].overlaps(rs[j]) {
                        return true;
                    }
                    j += 1;
                }
            }
            i += 1;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tiny_forces_compact_and_zero_sidebar() {
        let l = ShellLayout::compute(40, 10, true);
        assert!(l.compact, "tiny must set compact");
        assert_eq!(l.sidebar.w, 0);
        assert_eq!(l.sidebar.h, 0);
    }

    #[test]
    fn normal_terminal_has_no_overlap() {
        let l = ShellLayout::compute(120, 40, true);
        assert!(!l.compact);
        assert!(!l.has_overlap());
    }

    #[test]
    fn overlap_is_detected() {
        let l = ShellLayout {
            transcript: Rect {
                x: 0,
                y: 0,
                w: 50,
                h: 20,
            },
            composer: Rect {
                x: 0,
                y: 10,
                w: 50,
                h: 10,
            },
            sidebar: Rect {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
            },
            status: Rect {
                x: 0,
                y: 39,
                w: 50,
                h: 1,
            },
            compact: false,
        };
        assert!(l.has_overlap());
    }

    // -----------------------------------------------------------------------
    // LAY-01, LAY-11, LAY-14..LAY-16: pinned session-frame regions (P03).
    // -----------------------------------------------------------------------

    use crate::native_view_layout::{RightPane, SidebarMode, right_pane, sidebar_visible};

    fn docked(width: u16, height: u16, show_sidebar: bool, composer: u16) -> FrameSpec {
        let wide = crate::native_view_layout::session_wide(width, 0);
        let mode = if show_sidebar {
            SidebarMode::Auto
        } else {
            SidebarMode::Hide
        };
        let visible = sidebar_visible(false, false, mode, wide);
        let mut spec = FrameSpec::new(width, height);
        spec.wide = wide;
        spec.right = right_pane(false, false, visible, false);
        spec.composer_height = composer;
        spec.sidebar_title_height = 1;
        spec.sidebar_footer_height = 1;
        spec
    }

    /// LAY-01. The regions tile the terminal exactly at the four fixture sizes
    /// and never overlap: `available` + tab reservation = terminal,
    /// session + docked right pane = available, and transcript + action row +
    /// composer + bottom padding = session column (`session-frame.tsx:261-398`,
    /// `index.tsx:1510-1592`).
    #[test]
    fn lay_01_regions_tile_standard_viewports() {
        for (cols, rows) in [(80u16, 24u16), (120, 40), (160, 48), (200, 60)] {
            for sidebar in [false, true] {
                for composer in [0u16, 5] {
                    for scrollbar in [false, true] {
                        let mut spec = docked(cols, rows, sidebar, composer);
                        spec.scrollbar = scrollbar;
                        let r = SessionRegions::compute(&spec);
                        assert!(!r.has_overlap(), "{cols}x{rows}: regions must not overlap");
                        assert_eq!(r.available.x, 0);
                        assert_eq!(
                            r.session.w + r.right.w,
                            r.available.w,
                            "{cols}x{rows}: session + right pane fill the available width"
                        );
                        assert_eq!(
                            r.transcript.h + r.jump.h + r.composer.h + r.bottom_pad.h,
                            r.session.h,
                            "{cols}x{rows}: transcript/actions/composer/padding fill the column"
                        );
                        assert_eq!(r.jump.h, 1, "{cols}x{rows}: one action row is always reserved");
                        assert!(r.transcript.y + r.transcript.h <= rows);
                        assert!(r.composer.y + r.composer.h <= rows);
                        assert!(r.bottom_pad.y + r.bottom_pad.h <= rows);
                        assert_eq!(r.composer.y + r.composer.h + r.bottom_pad.h, rows);
                        if sidebar && r.sidebar.w > 0 {
                            assert!(r.sidebar_docked, "{cols}: sidebar docks when wide");
                            assert_eq!(r.right.w, 42);
                            assert_eq!(r.sidebar.x + r.sidebar.w, cols);
                            assert_eq!(
                                r.sidebar_title.y + r.sidebar_title.h
                                    + 1
                                    + r.sidebar_scroll.h
                                    + 1
                                    + r.sidebar_footer.h
                                    + 1,
                                r.sidebar.y + r.sidebar.h,
                                "{cols}: pad + title + gap + scroll + gap + footer + pad"
                            );
                        } else {
                            assert_eq!(r.right.w, 0);
                            assert_eq!(r.sidebar.w, 0);
                        }
                    }
                }
            }
        }
    }

    /// LAY-01 (degenerate viewports). Zero and tiny sizes saturate: no panic,
    /// every rect inside the viewport, no overlap.
    #[test]
    fn lay_01_degenerate_viewports_stay_in_range() {
        for (cols, rows) in [(0u16, 0u16), (0, 10), (10, 0), (1, 1), (4, 2), (20, 3), (43, 5)] {
            for sidebar in [false, true] {
                let r = SessionRegions::compute(&docked(cols, rows, sidebar, 3));
                assert!(!r.has_overlap(), "{cols}x{rows}: no overlap");
                for rect in [
                    r.available,
                    r.session,
                    r.transcript,
                    r.jump,
                    r.composer,
                    r.bottom_pad,
                    r.right,
                    r.sidebar,
                    r.sidebar_title,
                    r.sidebar_scroll,
                    r.sidebar_footer,
                ] {
                    assert!(
                        rect.x as u32 + rect.w as u32 <= cols as u32,
                        "{cols}x{rows}: rect {rect:?} inside width"
                    );
                    assert!(
                        rect.y as u32 + rect.h as u32 <= rows as u32,
                        "{cols}x{rows}: rect {rect:?} inside height"
                    );
                }
                assert_eq!(r.session.w + r.right.w, r.available.w);
            }
        }
    }

    /// LAY-11. A terminal/panel dock uses the resize width and never the
    /// sidebar width; the sidebar result is docked or overlaid, never both.
    #[test]
    fn lay_11_single_right_pane_geometry() {
        let mut spec = FrameSpec::new(160, 40);
        spec.right = RightPane::Sidebar;
        spec.wide = true;
        let r = SessionRegions::compute(&spec);
        assert!(r.sidebar_docked && !r.sidebar_overlay);
        assert_eq!(r.right.w, 42);

        spec.right = RightPane::Terminal;
        spec.pane_width = 57;
        let r = SessionRegions::compute(&spec);
        assert_eq!(r.right.w, 57, "terminal dock uses paneResize.size()");
        assert_eq!(r.session.w, 160 - 57);
        assert_eq!(r.sidebar.w, 0, "no sidebar box while a terminal is docked");

        spec.right = RightPane::Panel;
        spec.pane_width = 61;
        let r = SessionRegions::compute(&spec);
        assert_eq!(r.right.w, 61);

        // Narrow + explicit open: absolute overlay, no docked width.
        spec.right = RightPane::Sidebar;
        spec.wide = false;
        let r = SessionRegions::compute(&spec);
        assert!(r.sidebar_overlay && !r.sidebar_docked);
        assert_eq!(r.right.w, 0, "overlay does not consume a docked column");
        assert_eq!(r.session.w, 160);
        assert_eq!(r.sidebar.x, 118, "overlay hugs the right edge");
        assert_eq!(r.sidebar.w, 42);
        assert_eq!(r.sidebar.h, 40, "overlay spans the full height");
    }

    /// LAY-14. Transcript and sidebar scroll regions are disjoint from their
    /// pinned title/footer rows, so scrolling one never moves the other and
    /// never unpins a heading.
    #[test]
    fn lay_14_scroll_regions_are_disjoint_from_pinned_rows() {
        let mut spec = docked(160, 40, true, 5);
        spec.sidebar_title_height = 1;
        spec.sidebar_footer_height = 2;
        let r = SessionRegions::compute(&spec);
        assert!(!r.transcript.overlaps(r.sidebar_scroll));
        assert!(!r.sidebar_title.overlaps(r.sidebar_scroll));
        assert!(!r.sidebar_footer.overlaps(r.sidebar_scroll));
        assert!(!r.sidebar_title.overlaps(r.sidebar_footer));
        assert_eq!(r.sidebar_title.y, 1, "title pinned under the 1-row top pad");
        assert_eq!(r.sidebar_footer.y + r.sidebar_footer.h, 39, "footer pinned above the bottom pad");
        assert_eq!(r.transcript.y, 0, "transcript starts at the top of the column");
        assert_eq!(r.transcript.h, 40 - 1 - 5 - 1, "actions + composer + padding removed");
    }

    /// LAY-16. Scrollbar reservations: the transcript text area shrinks by the
    /// padding + track while the bar is shown, while every sidebar heading
    /// keeps the same origin and width (`index.tsx:1527-1537`,
    /// `sidebar.tsx:56-69`).
    #[test]
    fn lay_16_scrollbar_reservation_does_not_reflow_sidebar_headings() {
        let mut spec = docked(160, 40, true, 0);
        spec.scrollbar = false;
        let hidden = SessionRegions::compute(&spec);
        spec.scrollbar = true;
        let shown = SessionRegions::compute(&spec);
        assert!(!hidden.scrollbar && shown.scrollbar);
        assert_eq!(hidden.transcript_text.w - shown.transcript_text.w, 2);
        assert_eq!(hidden.transcript_text.x, shown.transcript_text.x, "left edge fixed");
        assert_eq!(shown.transcript_track.w, 1);
        assert_eq!(
            shown.transcript_track.x,
            shown.transcript_text.x + shown.transcript_text.w + 1,
            "one viewport padding column separates the text from the track"
        );
        // The sidebar is unaffected: absolute bar over reserved padding.
        assert_eq!(hidden.sidebar_title, shown.sidebar_title);
        assert_eq!(hidden.sidebar_scroll, shown.sidebar_scroll);
        assert_eq!(hidden.sidebar_footer, shown.sidebar_footer);
        assert_eq!(hidden.sidebar_text_width(), shown.sidebar_text_width());
        assert_eq!(shown.sidebar_text_width(), 37, "38 inner - 1 scroll reserve");
        assert_eq!(shown.sidebar_title_width(), 36, "38 inner - 2 title padding");
    }
}
