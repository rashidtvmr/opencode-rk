#![forbid(unsafe_code)]
//! Knight Rider scanner frames (mirrors `packages/tui/src/ui/spinner.ts`).
//!
//! TS `createFrames` (a0d9b6c `spinner.ts:272-329`): bidirectional cycle
//! Forward (width) + Hold End + Backward (width-1) + Hold Start, defaults
//! width 8 / holdStart 30 / holdEnd 9. Diamonds active shapes
//! `["⬥","◆","⬩","⬪"]` else `"·"`; blocks `"■"` else `"⬝"`
//! (`spinner.ts:313-324`). Trail alphas mirror `deriveTrailColors`
//! (`spinner.ts:209-221`); `spinner.tsx:10` braille frames out of scope.
//! Divergence note: checkout is a0d9b6c, not pinned 95daf90.
//!
//! Hold-dimming correction: `calculateColorIndex` (`spinner.ts:122-126`)
//! returns `directionalDistance + holdProgress` while holding, so the pinned
//! head dims once `holdProgress >= TRAIL_LEN`. End-hold last frame
//! (frame 16, width 8): head cell dist 0 + progress 8 = index 8 >= 6 -> `'·'`.
//! Same for start-hold last frame (frame 53): dist 0 + progress 29 = 29 -> `'·'`.
//! (`createFrames` glyph map `spinner.ts:313-318`: index in range -> shapes,
//! else `'·'`.) Tests assert this, not a permanently bright head.

/// Max scanner width (fail-closed bound).
pub const MAX_WIDTH: usize = 64;
/// TS defaults (`spinner.ts:273-276`).
pub const DEFAULT_WIDTH: usize = 8;
pub const HOLD_START: usize = 30;
pub const HOLD_END: usize = 9;
/// Default trail length = TS default colors array length (`spinner.ts:282-289`).
pub const TRAIL_LEN: usize = 6;

/// Knight Rider glyph set (`spinner.ts:246`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnightRiderStyle {
    Blocks,
    Diamonds,
}

/// Lead/trail alpha for step `i` (`deriveTrailColors`, `spinner.ts:209-221`).
#[must_use]
pub fn trail_alpha(step: usize) -> f32 {
    match step {
        0 => 1.0,
        1 => 0.9,
        _ => 0.65_f32.powi(step as i32 - 1),
    }
}

/// `steps` trail alphas via [`trail_alpha`].
#[must_use]
pub fn derive_trail_steps(steps: usize) -> Vec<f32> {
    (0..steps).map(trail_alpha).collect()
}

/// Active head position + direction for a frame (`getScannerState`, `spinner.ts:32-82`).
fn head(frame: usize, width: usize) -> (usize, bool) {
    if frame < width {
        return (frame, true);
    }
    let hold_end = frame - width;
    if hold_end < HOLD_END {
        return (width - 1, true);
    }
    let back = hold_end - HOLD_END;
    if back < width - 1 {
        return (width - 2 - back, false);
    }
    (0, false)
}

/// Trail index for a cell (`calculateColorIndex`, `spinner.ts:118-138`).
fn trail_index(active: usize, fwd: bool, char_idx: usize, holding: bool, hold_progress: usize) -> i32 {
    let dist = if fwd {
        active as i32 - char_idx as i32
    } else {
        char_idx as i32 - active as i32
    };
    if holding {
        return dist + hold_progress as i32;
    }
    if dist > 0 && (dist as usize) < TRAIL_LEN {
        return dist;
    }
    if dist == 0 {
        return 0;
    }
    -1
}

fn glyph(style: KnightRiderStyle, index: i32) -> char {
    const SHAPES: [char; 4] = ['⬥', '◆', '⬩', '⬪'];
    match style {
        KnightRiderStyle::Diamonds => {
            if index >= 0 && (index as usize) < TRAIL_LEN {
                SHAPES[(index as usize).min(SHAPES.len() - 1)]
            } else {
                '·'
            }
        }
        KnightRiderStyle::Blocks => {
            if index >= 0 && (index as usize) < TRAIL_LEN {
                '■'
            } else {
                '⬝'
            }
        }
    }
}

/// Full bidirectional frame cycle. Width 0 or > MAX_WIDTH yields empty (fail-closed).
#[must_use]
pub fn knight_frames(width: usize, style: KnightRiderStyle) -> Vec<String> {
    if width == 0 || width > MAX_WIDTH {
        return Vec::new();
    }
    let total = width + HOLD_END + (width - 1) + HOLD_START;
    (0..total)
        .map(|frame| {
            let (active, fwd) = head(frame, width);
            let holding = (frame >= width && frame < width + HOLD_END)
                || frame >= width + HOLD_END + (width - 1);
            let hold_progress = if frame >= width && frame < width + HOLD_END {
                frame - width
            } else if holding {
                frame - (width + HOLD_END + (width - 1))
            } else {
                0
            };
            (0..width)
                .map(|c| glyph(style, trail_index(active, fwd, c, holding, hold_progress)))
                .collect()
        })
        .collect()
}

/// Cyclic frame cursor over a [`knight_frames`] cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpinnerState {
    pub frame: usize,
    pub total: usize,
}

impl SpinnerState {
    #[must_use]
    pub const fn new(total: usize) -> Self {
        Self { frame: 0, total }
    }

    /// Advance one frame, wrapping at `total` (0 total stays 0).
    pub fn next_frame(&mut self) -> usize {
        if self.total == 0 {
            return 0;
        }
        self.frame = (self.frame + 1) % self.total;
        self.frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_count_formula() {
        assert_eq!(knight_frames(8, KnightRiderStyle::Diamonds).len(), 8 + 9 + 7 + 30);
        assert_eq!(knight_frames(8, KnightRiderStyle::Diamonds).len(), 54);
    }

    #[test]
    fn diamonds_inactive_dot() {
        let frames = knight_frames(8, KnightRiderStyle::Diamonds);
        assert!(frames[0].starts_with('⬥'));
        assert!(frames[0].contains('·'));
        assert!(!frames[0].contains('⬝'));
    }

    #[test]
    fn blocks_inactive_glyph() {
        let frames = knight_frames(8, KnightRiderStyle::Blocks);
        assert!(frames[0].starts_with('■'));
        assert!(frames[0].contains('⬝'));
        assert!(!frames[0].contains('·'));
    }

    #[test]
    fn width_zero_empty() {
        assert!(knight_frames(0, KnightRiderStyle::Blocks).is_empty());
        assert!(knight_frames(MAX_WIDTH + 1, KnightRiderStyle::Blocks).is_empty());
    }

    #[test]
    fn hold_positions_stable() {
        // TS `calculateColorIndex` (`spinner.ts:122-126`): while holding,
        // index = dist + holdProgress; `createFrames` (`spinner.ts:313-318`):
        // out-of-range index -> inactive '·'. So the pinned head dims late.
        let frames = knight_frames(8, KnightRiderStyle::Diamonds);
        let end_first = &frames[8];
        let end_last = &frames[8 + HOLD_END - 1];
        // End-hold first (frame 8, p0): idx 0 -> '⬥'. Last (frame 16, p8):
        // idx 0+8=8 >= TRAIL_LEN 6 -> '·'.
        assert!(end_first.chars().last() == Some('⬥'));
        assert!(end_last.chars().last() == Some('·'));
        let start_hold = &frames[8 + HOLD_END + 7..];
        assert_eq!(start_hold.len(), HOLD_START);
        // Start-hold first (frame 24, p0): idx 0 -> '⬥'.
        // Last (frame 53, p29): idx 0+29=29 >= 6 -> '·'.
        assert!(start_hold[0].starts_with('⬥'));
        assert!(start_hold[HOLD_START - 1].starts_with('·'));
    }

    #[test]
    fn trail_alpha_values() {
        assert_eq!(trail_alpha(0), 1.0);
        assert_eq!(trail_alpha(1), 0.9);
        assert!((trail_alpha(2) - 0.65).abs() < 1e-6);
        assert!((trail_alpha(3) - 0.65_f32.powi(2)).abs() < 1e-6);
        let steps = derive_trail_steps(6);
        assert_eq!(steps.len(), 6);
        assert_eq!([steps[0], steps[1]], [1.0, 0.9]);
    }

    #[test]
    fn spinner_state_wraps() {
        let mut s = SpinnerState::new(54);
        for _ in 0..53 {
            s.next_frame();
        }
        assert_eq!(s.frame, 53);
        assert_eq!(s.next_frame(), 0);
        let mut z = SpinnerState::new(0);
        assert_eq!(z.next_frame(), 0);
    }
}
