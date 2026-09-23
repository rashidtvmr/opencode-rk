#![forbid(unsafe_code)]
//! Background ring-pulse envelope (mirrors `packages/tui/src/component/bg-pulse-render.ts`).
//!
//! # SOURCE EVIDENCE (TS checkout at /home/rashid/projects/opencode, commit a0d9b6c, NOT pinned 95daf90)
//! - Curve: `bg-pulse-render.ts:4-23` consts: `PERIOD=4600`, `RINGS=3`, `WIDTH=3.8`,
//!   `TAIL=9.5`, `AMP=0.55`, `TAIL_AMP=0.16`, `BREATH_AMP=0.05`, `BREATH_SPEED=0.0008`,
//!   `PHASE_OFFSET=0.29`, `RING_SCALE=1/RINGS`, `TAIL_SCALE=1/TAIL`.
//! - Envelope: `:304-312`: per-ring `phase=(t/PERIOD+i/RINGS-PHASE_OFFSET+1)%1`,
//!   `envelope=sin(phase*PI)`, smoothstep eased `env*env*(3-2*env)`, `head=phase*reach`.
//! - Crest/tail: `:317-337`: `crest=|delta|<WIDTH ? 0.5+0.5*cos(delta/WIDTH*PI) : 0`,
//!   `tail=delta<0&&delta>-TAIL ? (1+delta*TAIL_SCALE)^2.3 : 0`,
//!   `level=(crest*AMP+tail*TAIL_AMP)*eased` summed over 3 rings.
//! - Breath/floor: `:302,334-339`: `breath=(0.5+0.5*sin(t*BREATH_SPEED))*BREATH_AMP`,
//!   `raw=(level*RING_SCALE+breath)*edgeFalloff`, `strength=min(raw,1)*0.7`.
//! - BOLD: `:64` `attributes: x > LOGO_LEFT_WIDTH ? TextAttributes.BOLD : 0`
//!   (right-half logo glyphs bold); stencil `:276` `buffers.attributes[index]=cell.attributes`.
//! - DIVERGENCE: `bg-pulse.tsx` fps pin `:74-86`: `BgPulse` captures
//!   `renderer.targetFps/maxFps` on mount, pins both to 30, restores on
//!   cleanup (`onCleanup :84-87`). Ported as `FPS_PIN` + `FpsPin` below.
//!   Upstream is time-based (`elapsed+=deltaTime`, ms); this module maps
//!   `frame -> elapsed_ms = frame/CACHE_FRAME_COUNT*PERIOD` with
//!   `CACHE_FRAME_COUNT=round(4600/(1000/30))=138` (`:80`).
//! - `MAX_PULSE` (0.7 output scale, `:339` `* 0.7`): the only TS-evidenced cap;
//!   there is no TS `MAX_PULSE` symbol or per-cell max count. Bounds-validated, not a TS const.
//!
//! ponytail: single-ring superposition + breath only, crest/tail spatial field and
//! logo shimmer left out. Upgrade: port drawBackground crest/tail loop + setLogoPulse
//! gaussians (bg-pulse-render.ts:317-371) when per-cell colors needed.

/// Animation period in milliseconds (TS `PERIOD`, :4).
pub const PERIOD_MS: f32 = 4600.0;
/// Ring count (TS `RINGS`, :5).
pub const RINGS: usize = 3;
/// Phase offset so ring emits at logo shimmer peak (TS `PHASE_OFFSET`, :13).
pub const PHASE_OFFSET: f32 = 0.29;
/// Breath amplitude / speed (TS `BREATH_AMP`/`BREATH_SPEED`, :10-11).
pub const BREATH_AMP: f32 = 0.05;
pub const BREATH_SPEED: f32 = 0.0008;
/// Output scale: TS clamps then `* 0.7` (:339). Only evidenced cap.
pub const MAX_PULSE: f32 = 0.7;
/// Cached frames per loop at 30fps (TS `CACHE_FRAME_COUNT`, :80): round(4600/(1000/30)) = 138.
pub const CACHE_FRAME_COUNT: u32 = 138;
/// Pinned renderer fps while the pulse is mounted (TS `bg-pulse.tsx:80-81`
/// `renderer.targetFps = 30; renderer.maxFps = 30`).
pub const FPS_PIN: u16 = 30;

/// Mount guard mirroring `BgPulse` `:77-87`: captures the previous
/// `targetFps`/`maxFps` on mount, pins both to [`FPS_PIN`], restores the
/// captured pair on cleanup (`onCleanup :84-87`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FpsPin {
    /// `(targetFps, maxFps)` captured before pinning.
    pub previous: (u16, u16),
    /// Whether the pin is currently applied.
    pub pinned: bool,
}

impl FpsPin {
    /// Capture `previous` and request the pin (TS `:77-81`).
    #[must_use]
    pub const fn pin(previous: (u16, u16)) -> Self {
        Self { previous, pinned: true }
    }

    /// Value to write back to `targetFps` (TS `:85`).
    #[must_use]
    pub const fn target_fps(self) -> u16 {
        if self.pinned { FPS_PIN } else { self.previous.0 }
    }

    /// Value to write back to `maxFps` (TS `:86`).
    #[must_use]
    pub const fn max_fps(self) -> u16 {
        if self.pinned { FPS_PIN } else { self.previous.1 }
    }

    /// Cleanup restore: returns the captured `(targetFps, maxFps)` pair
    /// (TS `:84-87` `onCleanup`) and marks the pin released.
    pub fn restore(&mut self) -> (u16, u16) {
        self.pinned = false;
        self.previous
    }
}

/// Envelope phase of one ring at elapsed `t_ms` (TS :304-306).
#[must_use]
pub fn ring_phase(t_ms: f32, ring: usize) -> f32 {
    let p = t_ms / PERIOD_MS + ring as f32 / RINGS as f32 - PHASE_OFFSET + 1.0;
    p - p.floor()
}

/// Smoothstep-eased sine envelope of one ring (TS :307-312).
#[must_use]
pub fn ring_envelope(t_ms: f32, ring: usize) -> f32 {
    let env = (ring_phase(t_ms, ring) * core::f32::consts::PI).sin();
    env * env * (3.0 - 2.0 * env)
}

/// Breathing floor glow (TS :302).
#[must_use]
pub fn breath(t_ms: f32) -> f32 {
    (0.5 + 0.5 * (t_ms * BREATH_SPEED).sin()) * BREATH_AMP
}

/// Which ring head is currently sweeping (TS phases, :304-306).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PulsePhase {
    Ring0,
    Ring1,
    Ring2,
    Idle,
}

impl PulsePhase {
    /// Ring with the strongest envelope at `t_ms`; `Idle` on total-zero/NaN.
    #[must_use]
    pub fn at(t_ms: f32) -> Self {
        let e0 = ring_envelope(t_ms, 0);
        let e1 = ring_envelope(t_ms, 1);
        let e2 = ring_envelope(t_ms, 2);
        if !(e0 > 0.0) && !(e1 > 0.0) && !(e2 > 0.0) {
            return Self::Idle;
        }
        if e0 >= e1 && e0 >= e2 {
            Self::Ring0
        } else if e1 >= e2 {
            Self::Ring1
        } else {
            Self::Ring2
        }
    }
}

/// Combined ring level at `t_ms`, superposition of the 3 eased envelopes
/// plus breath (spatial crest/tail folded out; see ponytail note).
#[must_use]
pub fn pulse_alpha(frame: u32) -> f32 {
    let t_ms = frame % CACHE_FRAME_COUNT as u32;
    let t_ms = t_ms as f32 * (PERIOD_MS / CACHE_FRAME_COUNT as f32);
    let level = (ring_envelope(t_ms, 0) + ring_envelope(t_ms, 1) + ring_envelope(t_ms, 2))
        / RINGS as f32
        + breath(t_ms);
    level.clamp(0.0, 1.0) * MAX_PULSE
}

/// A cell stamped by the pulse: bounds-validated position plus bold flag.
/// Bold mirrors TS :64 (`x > LOGO_LEFT_WIDTH ? BOLD : 0`); spatial rule
/// needs the logo width, so callers pass the precomputed flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PulseCell {
    pub x: u16,
    pub y: u16,
    /// Right-half logo glyph (TS :64).
    pub bold: bool,
}

impl PulseCell {
    /// Fail-closed: `None` when outside `width`x`height`.
    #[must_use]
    pub fn new(x: u16, y: u16, width: u16, height: u16, bold: bool) -> Option<Self> {
        if width == 0 || height == 0 || x >= width || y >= height {
            return None;
        }
        Some(Self { x, y, bold })
    }

    /// Attribute bits for the stencil write (TS :64, :276): bold = bit 0.
    #[must_use]
    pub const fn attributes(self) -> u16 {
        if self.bold { 1 } else { 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame0_alpha_matches_ts_curve() {
        // t=0: phases (0.71, 0.0433, 0.3767) -> eased (0.88637626, 0.05025674, 0.98432920);
        // breath 0.025; (1.92096219/3+0.025)*0.7 = 0.46572451.
        assert!((pulse_alpha(0) - 0.46572451).abs() < 1e-5);
    }

    #[test]
    fn frame_wraps_each_period() {
        assert_eq!(pulse_alpha(CACHE_FRAME_COUNT), pulse_alpha(0));
        assert_eq!(pulse_alpha(2 * CACHE_FRAME_COUNT + 1), pulse_alpha(1));
    }

    #[test]
    fn ring2_envelope_dominates_at_frame0() {
        // TS :305: ring1 phase = (0 + 1/3 - 0.29 + 1) % 1 = 0.0433, head nearest origin.
        assert!((ring_phase(0.0, 1) - 0.043333336).abs() < 1e-6);
        assert_eq!(PulsePhase::at(0.0), PulsePhase::Ring2);
    }

    #[test]
    fn breath_floor_and_mid_values() {
        assert_eq!(breath(0.0), 0.025);
        let mid = breath(2300.0);
        assert!((mid - 0.049099575).abs() < 1e-6);
        assert!(pulse_alpha(69) <= MAX_PULSE);
    }

    #[test]
    fn pulse_cell_bounds_and_bold_bit() {
        assert_eq!(PulseCell::new(0, 0, 0, 8, false), None);
        assert_eq!(PulseCell::new(8, 0, 8, 8, false), None);
        assert_eq!(PulseCell::new(0, 8, 8, 8, false), None);
        let bold = PulseCell::new(3, 1, 8, 8, true).unwrap();
        let plain = PulseCell::new(0, 1, 8, 8, false).unwrap();
        assert_eq!((bold.attributes(), plain.attributes()), (1, 0));
    }

    #[test]
    fn fps_pin_pins_both_to_30() {
        let guard = FpsPin::pin((60, 120));
        assert_eq!(FPS_PIN, 30);
        assert_eq!((guard.target_fps(), guard.max_fps()), (30, 30));
        assert!(guard.pinned);
    }

    #[test]
    fn fps_restore_returns_previous_pair() {
        let mut guard = FpsPin::pin((60, 120));
        assert_eq!(guard.restore(), (60, 120));
        assert!(!guard.pinned);
    }

    #[test]
    fn fps_unpinned_reads_previous() {
        let mut guard = FpsPin::pin((60, 120));
        guard.restore();
        assert_eq!((guard.target_fps(), guard.max_fps()), (60, 120));
    }
}
