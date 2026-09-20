#![forbid(unsafe_code)]
//! Row/column constraint solver (pure, bounded, no FFI).

/// Max segments per split (fail-closed bound).
pub const MAX_SEGMENTS: usize = 64;
/// Max percent per segment / total.
pub const MAX_PERCENT: u32 = 100;

/// Area / cell rectangle. All units cells, origin top-left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    #[must_use]
    pub const fn new(x: u32, y: u32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    #[must_use]
    pub const fn area(self) -> u64 {
        self.w as u64 * self.h as u64
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.w == 0 || self.h == 0
    }

    /// True when interiors intersect (touching edges fine).
    /// Zero-area rects never overlap.
    #[must_use]
    pub const fn overlaps(self, other: Rect) -> bool {
        if self.is_empty() || other.is_empty() {
            return false;
        }
        self.x < other.x.saturating_add(other.w)
            && other.x < self.x.saturating_add(self.w)
            && self.y < other.y.saturating_add(other.h)
            && other.y < self.y.saturating_add(self.h)
    }
}

/// Minimum viewport for full multi-column shell (mirrors
/// `crates/cli/src/native_layout.rs:12-13,89`).
pub const MIN_FULL_WIDTH: u32 = 80;
/// Minimum viewport height for full shell.
pub const MIN_FULL_HEIGHT: u32 = 24;
/// Preferred sidebar width on full viewports.
pub const SIDEBAR_WIDTH: u32 = 30;
/// Minimum sidebar width clamped into narrow full viewports.
pub const SIDEBAR_MIN_WIDTH: u32 = 20;

/// Sidebar width for full (non-compact) viewport `width` cells.
/// Clamped to `[SIDEBAR_MIN_WIDTH, SIDEBAR_WIDTH]`, never above `width`.
/// Mirrors `crates/cli/src/native_layout.rs:59-69`.
#[must_use]
pub const fn sidebar_width(width: u32) -> u32 {
    let third = width / 3;
    let capped = if third < SIDEBAR_WIDTH { third } else { SIDEBAR_WIDTH };
    let floor = if SIDEBAR_MIN_WIDTH < width { SIDEBAR_MIN_WIDTH } else { width };
    let floored = if capped < floor { floor } else { capped };
    if floored > width { width } else { floored }
}

/// Shell regions in paint order (mirrors CLI `ShellLayout`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellRegions {
    pub transcript: Rect,
    pub composer: Rect,
    pub sidebar: Rect,
    pub status: Rect,
    /// True on tiny viewports: single column, sidebar collapsed to zero.
    pub compact: bool,
}

impl ShellRegions {
    /// Stacked non-overlapping layout. `sidebar_visible` honored only when
    /// viewport fits full shell; tiny terminals force compact.
    /// Mirrors `crates/cli/src/native_layout.rs:88-142`.
    #[must_use]
    pub const fn compute(width: u32, height: u32, sidebar_visible: bool) -> ShellRegions {
        let compact = width < MIN_FULL_WIDTH || height < MIN_FULL_HEIGHT;
        let status_h: u32 = 1;
        let composer_h: u32 = if compact { 3 } else { 5 };
        let body_h = height.saturating_sub(status_h + composer_h);
        let status = Rect {
            x: 0,
            y: height.saturating_sub(status_h),
            w: width,
            h: if status_h < height { status_h } else { height },
        };
        let sub = height.saturating_sub(status_h);
        let composer = Rect {
            x: 0,
            y: height.saturating_sub(status_h + composer_h),
            w: width,
            h: if composer_h < sub { composer_h } else { sub },
        };
        if compact || !sidebar_visible {
            return ShellRegions {
                transcript: Rect { x: 0, y: 0, w: width, h: body_h },
                composer,
                sidebar: Rect { x: 0, y: 0, w: 0, h: 0 },
                status,
                compact: true,
            };
        }
        let side_w = sidebar_width(width);
        let side_w = if side_w < width { side_w } else { width };
        ShellRegions {
            transcript: Rect { x: 0, y: 0, w: width.saturating_sub(side_w), h: body_h },
            composer,
            sidebar: Rect { x: width.saturating_sub(side_w), y: 0, w: side_w, h: body_h },
            status,
            compact: false,
        }
    }

    /// True when any two non-empty regions intersect.
    #[must_use]
    pub const fn has_overlap(self) -> bool {
        let rs = [self.transcript, self.composer, self.sidebar, self.status];
        let mut i = 0;
        while i < rs.len() {
            if !rs[i].is_empty() {
                let mut j = i + 1;
                while j < rs.len() {
                    if !rs[j].is_empty() && rs[i].overlaps(rs[j]) {
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

/// Width/height demand per segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Constraint {
    Fixed(u32),
    Flex(u32),
    Percent(u8),
}

/// Fail-closed solver errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutError {
    TooManySegments,
    PercentOverflow,
    ZeroFlexWeight,
}

impl core::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooManySegments => write!(f, "too many segments"),
            Self::PercentOverflow => write!(f, "percent sum exceeds 100"),
            Self::ZeroFlexWeight => write!(f, "flex present with zero total weight"),
        }
    }
}

impl std::error::Error for LayoutError {}

fn split_lengths(total: u32, constraints: &[Constraint]) -> Result<Vec<u32>, LayoutError> {
    if constraints.len() > MAX_SEGMENTS {
        return Err(LayoutError::TooManySegments);
    }
    let mut percent_sum: u32 = 0;
    let mut fixed_total: u64 = 0;
    let mut weight_total: u64 = 0;
    let mut has_flex = false;
    for c in constraints {
        match *c {
            Constraint::Fixed(v) => fixed_total = fixed_total.saturating_add(v as u64),
            Constraint::Percent(p) => percent_sum = percent_sum.saturating_add(p as u32),
            Constraint::Flex(w) => {
                has_flex = true;
                weight_total = weight_total.saturating_add(w as u64);
            }
        }
    }
    if percent_sum > MAX_PERCENT {
        return Err(LayoutError::PercentOverflow);
    }
    if has_flex && weight_total == 0 {
        return Err(LayoutError::ZeroFlexWeight);
    }
    let total_u64 = total as u64;
    let percent_total: u64 = constraints
        .iter()
        .filter_map(|c| match *c {
            Constraint::Percent(p) => Some(total_u64.saturating_mul(p as u64) / 100),
            _ => None,
        })
        .fold(0u64, |a, b| a.saturating_add(b));
    let used = fixed_total.saturating_add(percent_total);
    let remaining = total_u64.saturating_sub(used);
    // Base flex shares (floor division); leftover dealt below.
    let mut out: Vec<u32> = Vec::with_capacity(constraints.len());
    let mut flex_sum: u64 = 0;
    for c in constraints {
        match *c {
            Constraint::Fixed(v) => out.push(v),
            Constraint::Percent(p) => out.push((total_u64.saturating_mul(p as u64) / 100) as u32),
            Constraint::Flex(w) => {
                let share = remaining.saturating_mul(w as u64) / weight_total;
                flex_sum = flex_sum.saturating_add(share);
                out.push(share as u32);
            }
        }
    }
    // Deal integer-division leftover (+1 cell each) to leading flex segments.
    let mut leftover = remaining.saturating_sub(flex_sum);
    for (i, c) in constraints.iter().enumerate() {
        if leftover == 0 {
            break;
        }
        if matches!(c, Constraint::Flex(_)) {
            out[i] = out[i].saturating_add(1);
            leftover -= 1;
        }
    }
    // Saturate into bounds: clamp each width to space left.
    let mut cursor: u32 = 0;
    for w in out.iter_mut() {
        let cap = total.saturating_sub(cursor);
        *w = (*w).min(cap);
        cursor = cursor.saturating_add(*w);
    }
    Ok(out)
}

/// Split `area` horizontally; `y`/`h` inherited, `x`/`w` divided.
#[must_use = "solver is pure; use the returned rects"]
pub fn split_row(area: Rect, constraints: &[Constraint]) -> Result<Vec<Rect>, LayoutError> {
    let widths = split_lengths(area.w, constraints)?;
    let mut rects = Vec::with_capacity(widths.len());
    let mut x = area.x;
    for w in widths {
        rects.push(Rect::new(x, area.y, w, area.h));
        x = x.saturating_add(w);
    }
    Ok(rects)
}

/// Split `area` vertically; `x`/`w` inherited, `y`/`h` divided.
#[must_use = "solver is pure; use the returned rects"]
pub fn split_col(area: Rect, constraints: &[Constraint]) -> Result<Vec<Rect>, LayoutError> {
    let heights = split_lengths(area.h, constraints)?;
    let mut rects = Vec::with_capacity(heights.len());
    let mut y = area.y;
    for h in heights {
        rects.push(Rect::new(area.x, y, area.w, h));
        y = y.saturating_add(h);
    }
    Ok(rects)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_plus_flex_row() {
        let area = Rect::new(0, 0, 100, 10);
        let out = split_row(area, &[Constraint::Fixed(20), Constraint::Flex(1), Constraint::Flex(3)])
            .unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!((out[0].x, out[0].w), (0, 20));
        assert_eq!((out[1].x, out[1].w), (20, 20));
        assert_eq!((out[2].x, out[2].w), (40, 60));
        assert!(out.iter().all(|r| r.y == 0 && r.h == 10));
        assert_eq!(out.iter().map(|r| r.w).sum::<u32>(), 100);
    }

    #[test]
    fn percent_col_and_remainder_to_leading_flex() {
        let area = Rect::new(5, 2, 8, 10);
        let out = split_col(area, &[Constraint::Percent(50), Constraint::Flex(1), Constraint::Flex(1)])
            .unwrap();
        assert_eq!(out.iter().map(|r| r.h).sum::<u32>(), 10);
        assert_eq!(out[0].h, 5);
        // 5 cells left over 2 equal weights: 5/2=2 each +1 leftover to first.
        assert_eq!((out[1].h, out[2].h), (3, 2));
        assert_eq!((out[0].y, out[1].y, out[2].y), (2, 7, 10));
    }

    #[test]
    fn percent_overflow_errors() {
        let area = Rect::new(0, 0, 100, 10);
        assert_eq!(
            split_row(area, &[Constraint::Percent(60), Constraint::Percent(41)]),
            Err(LayoutError::PercentOverflow)
        );
        assert_eq!(
            split_row(area, &[Constraint::Percent(101)]),
            Err(LayoutError::PercentOverflow)
        );
    }

    #[test]
    fn zero_flex_weight_errors() {
        let area = Rect::new(0, 0, 50, 5);
        assert_eq!(
            split_row(area, &[Constraint::Flex(0)]),
            Err(LayoutError::ZeroFlexWeight)
        );
        assert_eq!(
            split_col(area, &[Constraint::Fixed(10), Constraint::Flex(0)]),
            Err(LayoutError::ZeroFlexWeight)
        );
    }

    #[test]
    fn zero_size_area_yields_zero_cells() {
        let area = Rect::new(3, 4, 0, 0);
        let row = split_row(area, &[Constraint::Fixed(5), Constraint::Flex(1)]).unwrap();
        assert!(row.iter().all(|r| r.w == 0));
        let col = split_col(area, &[Constraint::Percent(50), Constraint::Flex(1)]).unwrap();
        assert!(col.iter().all(|r| r.h == 0));
    }

    #[test]
    fn too_many_segments_and_overflow_saturate() {
        let area = Rect::new(0, 0, 10, 10);
        let many = vec![Constraint::Fixed(1); MAX_SEGMENTS + 1];
        assert_eq!(split_row(area, &many), Err(LayoutError::TooManySegments));
        // Fixed demands exceed area: clamped inside, never overflow.
        let out = split_row(area, &[Constraint::Fixed(u32::MAX), Constraint::Fixed(5)]).unwrap();
        assert_eq!(out[0].w, 10);
        assert_eq!(out[1].w, 0);
        // Zero constraints: empty split.
        assert!(split_row(area, &[]).unwrap().is_empty());
    }

    #[test]
    fn shell_tiny_forces_compact_zero_sidebar() {
        let l = ShellRegions::compute(40, 10, true);
        assert!(l.compact);
        assert!(l.sidebar.is_empty());
        assert!(!l.has_overlap());
    }

    #[test]
    fn shell_full_no_overlap_widths_stack() {
        let l = ShellRegions::compute(120, 40, true);
        assert!(!l.compact);
        assert_eq!(l.sidebar.w, 30);
        assert_eq!(l.transcript.w + l.sidebar.w, 120);
        assert_eq!(l.status.h, 1);
        assert_eq!(l.composer.h, 5);
        assert_eq!(l.transcript.h, 34);
        assert_eq!(l.status.y, 39);
        assert_eq!(l.composer.y, 34);
        assert!(!l.has_overlap());
    }

    #[test]
    fn shell_hidden_sidebar_collapses() {
        let l = ShellRegions::compute(120, 40, false);
        assert!(l.compact);
        assert!(l.sidebar.is_empty());
        assert_eq!(l.transcript.w, 120);
        assert!(!l.has_overlap());
    }

    #[test]
    fn shell_compact_thresholds() {
        assert!(!ShellRegions::compute(80, 24, true).compact);
        assert!(ShellRegions::compute(79, 24, true).compact);
        assert!(ShellRegions::compute(80, 23, true).compact);
        let tiny = ShellRegions::compute(100, 2, true);
        assert!(tiny.compact);
        assert!(!tiny.has_overlap());
    }

    #[test]
    fn shell_sidebar_width_clamp() {
        assert_eq!(sidebar_width(120), 30);
        assert_eq!(sidebar_width(80), 26);
        assert_eq!(sidebar_width(10), 10);
    }
}
