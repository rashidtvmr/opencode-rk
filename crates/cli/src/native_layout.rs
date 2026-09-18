#![forbid(unsafe_code)]
//! Native shell layout (TUI-003, slice: layout types).
//!
//! Pure geometry only: viewport size, compact flag, sidebar width,
//! stacked regions, overlap checker. No rendering, no IO, no clock.
//!
//! Commit: 5af7884. Evidence: `crates/cli/src/native_app.rs:120-244`
//! (Rect/ShellLayout/compute/has_overlap semantics); card TUI-003-T03
//! (tiny/resized terminals: no overlap, no panic, focus visible).

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
    let capped = if third < SIDEBAR_WIDTH { third } else { SIDEBAR_WIDTH };
    let floor = if SIDEBAR_MIN_WIDTH < width {
        SIDEBAR_MIN_WIDTH
    } else {
        width
    };
    let floored = if capped < floor { floor } else { capped };
    if floored > width { width } else { floored }
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
        let rs = [
            self.transcript,
            self.composer,
            self.sidebar,
            self.status,
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
            transcript: Rect { x: 0, y: 0, w: 50, h: 20 },
            composer: Rect { x: 0, y: 10, w: 50, h: 10 },
            sidebar: Rect { x: 0, y: 0, w: 0, h: 0 },
            status: Rect { x: 0, y: 39, w: 50, h: 1 },
            compact: false,
        };
        assert!(l.has_overlap());
    }
}
