#![forbid(unsafe_code)]
//! Infinite canvas: camera + viewport with saturating transforms.
//!
//! Screen origin is top-left. World point under camera pos maps to
//! screen (0, 0) at any zoom:
//! `screen = (world - cam.pos) * zoom / 100`.

/// Minimum zoom percent.
pub const MIN_ZOOM_PCT: u16 = 25;
/// Maximum zoom percent.
pub const MAX_ZOOM_PCT: u16 = 400;
/// Default zoom percent (1:1).
pub const DEFAULT_ZOOM_PCT: u16 = 100;

/// Fail-closed zoom error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomError {
    OutOfRange(u16),
}

impl std::fmt::Display for ZoomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfRange(z) => write!(f, "zoom {z} out of range 25-400"),
        }
    }
}

impl std::error::Error for ZoomError {}

/// Clamp raw zoom into `[MIN_ZOOM_PCT, MAX_ZOOM_PCT]`.
#[must_use]
pub const fn clamp_zoom(zoom_pct: u16) -> u16 {
    if zoom_pct < MIN_ZOOM_PCT {
        MIN_ZOOM_PCT
    } else if zoom_pct > MAX_ZOOM_PCT {
        MAX_ZOOM_PCT
    } else {
        zoom_pct
    }
}

/// Camera: top-left world point shown at screen origin + zoom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Camera {
    pub pos_x: i32,
    pub pos_y: i32,
    pub zoom_pct: u16,
}

impl Camera {
    /// Fail-closed: zoom outside 25-400 is `Err`.
    pub const fn new(pos_x: i32, pos_y: i32, zoom_pct: u16) -> Result<Self, ZoomError> {
        if zoom_pct < MIN_ZOOM_PCT || zoom_pct > MAX_ZOOM_PCT {
            return Err(ZoomError::OutOfRange(zoom_pct));
        }
        Ok(Self {
            pos_x,
            pos_y,
            zoom_pct,
        })
    }

    /// Fail-closed zoom setter.
    pub fn set_zoom(&mut self, zoom_pct: u16) -> Result<(), ZoomError> {
        if zoom_pct < MIN_ZOOM_PCT || zoom_pct > MAX_ZOOM_PCT {
            return Err(ZoomError::OutOfRange(zoom_pct));
        }
        self.zoom_pct = zoom_pct;
        Ok(())
    }

    /// Pan by delta with saturating math.
    pub fn pan(&mut self, dx: i32, dy: i32) {
        self.pos_x = self.pos_x.saturating_add(dx);
        self.pos_y = self.pos_y.saturating_add(dy);
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            pos_x: 0,
            pos_y: 0,
            zoom_pct: DEFAULT_ZOOM_PCT,
        }
    }
}

/// Viewport size in screen cells/pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub w: u32,
    pub h: u32,
}

impl Viewport {
    #[must_use]
    pub const fn new(w: u32, h: u32) -> Self {
        Self { w, h }
    }
}

/// World → screen with saturating math.
#[must_use]
pub fn world_to_screen(wx: i32, wy: i32, cam: &Camera, _vp: &Viewport) -> (i32, i32) {
    let z = cam.zoom_pct as i64;
    let sx = (wx as i64 - cam.pos_x as i64) * z / 100;
    let sy = (wy as i64 - cam.pos_y as i64) * z / 100;
    (saturate_i64(sx), saturate_i64(sy))
}

/// Screen → world with saturating math.
#[must_use]
pub fn screen_to_world(sx: i32, sy: i32, cam: &Camera, _vp: &Viewport) -> (i32, i32) {
    let z = cam.zoom_pct as i64;
    let wx = sx as i64 * 100 / z + cam.pos_x as i64;
    let wy = sy as i64 * 100 / z + cam.pos_y as i64;
    (saturate_i64(wx), saturate_i64(wy))
}

/// Visible world rect `(x, y, w, h)` for viewport at camera zoom.
#[must_use]
pub fn visible_rect(cam: &Camera, vp: &Viewport) -> (i32, i32, u32, u32) {
    let z = cam.zoom_pct as u64;
    let w = (vp.w as u64 * 100 / z).min(u32::MAX as u64) as u32;
    let h = (vp.h as u64 * 100 / z).min(u32::MAX as u64) as u32;
    (cam.pos_x, cam.pos_y, w.max(1), h.max(1))
}

fn saturate_i64(v: i64) -> i32 {
    if v > i32::MAX as i64 {
        i32::MAX
    } else if v < i32::MIN as i64 {
        i32::MIN
    } else {
        v as i32
    }
}

/// Shell region in paint order (mirrors CLI `ShellLayout` fields).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    Transcript,
    Composer,
    Sidebar,
    Status,
}

/// Paint order: transcript, composer, sidebar, status.
pub const PAINT_ORDER: [Region; 4] = [
    Region::Transcript,
    Region::Composer,
    Region::Sidebar,
    Region::Status,
];

/// One paint call: region + its absolute screen rect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaintCall {
    pub region: Region,
    pub rect: crate::layout::Rect,
}

/// Pure scene composition: shell region rects mapped to paint calls.
/// CLI `ShellLayout` (u16 rects) converts in via `ShellRegions::new`;
/// CLI wires later. Empty rects paint nothing; non-empty pairs are disjoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellRegions {
    pub transcript: crate::layout::Rect,
    pub composer: crate::layout::Rect,
    pub sidebar: crate::layout::Rect,
    pub status: crate::layout::Rect,
    pub compact: bool,
}

impl ShellRegions {
    #[must_use]
    pub const fn new(
        transcript: crate::layout::Rect,
        composer: crate::layout::Rect,
        sidebar: crate::layout::Rect,
        status: crate::layout::Rect,
        compact: bool,
    ) -> Self {
        Self {
            transcript,
            composer,
            sidebar,
            status,
            compact,
        }
    }

    #[must_use]
    pub const fn rect_of(self, region: Region) -> crate::layout::Rect {
        match region {
            Region::Transcript => self.transcript,
            Region::Composer => self.composer,
            Region::Sidebar => self.sidebar,
            Region::Status => self.status,
        }
    }

    fn is_empty(r: crate::layout::Rect) -> bool {
        r.w == 0 || r.h == 0
    }

    fn overlaps(a: crate::layout::Rect, b: crate::layout::Rect) -> bool {
        if Self::is_empty(a) || Self::is_empty(b) {
            return false;
        }
        a.x < b.x.saturating_add(b.w)
            && b.x < a.x.saturating_add(a.w)
            && a.y < b.y.saturating_add(b.h)
            && b.y < a.y.saturating_add(a.h)
    }

    /// Paint calls in `PAINT_ORDER`, skipping empty rects.
    #[must_use]
    pub fn paint_calls(self) -> Vec<PaintCall> {
        let mut out = Vec::with_capacity(4);
        for region in PAINT_ORDER {
            let rect = self.rect_of(region);
            if !Self::is_empty(rect) {
                out.push(PaintCall { region, rect });
            }
        }
        out
    }

    /// True when two non-empty regions intersect.
    #[must_use]
    pub fn has_overlap(self) -> bool {
        let rs = [self.transcript, self.composer, self.sidebar, self.status];
        for i in 0..rs.len() {
            if Self::is_empty(rs[i]) {
                continue;
            }
            for other in rs.iter().skip(i + 1) {
                if Self::is_empty(*other) {
                    continue;
                }
                if Self::overlaps(rs[i], *other) {
                    return true;
                }
            }
        }
        false
    }
}

/// Visible world rect `(x, y, w, h)` for one region at camera zoom.
/// Empty region composes to zero area (paints nothing).
#[must_use]
pub fn region_world_rect(cam: &Camera, region: &crate::layout::Rect) -> (i32, i32, u32, u32) {
    if region.w == 0 || region.h == 0 {
        return (cam.pos_x, cam.pos_y, 0, 0);
    }
    let z = cam.zoom_pct as u64;
    let w = (region.w as u64 * 100 / z).min(u32::MAX as u64) as u32;
    let h = (region.h as u64 * 100 / z).min(u32::MAX as u64) as u32;
    (cam.pos_x, cam.pos_y, w.max(1), h.max(1))
}

/// World point to absolute screen cell through a region camera anchored
/// at the region origin. `None` when outside the region or empty.
#[must_use]
pub fn region_world_to_screen(
    wx: i32,
    wy: i32,
    cam: &Camera,
    region: &crate::layout::Rect,
) -> Option<(u32, u32)> {
    if region.w == 0 || region.h == 0 {
        return None;
    }
    let vp = Viewport::new(region.w, region.h);
    let (sx, sy) = world_to_screen(wx, wy, cam, &vp);
    let ox = region.x as i64 + sx as i64;
    let oy = region.y as i64 + sy as i64;
    let x0 = region.x as i64;
    let y0 = region.y as i64;
    let ex = region.x as u64 + region.w as u64;
    let ey = region.y as u64 + region.h as u64;
    if ox < x0 || oy < y0 || ox as u64 >= ex || oy as u64 >= ey {
        return None;
    }
    Some((ox as u32, oy as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam(x: i32, y: i32, z: u16) -> Camera {
        Camera::new(x, y, z).unwrap()
    }

    #[test]
    fn roundtrip_at_various_zooms() {
        let vp = Viewport::new(800, 600);
        for z in [25, 100, 150, 400] {
            let c = cam(10, -20, z);
            // One screen unit spans ceil(100/z) world units; truncation can
            // lose up to that quantum per axis.
            let tol = (100 / i32::from(z)) + 1;
            for (wx, wy) in [(0, 0), (10, -20), (100, 200), (-500, 300)] {
                let (sx, sy) = world_to_screen(wx, wy, &c, &vp);
                let back = screen_to_world(sx, sy, &c, &vp);
                assert!((back.0 - wx).abs() <= tol, "x {wx} -> {sx} -> {}", back.0);
                assert!((back.1 - wy).abs() <= tol, "y {wy} -> {sy} -> {}", back.1);
            }
        }
    }

    #[test]
    fn zoom_fail_closed_and_clamp() {
        assert!(Camera::new(0, 0, 24).is_err());
        assert!(Camera::new(0, 0, 401).is_err());
        assert!(Camera::new(0, 0, 100).is_ok());
        let mut c = Camera::default();
        assert!(c.set_zoom(0).is_err());
        assert_eq!(c.zoom_pct, 100);
        assert!(c.set_zoom(200).is_ok());
        assert_eq!(clamp_zoom(0), MIN_ZOOM_PCT);
        assert_eq!(clamp_zoom(500), MAX_ZOOM_PCT);
        assert_eq!(clamp_zoom(150), 150);
    }

    #[test]
    fn pan_offset_and_visible_rect() {
        let mut c = cam(0, 0, 100);
        let vp = Viewport::new(800, 600);
        c.pan(50, -30);
        let (sx, sy) = world_to_screen(50, -30, &c, &vp);
        assert_eq!((sx, sy), (0, 0));
        let (sx2, _) = world_to_screen(60, -30, &c, &vp);
        assert_eq!(sx2, 10);
        assert_eq!(visible_rect(&c, &vp), (50, -30, 800, 600));
        let z2 = cam(0, 0, 200);
        assert_eq!(visible_rect(&z2, &vp), (0, 0, 400, 300));
        // Saturating: no panic at extremes.
        let big = cam(i32::MAX, i32::MIN, 400);
        let _ = world_to_screen(i32::MIN, i32::MAX, &big, &vp);
        let _ = screen_to_world(i32::MAX, i32::MAX, &big, &vp);
    }

    // RED: scene composition for shell pages (CLI wires later).
    use crate::layout::Rect as CellRect;

    fn full_regions() -> ShellRegions {
        ShellRegions::new(
            CellRect::new(0, 0, 90, 18),
            CellRect::new(0, 18, 120, 5),
            CellRect::new(90, 0, 30, 18),
            CellRect::new(0, 23, 120, 1),
            false,
        )
    }

    #[test]
    fn full_shell_paints_all_four_in_order_without_overlap() {
        let regions = full_regions();
        let calls = regions.paint_calls();
        assert_eq!(calls.len(), 4, "full shell paints every region");
        let order: Vec<Region> = calls.iter().map(|c| c.region).collect();
        assert_eq!(order, PAINT_ORDER.to_vec());
        assert_eq!(calls[0].rect, regions.rect_of(Region::Transcript));
        assert!(!regions.has_overlap(), "shell regions must be disjoint");
    }

    #[test]
    fn compact_shell_skips_empty_sidebar() {
        let regions = ShellRegions::new(
            CellRect::new(0, 0, 40, 6),
            CellRect::new(0, 6, 40, 3),
            CellRect::new(0, 0, 0, 0),
            CellRect::new(0, 9, 40, 1),
            true,
        );
        let calls = regions.paint_calls();
        assert_eq!(calls.len(), 3, "empty sidebar paints nothing");
        assert!(calls.iter().all(|c| c.region != Region::Sidebar));
        assert!(!regions.has_overlap());
    }

    #[test]
    fn empty_shell_composes_nothing() {
        let regions = ShellRegions::new(
            CellRect::new(0, 0, 0, 0),
            CellRect::new(0, 0, 0, 0),
            CellRect::new(0, 0, 0, 0),
            CellRect::new(0, 0, 0, 0),
            true,
        );
        assert!(regions.paint_calls().is_empty());
        assert!(!regions.has_overlap(), "empty rects never overlap");
    }

    #[test]
    fn region_world_rect_scales_with_zoom() {
        let c = cam(10, 20, 200);
        let rect = CellRect::new(0, 0, 800, 600);
        assert_eq!(region_world_rect(&c, &rect), (10, 20, 400, 300));
        let empty = CellRect::new(0, 0, 0, 0);
        assert_eq!(region_world_rect(&c, &empty), (10, 20, 0, 0));
    }

    #[test]
    fn region_world_to_screen_clips_outside() {
        let c = cam(0, 0, 100);
        let region = CellRect::new(90, 0, 30, 18);
        assert_eq!(region_world_to_screen(5, 7, &c, &region), Some((95, 7)));
        assert_eq!(region_world_to_screen(0, 0, &c, &region), Some((90, 0)));
        assert_eq!(region_world_to_screen(30, 0, &c, &region), None);
        assert_eq!(region_world_to_screen(0, 18, &c, &region), None);
        assert_eq!(region_world_to_screen(-1, 0, &c, &region), None);
    }
}
