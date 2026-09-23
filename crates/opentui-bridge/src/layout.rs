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


// --- Flexbox subset (yoga web-default semantics, golden-covered cases) ---

/// Flex main axis. Mirrors `yoga.ts:FlexDirection` (Column default per Renderable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirection {
    #[default]
    Column,
    ColumnReverse,
    Row,
    RowReverse,
}

impl FlexDirection {
    #[must_use]
    pub const fn is_row(self) -> bool {
        matches!(self, Self::Row | Self::RowReverse)
    }
    #[must_use]
    pub const fn is_reverse(self) -> bool {
        matches!(self, Self::ColumnReverse | Self::RowReverse)
    }
}

/// Style dimension. Mirrors yoga `Value`/`Unit` (`yoga.ts:parseValue`):
/// number=Point, "N%"=Percent, "auto"=Auto, unset=Undefined.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Dim {
    #[default]
    Undefined,
    Auto,
    Point(f32),
    Percent(f32),
}

impl Dim {
    /// Resolve against `base`; Auto/Undefined -> None (intrinsic/flex fallback).
    #[must_use]
    pub fn resolve(self, base: f32) -> Option<f32> {
        match self {
            Self::Point(v) if v.is_finite() && v >= 0.0 => Some(v),
            Self::Percent(p) if p.is_finite() && p >= 0.0 => Some(base * p / 100.0),
            _ => None,
        }
    }
}

/// Box edge selector (subset of yoga `Edge`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Left,
    Top,
    Right,
    Bottom,
    Horizontal,
    Vertical,
    All,
}

/// Computed layout rect (floats; cells as f32). Mirrors yoga `Layout`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayoutRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

type MeasureFn = Box<dyn Fn(f32, f32) -> (f32, f32)>;
type DirtiedFn = Box<dyn FnMut()>;

/// Flex node: style + children + computed layout.
/// Web defaults: Column, grow 0, shrink 1, basis auto (`Renderable.ts` forces
/// shrink 0 only when explicit numeric w/h set — caller's choice via setter).
pub struct LayoutNode {
    direction: FlexDirection,
    flex_grow: f32,
    flex_shrink: f32,
    flex_basis: Dim,
    width: Dim,
    height: Dim,
    min_width: Dim,
    min_height: Dim,
    max_width: Dim,
    max_height: Dim,
    margin: [Dim; 4],  // L,T,R,B
    padding: [Dim; 4], // L,T,R,B
    children: Vec<LayoutNode>,
    measure: Option<MeasureFn>,
    dirtied: Option<DirtiedFn>,
    dirty: bool,
    computed: LayoutRect,
}

impl Default for LayoutNode {
    fn default() -> Self {
        Self {
            direction: FlexDirection::Column,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Dim::Auto,
            width: Dim::Auto,
            height: Dim::Auto,
            min_width: Dim::Undefined,
            min_height: Dim::Undefined,
            max_width: Dim::Undefined,
            max_height: Dim::Undefined,
            margin: [Dim::Undefined; 4],
            padding: [Dim::Undefined; 4],
            children: Vec::new(),
            measure: None,
            dirtied: None,
            dirty: true,
            computed: LayoutRect::default(),
        }
    }
}

fn edge_slots(edge: Edge) -> &'static [usize] {
    match edge {
        Edge::Left => &[0],
        Edge::Top => &[1],
        Edge::Right => &[2],
        Edge::Bottom => &[3],
        Edge::Horizontal => &[0, 2],
        Edge::Vertical => &[1, 3],
        Edge::All => &[0, 1, 2, 3],
    }
}

impl LayoutNode {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn touch(&mut self) {
        self.dirty = true;
        if let Some(f) = self.dirtied.as_mut() {
            f();
        }
    }

    pub fn set_direction(&mut self, d: FlexDirection) {
        self.direction = d;
        self.touch();
    }
    pub fn set_flex_grow(&mut self, v: f32) {
        self.flex_grow = if v.is_finite() && v >= 0.0 { v } else { 0.0 };
        self.touch();
    }
    pub fn set_flex_shrink(&mut self, v: f32) {
        self.flex_shrink = if v.is_finite() && v >= 0.0 { v } else { 1.0 };
        self.touch();
    }
    pub fn set_flex_basis(&mut self, v: Dim) {
        self.flex_basis = v;
        self.touch();
    }
    pub fn set_width(&mut self, v: Dim) {
        self.width = v;
        self.touch();
    }
    pub fn set_height(&mut self, v: Dim) {
        self.height = v;
        self.touch();
    }
    pub fn set_min_width(&mut self, v: Dim) {
        self.min_width = v;
        self.touch();
    }
    pub fn set_min_height(&mut self, v: Dim) {
        self.min_height = v;
        self.touch();
    }
    pub fn set_max_width(&mut self, v: Dim) {
        self.max_width = v;
        self.touch();
    }
    pub fn set_max_height(&mut self, v: Dim) {
        self.max_height = v;
        self.touch();
    }
    pub fn set_margin(&mut self, edge: Edge, v: Dim) {
        for &s in edge_slots(edge) {
            self.margin[s] = v;
        }
        self.touch();
    }
    pub fn set_padding(&mut self, edge: Edge, v: Dim) {
        for &s in edge_slots(edge) {
            self.padding[s] = v;
        }
        self.touch();
    }

    pub fn insert_child(&mut self, child: LayoutNode, index: usize) {
        let i = index.min(self.children.len());
        self.children.insert(i, child);
        self.touch();
    }
    pub fn remove_child(&mut self, index: usize) -> Option<LayoutNode> {
        if index < self.children.len() {
            self.touch();
            Some(self.children.remove(index))
        } else {
            None
        }
    }
    #[must_use]
    pub fn child(&self, index: usize) -> Option<&LayoutNode> {
        self.children.get(index)
    }
    fn child_mut(&mut self, index: usize) -> Option<&mut LayoutNode> {
        self.children.get_mut(index)
    }
    #[must_use]
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Set leaf measure func (intrinsic size from available w/h).
    /// Single slot like yoga (`yoga.ts:setMeasureFunc`); marks dirty + dirtied.
    pub fn set_measure_func<F>(&mut self, f: F)
    where
        F: Fn(f32, f32) -> (f32, f32) + 'static,
    {
        self.measure = Some(Box::new(f));
        self.touch();
    }
    pub fn unset_measure_func(&mut self) {
        self.measure = None;
        self.touch();
    }
    #[must_use]
    pub fn has_measure_func(&self) -> bool {
        self.measure.is_some()
    }

    /// Dirtied callback fired on any style/measure/dirty change.
    pub fn set_dirtied_func<F>(&mut self, f: F)
    where
        F: FnMut() + 'static,
    {
        self.dirtied = Some(Box::new(f));
    }
    pub fn unset_dirtied_func(&mut self) {
        self.dirtied = None;
    }

    pub fn mark_dirty(&mut self) {
        self.touch();
    }
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        if self.dirty {
            return true;
        }
        self.children.iter().any(LayoutNode::is_dirty)
    }
    #[must_use]
    pub fn computed(&self) -> LayoutRect {
        self.computed
    }

    #[allow(clippy::too_many_lines)]
    fn layout_into(
        &mut self,
        ox: f32,
        oy: f32,
        avail_w: f32,
        avail_h: f32,
        imposed_w: Option<f32>,
        imposed_h: Option<f32>,
    ) {
        // Own size per axis: parent-imposed flex length wins (it already
        // folded style-as-basis + grow/shrink); otherwise style percent/point
        // (resolved once, against parent-provided available) > measure > fill.
        let mut w = imposed_w.or_else(|| self.width.resolve(avail_w)).unwrap_or_else(|| {
            self.measure
                .as_ref()
                .map(|m| m(avail_w, avail_h).0.max(0.0))
                .unwrap_or(avail_w)
        });
        let mut h = imposed_h.or_else(|| self.height.resolve(avail_h)).unwrap_or_else(|| {
            self.measure
                .as_ref()
                .map(|m| m(avail_w, avail_h).1.max(0.0))
                .unwrap_or(avail_h)
        });
        // min/max clamps (percent resolved against available).
        if let Some(lo) = self.min_width.resolve(avail_w) {
            w = w.max(lo);
        }
        if let Some(hi) = self.max_width.resolve(avail_w) {
            w = w.min(hi);
        }
        if let Some(lo) = self.min_height.resolve(avail_h) {
            h = h.max(lo);
        }
        if let Some(hi) = self.max_height.resolve(avail_h) {
            h = h.min(hi);
        }
        self.computed = LayoutRect { x: ox, y: oy, width: w, height: h };
        let row = self.direction.is_row();

        if self.children.is_empty() {
            self.dirty = false;
            return;
        }

        let (pad_l, pad_t, pad_r, pad_b) = (
            self.padding[0].resolve(w).unwrap_or(0.0),
            self.padding[1].resolve(h).unwrap_or(0.0),
            self.padding[2].resolve(w).unwrap_or(0.0),
            self.padding[3].resolve(h).unwrap_or(0.0),
        );
        let inner_x = ox + pad_l;
        let inner_y = oy + pad_t;
        let inner_w = (w - pad_l - pad_r).max(0.0);
        let inner_h = (h - pad_t - pad_b).max(0.0);
        let inner_main = if row { inner_w } else { inner_h };

        // Basis per child: basis pt > explicit main dim > measure > 0 (percent vs inner).
        let n = self.children.len();
        let mut basis = vec![0.0f32; n];
        let mut marg: Vec<(f32, f32)> = vec![(0.0, 0.0); n]; // (main_before, main_after)
        for (i, c) in self.children.iter().enumerate() {
            let (mb, ma) = if row {
                (
                    c.margin[0].resolve(inner_w).unwrap_or(0.0),
                    c.margin[2].resolve(inner_w).unwrap_or(0.0),
                )
            } else {
                (
                    c.margin[1].resolve(inner_h).unwrap_or(0.0),
                    c.margin[3].resolve(inner_h).unwrap_or(0.0),
                )
            };
            marg[i] = (mb, ma);
            let explicit = if row {
                c.width.resolve(inner_w)
            } else {
                c.height.resolve(inner_h)
            };
            basis[i] = c
                .flex_basis
                .resolve(inner_main)
                .or(explicit)
                .unwrap_or_else(|| {
                    c.measure
                        .as_ref()
                        .map(|m| {
                            let (mw, mh) = m(inner_w, inner_h);
                            (if row { mw } else { mh }).max(0.0)
                        })
                        .unwrap_or(0.0)
                });
        }

        let used: f32 = basis.iter().sum::<f32>() + marg.iter().map(|(a, b)| a + b).sum::<f32>();
        let mut main = basis;
        let free = inner_main - used;
        if free > 0.0 {
            // Grow with max-clamp redistribution: clamped surplus returns to
            // unfrozen growers (yoga web-default behavior).
            // Grow with max-clamp redistribution: each pass snapshots the
            // distributable amount; clamped surplus loops back to unfrozen
            // growers (yoga web-default behavior).
            let mut frozen = vec![false; n];
            let mut remaining = free;
            loop {
                let g: f32 = self
                    .children
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !frozen[*i])
                    .map(|(_, c)| c.flex_grow)
                    .sum();
                if g <= 0.0 || remaining <= 0.0 {
                    break;
                }
                let round = remaining;
                remaining = 0.0;
                for i in 0..n {
                    if frozen[i] {
                        continue;
                    }
                    main[i] += round * self.children[i].flex_grow / g;
                    let mx = if row {
                        self.children[i].max_width.resolve(inner_w)
                    } else {
                        self.children[i].max_height.resolve(inner_h)
                    };
                    if let Some(hi) = mx {
                        if main[i] > hi {
                            remaining += main[i] - hi;
                            main[i] = hi;
                            frozen[i] = true;
                        }
                    }
                }
            }
        } else if free < 0.0 {
            let s: f32 = self
                .children
                .iter()
                .enumerate()
                .map(|(i, c)| c.flex_shrink * main[i])
                .sum();
            if s > 0.0 {
                for (i, c) in self.children.iter().enumerate() {
                    main[i] = (main[i] + free * c.flex_shrink * main[i] / s).max(0.0);
                }
            }
        }

        // Place children; each resolves its own cross percent against inner.
        let mut cursor = 0.0f32;
        for i in 0..n {
            let (px, py) = if row {
                (inner_x + cursor + marg[i].0, inner_y)
            } else {
                (inner_x, inner_y + cursor + marg[i].0)
            };
            let c = &self.children[i];
            let (mn, mx) = if row {
                (c.min_width.resolve(inner_w), c.max_width.resolve(inner_w))
            } else {
                (c.min_height.resolve(inner_h), c.max_height.resolve(inner_h))
            };
            let mut mlen = main[i];
            if let Some(lo) = mn {
                mlen = mlen.max(lo);
            }
            if let Some(hi) = mx {
                mlen = mlen.min(hi);
            }
            if let Some(child) = self.child_mut(i) {
                let (iw, ih) = if row { (Some(mlen), None) } else { (None, Some(mlen)) };
                child.layout_into(px, py, inner_w, inner_h, iw, ih);
            }
            cursor += mlen + marg[i].0 + marg[i].1;
        }
        // Reverse directions: mirror positions about inner far edge.
        if self.direction.is_reverse() {
            let total = cursor;
            for i in 0..n {
                if let Some(child) = self.child_mut(i) {
                    let r = child.computed;
                    if row {
                        child.computed.x = inner_x + total - (r.x - inner_x) - r.width;
                    } else {
                        child.computed.y = inner_y + total - (r.y - inner_y) - r.height;
                    }
                }
            }
        }
        self.dirty = false;
    }
}

/// Compute layout for `root` (and subtree). `width`/`height`: explicit
/// viewport (None = auto/0, mirrors yoga NaN). Marks clean when done.
pub fn calculate_layout(root: &mut LayoutNode, width: Option<f32>, height: Option<f32>) {
    let w = width.filter(|v| v.is_finite() && *v >= 0.0).unwrap_or(0.0);
    let h = height.filter(|v| v.is_finite() && *v >= 0.0).unwrap_or(0.0);
    root.layout_into(0.0, 0.0, w, h, None, None);
}

#[cfg(test)]
mod flex_tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn flex_column_stacks_children() {
        let mut root = LayoutNode::new();
        root.set_width(Dim::Point(10.0));
        root.set_height(Dim::Point(30.0));
        let mut a = LayoutNode::new();
        a.set_height(Dim::Point(10.0));
        let mut b = LayoutNode::new();
        b.set_height(Dim::Point(20.0));
        root.insert_child(a, 0);
        root.insert_child(b, 1);
        calculate_layout(&mut root, None, None);
        let la = root.child(0).unwrap().computed();
        let lb = root.child(1).unwrap().computed();
        assert_eq!((la.y, la.height), (0.0, 10.0));
        assert_eq!((lb.y, lb.height), (10.0, 20.0));
    }

    #[test]
    fn flex_row_grow_splits_free_space() {
        let mut root = LayoutNode::new();
        root.set_direction(FlexDirection::Row);
        root.set_width(Dim::Point(100.0));
        root.set_height(Dim::Point(10.0));
        let mut a = LayoutNode::new();
        a.set_width(Dim::Point(20.0));
        let mut b = LayoutNode::new();
        b.set_flex_grow(1.0);
        let mut c = LayoutNode::new();
        c.set_flex_grow(3.0);
        root.insert_child(a, 0);
        root.insert_child(b, 1);
        root.insert_child(c, 2);
        calculate_layout(&mut root, None, None);
        assert_eq!(root.child(0).unwrap().computed().width, 20.0);
        assert_eq!(root.child(1).unwrap().computed().width, 20.0);
        assert_eq!(root.child(2).unwrap().computed().width, 60.0);
    }

    #[test]
    fn flex_shrink_on_overflow() {
        let mut root = LayoutNode::new();
        root.set_direction(FlexDirection::Row);
        root.set_width(Dim::Point(50.0));
        root.set_height(Dim::Point(10.0));
        let mut a = LayoutNode::new();
        a.set_flex_basis(Dim::Point(30.0));
        let mut b = LayoutNode::new();
        b.set_flex_basis(Dim::Point(30.0));
        root.insert_child(a, 0);
        root.insert_child(b, 1);
        calculate_layout(&mut root, None, None);
        assert_eq!(root.child(0).unwrap().computed().width, 25.0);
        assert_eq!(root.child(1).unwrap().computed().width, 25.0);
    }

    #[test]
    fn flex_padding_insets_children() {
        let mut root = LayoutNode::new();
        root.set_width(Dim::Point(20.0));
        root.set_height(Dim::Point(20.0));
        root.set_padding(Edge::All, Dim::Point(2.0));
        let mut a = LayoutNode::new();
        a.set_height(Dim::Point(5.0));
        root.insert_child(a, 0);
        calculate_layout(&mut root, None, None);
        let l = root.child(0).unwrap().computed();
        assert_eq!((l.x, l.y), (2.0, 2.0));
        assert_eq!(l.width, 16.0);
    }

    #[test]
    fn flex_measure_leaf_and_dirtied_recompute() {
        let mut leaf = LayoutNode::new();
        leaf.set_measure_func(|_w, _h| (7.0, 3.0));
        let fired = Rc::new(Cell::new(false));
        let f2 = fired.clone();
        leaf.set_dirtied_func(move || f2.set(true));
        calculate_layout(&mut leaf, None, None);
        assert_eq!((leaf.computed().width, leaf.computed().height), (7.0, 3.0));
        assert!(leaf.has_measure_func());
        leaf.mark_dirty();
        assert!(fired.get());
        assert!(leaf.is_dirty());
        calculate_layout(&mut leaf, None, None);
        assert!(!leaf.is_dirty());
    }

    #[test]
    fn flex_min_clamp() {
        let mut root = LayoutNode::new();
        root.set_direction(FlexDirection::Row);
        root.set_width(Dim::Point(10.0));
        root.set_height(Dim::Point(10.0));
        let mut a = LayoutNode::new();
        a.set_flex_grow(1.0);
        a.set_min_width(Dim::Point(9.0));
        let mut b = LayoutNode::new();
        b.set_flex_grow(1.0);
        root.insert_child(a, 0);
        root.insert_child(b, 1);
        calculate_layout(&mut root, None, None);
        assert_eq!(root.child(0).unwrap().computed().width, 9.0);
    }

    #[test]
    fn flex_max_clamp() {
        let mut root = LayoutNode::new();
        root.set_direction(FlexDirection::Row);
        root.set_width(Dim::Point(100.0));
        root.set_height(Dim::Point(10.0));
        let mut a = LayoutNode::new();
        a.set_flex_grow(1.0);
        a.set_max_width(Dim::Point(10.0));
        let mut b = LayoutNode::new();
        b.set_flex_grow(1.0);
        root.insert_child(a, 0);
        root.insert_child(b, 1);
        calculate_layout(&mut root, None, None);
        assert_eq!(root.child(0).unwrap().computed().width, 10.0);
        assert_eq!(root.child(1).unwrap().computed().width, 90.0);
    }

    #[test]
    fn flex_percent_and_auto() {
        let mut root = LayoutNode::new();
        root.set_width(Dim::Point(200.0));
        root.set_height(Dim::Point(100.0));
        let mut a = LayoutNode::new();
        a.set_width(Dim::Percent(50.0));
        a.set_height(Dim::Auto);
        a.set_measure_func(|_w, _h| (999.0, 11.0));
        root.insert_child(a, 0);
        calculate_layout(&mut root, None, None);
        let l = root.child(0).unwrap().computed();
        assert_eq!(l.width, 100.0);
        assert_eq!(l.height, 11.0);
    }
}
