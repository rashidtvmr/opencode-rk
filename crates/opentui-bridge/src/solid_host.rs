#![forbid(unsafe_code)]
//! Solid renderer-host contract mirror (USED `@opentui/solid` boundary only).
//!
//! TS checkout a0d9b6c (NOT pinned 95daf90). Unvendored npm source; this file
//! mirrors the used surface, it does not reimplement the Solid runtime.
//! Native-owned: reactivity (`createSignal` etc.) stays in TS; Rust keeps
//! dims/slot-id/TTFD guards only.
//!
//! Used API evidence (`packages/tui/src`):
//! - `render` + `TimeToFirstDraw` + `useRenderer` + `useTerminalDimensions`:
//!   `app.tsx:1`, `render(...)` at `app.tsx:245`, `<TimeToFirstDraw/>` at
//!   `app.tsx:1108`, `dimensions().width/height` at `app.tsx:1089-1090`.
//! - `Portal`: `routes/session/permission.tsx:4`, used at `:714` for expanded
//!   permission overlay.
//! - `useKeyboard`: `component/dialog-workspace-file-changes.tsx:2,46`,
//!   `component/error-component.tsx:2,60` (`evt.name`, preventDefault,
//!   stopPropagation). NOTE: app keybindings (`useBindings`) come from
//!   `@opentui/keymap/solid` via `keymap.tsx:15`, NOT `@opentui/solid`.
//! - `createSlot` + `createSolidSlotRegistry` + `SolidPlugin` + `JSX`:
//!   `plugin/slots.tsx:2,33-48` (`registry.register(plugin)` returns opaque
//!   unregister fn; `dispose` only resets view at `:56-58`).
//! - `extend`: `component/bg-pulse.tsx:8,69` (`extend({go_upsell_art...})`).
//! - `getComponentCatalogue` + `registerSpinner`: NOT `@opentui/solid` root;
//!   `component/register-spinner.ts:1-2`
//!   (`@opentui/solid/components`, `opentui-spinner/solid`). Out of scope.
//! - `targetFps`: `component/bg-pulse.tsx:74-86` (`renderer.targetFps/maxFps`
//!   save-set-restore); `createCliRenderer({targetFps:60})` at `app.tsx:196`
//!   is `@opentui/core`, not solid. Kept here as optional hint only.
//!
//! ponytail: no Solid runtime, no renderer lifecycle reuse (dims-only local
//! `RenderOptions`); add full lifecycle wiring when native renderer lands.

/// Max slot id (fail-closed bound; TS registry unbounded).
pub const MAX_SLOT_ID: u32 = 1024;
/// Max terminal dimension cells (fail-closed; TS plain numbers).
pub const MAX_DIM: u32 = 4096;
/// Max target fps hint (mirrors `renderer_lifecycle` 1..=240, kept local).
pub const MAX_FPS: u16 = 240;

/// Which used `@opentui/solid` APIs the host exposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HostCaps {
    /// `createSlot`/`createSolidSlotRegistry` (`plugin/slots.tsx:33-48`).
    pub slots: bool,
    /// `Portal` (`permission.tsx:714`).
    pub portal: bool,
    /// `useKeyboard` (`dialog-workspace-file-changes.tsx:46`).
    pub keyboard: bool,
    /// `useTerminalDimensions` (`app.tsx:369,1089-1090`).
    pub dimensions: bool,
}

impl HostCaps {
    /// Caps for the full used boundary.
    #[must_use]
    pub const fn all_used() -> Self {
        Self { slots: true, portal: true, keyboard: true, dimensions: true }
    }

    /// Names of enabled caps; empty when nothing enabled.
    #[must_use]
    pub fn active_names(self) -> Vec<&'static str> {
        let mut out = Vec::with_capacity(4);
        if self.slots {
            out.push("slots");
        }
        if self.portal {
            out.push("portal");
        }
        if self.keyboard {
            out.push("keyboard");
        }
        if self.dimensions {
            out.push("dimensions");
        }
        out
    }
}

/// Terminal size from `useTerminalDimensions()` (`app.tsx:1089-1090`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalDimensions {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimsError {
    Zero,
    TooLarge,
}

impl core::fmt::Display for DimsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Zero => f.write_str("terminal dimensions must be nonzero"),
            Self::TooLarge => f.write_str("terminal dimensions too large"),
        }
    }
}

impl std::error::Error for DimsError {}

impl TerminalDimensions {
    /// Fail-closed: zero errs, over `MAX_DIM` errs.
    pub fn validate(self) -> Result<(), DimsError> {
        if self.width == 0 || self.height == 0 {
            return Err(DimsError::Zero);
        }
        if self.width > MAX_DIM || self.height > MAX_DIM {
            return Err(DimsError::TooLarge);
        }
        Ok(())
    }

    /// Narrow-layout predicate (`permission.tsx:448,541`: `width < 80`).
    #[must_use]
    pub const fn is_narrow(self) -> bool {
        self.width < 80
    }
}

/// Dims-only render hint (`bg-pulse.tsx:80-81` fps save-set-restore).
/// Local; deliberately does NOT reuse `renderer_lifecycle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RenderOptions {
    pub target_fps: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionsError {
    InvalidFps(u16),
}

impl core::fmt::Display for OptionsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidFps(fps) => write!(f, "target_fps out of range 1..=240: {fps}"),
        }
    }
}

impl std::error::Error for OptionsError {}

impl RenderOptions {
    pub fn validate(self) -> Result<(), OptionsError> {
        if let Some(fps) = self.target_fps {
            if !(1..=MAX_FPS).contains(&fps) {
                return Err(OptionsError::InvalidFps(fps));
            }
        }
        Ok(())
    }
}

/// Bounded slot handle (TS `registry.register` returns opaque unregister fn;
/// Rust uses an explicit bounded id instead).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlotId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotIdError {
    OutOfBounds(u32),
}

impl core::fmt::Display for SlotIdError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OutOfBounds(id) => write!(f, "slot id out of bounds 0..={MAX_SLOT_ID}: {id}"),
        }
    }
}

impl std::error::Error for SlotIdError {}

impl SlotId {
    pub fn new(id: u32) -> Result<Self, SlotIdError> {
        if id > MAX_SLOT_ID {
            return Err(SlotIdError::OutOfBounds(id));
        }
        Ok(Self(id))
    }
}

/// First-draw deadline (`<TimeToFirstDraw/>`, `app.tsx:1107-1109` behind
/// `Flag.OPENCODE_SHOW_TTFD`). Native-owned check helper only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeToFirstDraw {
    pub deadline_ms: u64,
}

impl TimeToFirstDraw {
    /// True when `elapsed_ms` exceeds the deadline.
    #[must_use]
    pub const fn is_breached(self, elapsed_ms: u64) -> bool {
        elapsed_ms > self.deadline_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dims_zero_errs() {
        assert_eq!(
            TerminalDimensions { width: 0, height: 24 }.validate(),
            Err(DimsError::Zero)
        );
        assert_eq!(
            TerminalDimensions { width: 80, height: 0 }.validate(),
            Err(DimsError::Zero)
        );
    }

    #[test]
    fn dims_ok_and_narrow() {
        let wide = TerminalDimensions { width: 120, height: 30 };
        assert!(wide.validate().is_ok());
        assert!(!wide.is_narrow());
        assert!(TerminalDimensions { width: 79, height: 30 }.is_narrow());
        assert_eq!(
            TerminalDimensions { width: MAX_DIM + 1, height: 24 }.validate(),
            Err(DimsError::TooLarge)
        );
    }

    #[test]
    fn slot_id_bound() {
        assert!(SlotId::new(MAX_SLOT_ID).is_ok());
        assert_eq!(SlotId::new(MAX_SLOT_ID + 1), Err(SlotIdError::OutOfBounds(MAX_SLOT_ID + 1)));
    }

    #[test]
    fn ttf_draw_ok() {
        let t = TimeToFirstDraw { deadline_ms: 1000 };
        assert!(!t.is_breached(0));
        assert!(!t.is_breached(1000));
    }

    #[test]
    fn ttf_draw_breached() {
        let t = TimeToFirstDraw { deadline_ms: 1000 };
        assert!(t.is_breached(1001));
    }

    #[test]
    fn caps_list_non_empty() {
        let caps = HostCaps::all_used();
        let names = caps.active_names();
        assert_eq!(names, vec!["slots", "portal", "keyboard", "dimensions"]);
        assert!(HostCaps::default().active_names().is_empty());
    }

    #[test]
    fn render_options_fps_bound() {
        assert!(RenderOptions { target_fps: Some(30) }.validate().is_ok());
        assert!(RenderOptions { target_fps: None }.validate().is_ok());
        assert_eq!(
            RenderOptions { target_fps: Some(0) }.validate(),
            Err(OptionsError::InvalidFps(0))
        );
    }
}
