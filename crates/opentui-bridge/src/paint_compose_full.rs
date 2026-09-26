#![forbid(unsafe_code)]

use crate::layout::ShellRegions as ShellRegionsLayout;
use crate::world::{Region, PAINT_ORDER};

/// Compose shell regions into `(region_id, x, y, w, h)` in `PAINT_ORDER`.
/// Skips empty rects; saturates u32 coords to u16. Id = `Region` ordinal.
#[must_use]
pub fn compose_calls(regions: ShellRegionsLayout) -> Vec<(u8, u16, u16, u16, u16)> {
    let mut out = Vec::new();
    for region in PAINT_ORDER {
        let rect = match region {
            Region::Transcript => regions.transcript,
            Region::Composer => regions.composer,
            Region::Sidebar => regions.sidebar,
            Region::Status => regions.status,
        };
        if rect.is_empty() {
            continue;
        }
        out.push((
            region as u8,
            rect.x.min(u16::MAX as u32) as u16,
            rect.y.min(u16::MAX as u32) as u16,
            rect.w.min(u16::MAX as u32) as u16,
            rect.h.min(u16::MAX as u32) as u16,
        ));
    }
    out
}

/// Clip `text` to `width` chars (char-level). Zero width yields `""`.
#[must_use]
pub fn frame_line(text: &str, width: u16) -> String {
    text.chars().take(width as usize).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Rect;

    fn r(x: u16, y: u16, w: u16, h: u16) -> Rect {
        Rect::new(x as u32, y as u32, w as u32, h as u32)
    }

    #[test]
    fn compose_calls_skips_empty() {
        let regions = ShellRegionsLayout {
            transcript: r(0, 0, 10, 5),
            composer: r(0, 5, 10, 3),
            sidebar: r(0, 0, 0, 0),
            status: r(0, 8, 10, 1),
            compact: true,
        };
        let calls = compose_calls(regions);
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].0, Region::Transcript as u8);
        assert_eq!(calls[1].0, Region::Composer as u8);
        assert_eq!(calls[2].0, Region::Status as u8);
    }

    #[test]
    fn compose_calls_all_empty() {
        let z = r(0, 0, 0, 0);
        let regions = ShellRegionsLayout {
            transcript: z,
            composer: z,
            sidebar: z,
            status: z,
            compact: true,
        };
        assert!(compose_calls(regions).is_empty());
    }

    #[test]
    fn compose_calls_paint_order() {
        let one = r(0, 0, 1, 1);
        let regions = ShellRegionsLayout {
            transcript: one,
            composer: one,
            sidebar: one,
            status: one,
            compact: false,
        };
        let ids: Vec<u8> = compose_calls(regions).iter().map(|c| c.0).collect();
        assert_eq!(
            ids,
            [
                Region::Transcript as u8,
                Region::Composer as u8,
                Region::Sidebar as u8,
                Region::Status as u8,
            ]
        );
    }

    #[test]
    fn frame_line_clips_and_bounds() {
        assert_eq!(frame_line("hello world", 5), "hello");
        assert_eq!(frame_line("ab", 5), "ab");
        assert_eq!(frame_line("anything", 0), "");
        assert_eq!(frame_line("a\u{1F389}b", 2), "a\u{1F389}");
    }
}
