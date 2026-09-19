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
        Ok(Self { pos_x, pos_y, zoom_pct })
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
        Self { pos_x: 0, pos_y: 0, zoom_pct: DEFAULT_ZOOM_PCT }
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
}
