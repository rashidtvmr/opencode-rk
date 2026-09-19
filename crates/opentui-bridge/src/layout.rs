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
}
