#![forbid(unsafe_code)]
//! Scroll box controller over a cell viewport (FFI-09).
//!
//! EVIDENCE / BOUNDARY
//! - The audited boundary (`docs/ui-parity/opencode-ui-parity-kit/
//!   audit-opentui-boundary.md` §3 "Diffs, scroll controllers and terminal
//!   scrollback", ref [S1] `packages/core/src/renderables/ScrollBox.ts:510-712`)
//!   states that `ScrollBoxRenderable` supplies *wrapper, viewport, content and
//!   scrollbars*, and that its TypeScript controller (not the native viewport
//!   clip/cull) owns: Shift-wheel axis swapping, wheel acceleration, retaining
//!   fractional deltas until whole cells can be scrolled, sticky/manual-scroll
//!   state, and time-based selection autoscroll.
//! - The vendored library exports **no** scroll-box entry point
//!   (`nm -gU …/libopentui.dylib` has no `scroll*` symbol), so no `extern "C"`
//!   is declared and this controller is a Rust model of that documented
//!   behaviour. Native viewport clipping is modelled by [`Scene`] clip
//!   rectangles, not here.
//! - Call sites show the shape the model must support: OpenCode drives a
//!   scroll box with `scrollAcceleration`, `verticalScrollBar`, `viewport` and
//!   `content` resize events
//!   (`packages/tui/src/ui/dialog-select.tsx:757-768`, pin 450e7b9).

use crate::buffer::{Cell, CellGrid};
use crate::color::RgbaU16;
use crate::layout::Rect;

/// Upper bound for a content/viewport dimension (fail-closed).
pub const MAX_SCROLL_EXTENT: u32 = 1_000_000;
/// Fractional wheel delta kept between events (cells).
pub const MAX_FRACTIONAL: f32 = 64.0;
/// Grow factor applied per consecutive wheel event (audit [S1] acceleration).
pub const ACCEL_STEP: f32 = 1.5;
/// Hard cap on the acceleration multiplier so a burst cannot jump the document.
pub const ACCEL_MAX: f32 = 6.0;
/// Drag-selection autoscroll speed in cells per second (audit [S1]
/// "time-based selection autoscroll").
pub const AUTOSCROLL_CELLS_PER_SECOND: f32 = 24.0;
/// Largest autoscroll step emitted for one frame.
pub const AUTOSCROLL_MAX_STEP: u32 = 8;

/// Scroll axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Vertical,
    Horizontal,
}

/// Wheel acceleration state. Starts neutral; each consecutive event multiplies
/// the applied cells, and [`ScrollBox::idle`] resets it (a new gesture).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Acceleration {
    pub step: f32,
    pub max: f32,
}

impl Default for Acceleration {
    fn default() -> Self {
        Self {
            step: ACCEL_STEP,
            max: ACCEL_MAX,
        }
    }
}

/// Scrollbar thumb in track cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thumb {
    /// Offset of the thumb from the track start.
    pub start: u32,
    /// Visible thumb length (always >= 1).
    pub len: u32,
}

/// Scroll box controller: wrapper/viewport/content geometry plus scroll state.
#[derive(Debug, Clone)]
pub struct ScrollBox {
    viewport: Rect,
    content_w: u32,
    content_h: u32,
    offset_x: u32,
    offset_y: u32,
    /// "Stick to bottom": when set, content growth keeps the tail visible.
    pub sticky: bool,
    /// Currently pinned to the bottom edge (cleared by manual upward scroll).
    stuck: bool,
    pub accel: Acceleration,
    accel_factor: f32,
    frac_x: f32,
    frac_y: f32,
}

impl ScrollBox {
    /// New box with an empty (0x0) content area, stuck to the bottom.
    #[must_use]
    pub fn new(viewport: Rect) -> Self {
        Self {
            viewport: Rect::new(
                viewport.x,
                viewport.y,
                viewport.w.min(MAX_SCROLL_EXTENT),
                viewport.h.min(MAX_SCROLL_EXTENT),
            ),
            content_w: 0,
            content_h: 0,
            offset_x: 0,
            offset_y: 0,
            sticky: true,
            stuck: true,
            accel: Acceleration::default(),
            accel_factor: 1.0,
            frac_x: 0.0,
            frac_y: 0.0,
        }
    }

    #[must_use]
    pub const fn viewport(&self) -> Rect {
        self.viewport
    }

    #[must_use]
    pub const fn content_size(&self) -> (u32, u32) {
        (self.content_w, self.content_h)
    }

    #[must_use]
    pub const fn offset(&self) -> (u32, u32) {
        (self.offset_x, self.offset_y)
    }

    /// True while the box follows the bottom edge.
    #[must_use]
    pub const fn is_stuck(&self) -> bool {
        self.stuck
    }

    #[must_use]
    pub const fn max_offset_y(&self) -> u32 {
        self.content_h.saturating_sub(self.viewport.h)
    }

    #[must_use]
    pub const fn max_offset_x(&self) -> u32 {
        self.content_w.saturating_sub(self.viewport.w)
    }

    /// Update the wrapper/viewport rectangle (native resize). A stuck box jumps
    /// to the new bottom; a detached box keeps its position, clamped.
    pub fn set_viewport(&mut self, viewport: Rect) {
        self.viewport = Rect::new(
            viewport.x,
            viewport.y,
            viewport.w.min(MAX_SCROLL_EXTENT),
            viewport.h.min(MAX_SCROLL_EXTENT),
        );
        self.reconcile();
    }

    /// Update the content size. Content **growth** while stuck follows the tail
    /// (no-op for a detached box); content **shrink** clamps both axes.
    pub fn set_content_size(&mut self, width: u32, height: u32) {
        self.content_w = width.min(MAX_SCROLL_EXTENT);
        self.content_h = height.min(MAX_SCROLL_EXTENT);
        self.reconcile();
    }

    fn reconcile(&mut self) {
        if self.sticky && self.stuck {
            self.offset_y = self.max_offset_y();
        }
        self.offset_y = self.offset_y.min(self.max_offset_y());
        self.offset_x = self.offset_x.min(self.max_offset_x());
        // Reaching the bottom re-attaches the sticky edge.
        if self.sticky && self.offset_y >= self.max_offset_y() {
            self.stuck = true;
        }
    }

    /// Absolute vertical offset. Scrolling to the bottom re-attaches the sticky
    /// edge; scrolling above it detaches (manual mode, audit [S1]).
    pub fn scroll_to_y(&mut self, y: u32) {
        let y = y.min(self.max_offset_y());
        self.offset_y = y;
        self.stuck = self.sticky && y >= self.max_offset_y();
    }

    /// Absolute horizontal offset.
    pub fn scroll_to_x(&mut self, x: u32) {
        self.offset_x = x.min(self.max_offset_x());
    }

    /// Relative scroll. Positive `dy` moves the content up (shows later rows).
    pub fn scroll_by(&mut self, dx: i32, dy: i32) {
        let x = apply_delta(self.offset_x, dx, self.max_offset_x());
        let y = apply_delta(self.offset_y, dy, self.max_offset_y());
        self.offset_x = x;
        self.scroll_to_y(y);
    }

    /// Wheel event. `dy`/`dx` are fractional cell deltas (positive = down /
    /// right). Shift swaps the axis, matching the audited controller. Fractional
    /// remainders are retained until they make a whole cell.
    ///
    /// Returns the whole-cell movement applied on each axis.
    pub fn wheel(&mut self, dy: f32, dx: f32, shift: bool) -> (i32, i32) {
        let (vertical, horizontal) = if shift { (dx, dy) } else { (dy, dx) };
        let factor = self.accel_factor * self.accel.step;
        let factor = if factor > self.accel.max {
            self.accel.max
        } else {
            factor
        };
        self.accel_factor = factor;
        let moved_y = self.apply_fractional(Axis::Vertical, vertical * factor);
        let moved_x = self.apply_fractional(Axis::Horizontal, horizontal * factor);
        (moved_x, moved_y)
    }

    fn apply_fractional(&mut self, axis: Axis, delta: f32) -> i32 {
        if !delta.is_finite() {
            return 0;
        }
        let (offset, max) = match axis {
            Axis::Vertical => (self.offset_y, self.max_offset_y()),
            Axis::Horizontal => (self.offset_x, self.max_offset_x()),
        };
        let frac = match axis {
            Axis::Vertical => &mut self.frac_y,
            Axis::Horizontal => &mut self.frac_x,
        };
        let total = (*frac + delta).clamp(-MAX_FRACTIONAL, MAX_FRACTIONAL);
        let whole = total.trunc();
        *frac = total - whole;
        let whole = whole as i32;
        let next = apply_delta(offset, whole, max);
        match axis {
            Axis::Vertical => self.scroll_to_y(next),
            Axis::Horizontal => self.offset_x = next,
        }
        next as i32 - offset as i32
    }

    /// Reset wheel acceleration and retained fractions (idle timeout / new
    /// gesture).
    pub fn idle(&mut self) {
        self.accel_factor = 1.0;
        self.frac_x = 0.0;
        self.frac_y = 0.0;
    }

    /// Page up (`-1`) / page down (`1`) by one viewport, minus one row of
    /// overlap so context is kept.
    pub fn page(&mut self, direction: i32) {
        let step = self.viewport.h.saturating_sub(1).max(1) as i32;
        self.scroll_by(0, direction * step);
    }

    /// Jump to the top (detaches) or the bottom (re-attaches when sticky).
    pub fn home(&mut self) {
        self.scroll_to_y(0);
    }

    pub fn end(&mut self) {
        self.scroll_to_y(self.max_offset_y());
    }

    /// First visible content row.
    #[must_use]
    pub const fn first_visible_row(&self) -> u32 {
        self.offset_y
    }

    /// Exclusive last visible content row.
    #[must_use]
    pub const fn last_visible_row(&self) -> u32 {
        let end = self.offset_y.saturating_add(self.viewport.h);
        if end > self.content_h {
            self.content_h
        } else {
            end
        }
    }

    /// Content row painted at absolute screen row `screen_y`, or `None` when the
    /// viewport starts below the content.
    #[must_use]
    pub fn content_row_at(&self, screen_y: u32) -> Option<u32> {
        if screen_y < self.viewport.y || screen_y >= self.viewport.y.saturating_add(self.viewport.h)
        {
            return None;
        }
        let row = self.offset_y.saturating_add(screen_y - self.viewport.y);
        if row < self.content_h {
            Some(row)
        } else {
            None
        }
    }

    /// Vertical scrollbar thumb for a track of `track_len` cells. `None` when
    /// the content fits (no scrollbar is shown).
    #[must_use]
    pub fn vertical_thumb(&self, track_len: u32) -> Option<Thumb> {
        thumb(
            track_len,
            self.content_h,
            self.viewport.h,
            self.offset_y,
            self.max_offset_y(),
        )
    }

    /// Horizontal scrollbar thumb.
    #[must_use]
    pub fn horizontal_thumb(&self, track_len: u32) -> Option<Thumb> {
        thumb(
            track_len,
            self.content_w,
            self.viewport.w,
            self.offset_x,
            self.max_offset_x(),
        )
    }

    /// Time-based autoscroll used by a drag selection held past an edge:
    /// `elapsed_ms` since the previous frame, `direction` -1 (up) / 1 (down).
    /// Returns the rows scrolled this frame (bounded).
    pub fn autoscroll(&mut self, elapsed_ms: u32, direction: i32) -> u32 {
        if direction == 0 {
            return 0;
        }
        let rows =
            (elapsed_ms as f32 / 1000.0 * AUTOSCROLL_CELLS_PER_SECOND).round() as u32;
        let rows = rows.min(AUTOSCROLL_MAX_STEP);
        if rows == 0 {
            return 0;
        }
        let before = self.offset_y;
        self.scroll_by(0, direction.min(1).max(-1) * rows as i32);
        self.offset_y.abs_diff(before)
    }

    /// Paint the vertical scrollbar thumb into `grid` at column `x`, rows
    /// `track` (clipped by the grid's scissor stack). Returns cells written.
    pub fn paint_vertical_scrollbar(
        &self,
        grid: &mut CellGrid,
        track: Rect,
        color: RgbaU16,
    ) -> u32 {
        let Some(thumb) = self.vertical_thumb(track.h) else {
            return 0;
        };
        let mut written = 0;
        for i in 0..thumb.len {
            let y = track.y.saturating_add(thumb.start).saturating_add(i);
            if grid.set(track.x, y, Cell::new('█', color, Cell::BLANK.bg)) {
                written += 1;
            }
        }
        written
    }
}

fn apply_delta(offset: u32, delta: i32, max: u32) -> u32 {
    if delta >= 0 {
        offset.saturating_add(delta as u32).min(max)
    } else {
        offset.saturating_sub(delta.unsigned_abs()).min(max)
    }
}

fn thumb(
    track_len: u32,
    content: u32,
    viewport: u32,
    offset: u32,
    max_offset: u32,
) -> Option<Thumb> {
    if track_len == 0 || content <= viewport || viewport == 0 {
        return None;
    }
    let len = (track_len.saturating_mul(viewport) / content).max(1).min(track_len);
    let travel = track_len.saturating_sub(len);
    let start = if max_offset == 0 {
        0
    } else {
        (travel as u64 * offset as u64 / max_offset as u64) as u32
    };
    Some(Thumb {
        start: start.min(travel),
        len,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewport(h: u32) -> Rect {
        Rect::new(0, 0, 20, h)
    }

    fn boxed(h: u32, content_h: u32) -> ScrollBox {
        let mut s = ScrollBox::new(viewport(h));
        s.set_content_size(20, content_h);
        s
    }

    #[test]
    fn ffi09_sticky_bottom_follows_growth_and_detaches_on_manual_up() {
        let mut s = boxed(5, 10);
        // Initially stuck at the bottom: offset = 5.
        assert!(s.is_stuck());
        assert_eq!(s.offset().1, 5);
        // Growth while stuck keeps the tail visible.
        s.set_content_size(20, 12);
        assert_eq!(s.offset().1, 7);
        // Manual upward scroll detaches.
        s.scroll_by(0, -3);
        assert_eq!(s.offset().1, 4);
        assert!(!s.is_stuck());
        // Growth no longer moves a detached box.
        s.set_content_size(20, 20);
        assert_eq!(s.offset().1, 4);
        // Scrolling back to the bottom re-attaches.
        s.end();
        assert_eq!(s.offset().1, 15);
        assert!(s.is_stuck());
    }

    #[test]
    fn ffi09_resize_clamps_and_resticks() {
        let mut s = boxed(5, 10);
        s.scroll_by(0, -4);
        assert_eq!(s.offset().1, 1);
        assert!(!s.is_stuck());
        // Viewport grows past the content: offset clamps to zero.
        s.set_viewport(viewport(20));
        assert_eq!(s.offset().1, 0);
        assert!(s.is_stuck(), "fits => bottom is the top edge");
        // Content shrink below the offset clamps too.
        let mut s = boxed(5, 10);
        s.set_content_size(20, 3);
        assert_eq!(s.offset().1, 0);
        assert_eq!(s.max_offset_y(), 0);
        assert_eq!(s.vertical_thumb(10), None, "no scrollbar when it fits");
    }

    #[test]
    fn ffi09_wheel_retains_fractional_delta() {
        let mut s = boxed(5, 20);
        // Neutral acceleration isolates the fractional accumulation rule.
        s.accel = Acceleration {
            step: 1.0,
            max: 1.0,
        };
        s.scroll_to_y(0);
        // 0.4 cells: nothing yet, remainder retained.
        assert_eq!(s.wheel(0.4, 0.0, false).1, 0);
        assert_eq!(s.offset().1, 0);
        // 0.4 + 0.4 = 0.8: still nothing.
        assert_eq!(s.wheel(0.4, 0.0, false).1, 0);
        // +0.4 = 1.2 -> one whole cell, 0.2 retained.
        assert_eq!(s.wheel(0.4, 0.0, false).1, 1);
        assert_eq!(s.offset().1, 1);
        // Negative deltas accumulate the same way (direction reversal).
        s.idle();
        assert_eq!(s.wheel(-0.6, 0.0, false).1, 0);
        assert_eq!(s.wheel(-0.6, 0.0, false).1, -1);
        assert_eq!(s.offset().1, 0);
        // Whole cells are not lost when the wheel hits an edge.
        assert_eq!(s.wheel(-5.0, 0.0, false).1, 0);
        assert_eq!(s.offset().1, 0);
    }

    #[test]
    fn ffi09_wheel_acceleration_is_capped() {
        let mut s = boxed(5, 1000);
        s.scroll_to_y(0);
        let first = s.wheel(1.0, 0.0, false).1;
        let second = s.wheel(1.0, 0.0, false).1;
        let third = s.wheel(1.0, 0.0, false).1;
        assert_eq!(first, 1, "first event is unaccelerated");
        assert!(second > first, "acceleration applies on a burst");
        assert!(third >= second);
        // A long burst saturates at ACCEL_MAX cells per event.
        for _ in 0..40 {
            let moved = s.wheel(1.0, 0.0, false).1;
            assert!(moved <= ACCEL_MAX as i32, "moved {moved} exceeds cap");
        }
        // Idle resets the factor.
        s.idle();
        assert_eq!(s.wheel(1.0, 0.0, false).1, 1);
    }

    #[test]
    fn ffi09_shift_wheel_swaps_axis() {
        let mut s = ScrollBox::new(Rect::new(0, 0, 10, 5));
        s.accel = Acceleration {
            step: 1.0,
            max: 1.0,
        };
        s.set_content_size(40, 40);
        s.scroll_to_y(0);
        s.scroll_to_x(0);
        s.idle();
        // Plain vertical wheel moves rows only.
        assert_eq!(s.wheel(2.0, 0.0, false), (0, 2));
        assert_eq!(s.offset(), (0, 2));
        s.idle();
        // Shift swaps: the vertical delta scrolls columns.
        assert_eq!(s.wheel(3.0, 0.0, true), (3, 0));
        assert_eq!(s.offset(), (3, 2));
    }

    #[test]
    fn ffi09_scrollbar_thumb_geometry() {
        let mut s = boxed(5, 20);
        s.scroll_to_y(0);
        // Track 10, viewport 5, content 20 -> 2-cell thumb at the top.
        assert_eq!(s.vertical_thumb(10), Some(Thumb { start: 0, len: 2 }));
        s.scroll_to_y(7);
        assert_eq!(s.vertical_thumb(10), Some(Thumb { start: 3, len: 2 }));
        s.end();
        assert_eq!(s.vertical_thumb(10), Some(Thumb { start: 8, len: 2 }));
        // Tiny content still yields a 1-cell thumb, never zero.
        let s2 = boxed(10, 11);
        assert_eq!(s2.vertical_thumb(10).map(|t| t.len), Some(9));
        assert_eq!(s2.vertical_thumb(0), None);
    }

    #[test]
    fn ffi09_scrollbar_paints_into_cells() {
        let mut s = boxed(5, 20);
        s.scroll_to_y(7);
        let mut grid = CellGrid::new(10, 10);
        let written = s.paint_vertical_scrollbar(
            &mut grid,
            Rect::new(0, 0, 1, 10),
            crate::color::rgb(200, 200, 200),
        );
        assert_eq!(written, 2);
        assert_eq!(grid.get(0, 2).unwrap().ch, ' ');
        assert_eq!(grid.get(0, 3).unwrap().ch, '\u{2588}');
        assert_eq!(grid.get(0, 4).unwrap().ch, '\u{2588}');
        assert_eq!(grid.get(0, 5).unwrap().ch, ' ');
    }

    #[test]
    fn ffi09_page_home_end_and_visible_range() {
        let mut s = boxed(5, 20);
        s.end();
        assert_eq!(s.first_visible_row(), 15);
        assert_eq!(s.last_visible_row(), 20);
        s.page(-1);
        assert_eq!(s.offset().1, 11, "page up by height-1");
        assert!(!s.is_stuck());
        s.home();
        assert_eq!((s.first_visible_row(), s.last_visible_row()), (0, 5));
        s.page(1);
        assert_eq!(s.offset().1, 4);
        assert_eq!(s.content_row_at(0), Some(4));
        assert_eq!(s.content_row_at(4), Some(8));
        assert_eq!(s.content_row_at(5), None, "outside the viewport");
    }

    #[test]
    fn ffi09_autoscroll_is_time_based_and_bounded() {
        let mut s = boxed(5, 100);
        s.scroll_to_y(50);
        // 10 ms at 24 cells/s rounds to 0 rows.
        assert_eq!(s.autoscroll(10, 1), 0);
        // 500 ms ~ 12 rows, capped at AUTOSCROLL_MAX_STEP.
        assert_eq!(s.autoscroll(500, 1), AUTOSCROLL_MAX_STEP);
        assert_eq!(s.offset().1, 50 + AUTOSCROLL_MAX_STEP);
        // Upward drag scrolls back; no direction is a no-op.
        assert_eq!(s.autoscroll(100, -1), 2);
        assert_eq!(s.autoscroll(100, 0), 0);
        // Edge: already at the top, upward autoscroll reports 0.
        s.home();
        assert_eq!(s.autoscroll(1000, -1), 0);
    }

    #[test]
    fn ffi09_wheel_ignores_non_finite_deltas() {
        let mut s = boxed(5, 20);
        s.scroll_to_y(0);
        assert_eq!(s.wheel(f32::NAN, 0.0, false).1, 0);
        assert_eq!(s.wheel(f32::INFINITY, 0.0, false).1, 0);
        assert_eq!(s.offset().1, 0);
    }
}
