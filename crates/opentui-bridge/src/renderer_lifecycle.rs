//! Renderer lifecycle: config mirror + fail-closed state machine.
//!
//! Upstream: `packages/tui/src/app.tsx` `createCliRenderer` call site
//! (TS checkout a0d9b6c, NOT pinned 95daf90) and
//! `packages/tui/src/util/renderer.ts` `destroyRenderer` guard.
//! Pure safe code; no FFI here (FFI lives in `renderer.rs`).

#![forbid(unsafe_code)]

use std::error::Error;
use std::fmt;

/// Mirrors `externalOutputMode` option. Only value used by app.tsx is
/// `"passthrough"`; kept as enum so other modes fail loudly at parse time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExternalOutputMode {
    #[default]
    Passthrough,
}

/// Mirrors every `createCliRenderer` option used in `app.tsx`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererConfig {
    pub external_output_mode: ExternalOutputMode,
    pub target_fps: u16,
    pub gather_stats: bool,
    pub exit_on_ctrl_c: bool,
    pub auto_focus: bool,
    pub open_console_on_error: bool,
    pub use_mouse: bool,
    pub kitty_flags: u8,
    pub console_copy_key: String,
}

/// Defaults mirror `app.tsx` call site: passthrough, 60fps, all false except
/// `useMouse` (true when mouse not disabled), kitty `from_empty()` (=5,
/// disambiguate+alternateKeys on), copy key `"y"`.
impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            external_output_mode: ExternalOutputMode::Passthrough,
            target_fps: 60,
            gather_stats: false,
            exit_on_ctrl_c: false,
            auto_focus: false,
            open_console_on_error: false,
            use_mouse: true,
            kitty_flags: 5,
            console_copy_key: String::from("y"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererConfigError {
    InvalidFps(u16),
    EmptyCopyKey,
}

impl fmt::Display for RendererConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFps(fps) => write!(f, "target_fps out of range 1..=240: {fps}"),
            Self::EmptyCopyKey => f.write_str("console_copy_key must not be empty"),
        }
    }
}

impl Error for RendererConfigError {}

impl RendererConfig {
    /// Fail-closed validation: fps must be 1..=240, copy key non-empty.
    pub fn validate(&self) -> Result<(), RendererConfigError> {
        if !(1..=240).contains(&self.target_fps) {
            return Err(RendererConfigError::InvalidFps(self.target_fps));
        }
        if self.console_copy_key.is_empty() {
            return Err(RendererConfigError::EmptyCopyKey);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleError {
    IllegalTransition { from: Lifecycle, event: &'static str },
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IllegalTransition { from, event } => {
                write!(f, "illegal lifecycle {event} from {from:?}")
            }
        }
    }
}

impl Error for LifecycleError {}

/// Fail-closed renderer state. Mirrors `renderer.ts isDestroyed` guard plus
/// native suspend/resume (`renderer.rs` `suspendRenderer`/`resumeRenderer`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lifecycle {
    #[default]
    Created,
    Running,
    Suspended,
    Destroyed,
}

impl Lifecycle {
    pub fn start(self) -> Result<Self, LifecycleError> {
        match self {
            Self::Created => Ok(Self::Running),
            _ => Err(LifecycleError::IllegalTransition { from: self, event: "start" }),
        }
    }

    pub fn suspend(self) -> Result<Self, LifecycleError> {
        match self {
            Self::Running => Ok(Self::Suspended),
            _ => Err(LifecycleError::IllegalTransition { from: self, event: "suspend" }),
        }
    }

    pub fn resume(self) -> Result<Self, LifecycleError> {
        match self {
            Self::Suspended => Ok(Self::Running),
            _ => Err(LifecycleError::IllegalTransition { from: self, event: "resume" }),
        }
    }

    /// Destroy from any live state (Created/Running/Suspended). Double-destroy errs.
    pub fn destroy(self) -> Result<Self, LifecycleError> {
        match self {
            Self::Destroyed => {
                Err(LifecycleError::IllegalTransition { from: self, event: "destroy" })
            }
            _ => Ok(Self::Destroyed),
        }
    }

    /// Guard mirroring `destroyRenderer` early-return: only live states need destroy.
    pub fn needs_destroy(self) -> bool {
        self != Self::Destroyed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_app_tsx() {
        let c = RendererConfig::default();
        assert_eq!(c.external_output_mode, ExternalOutputMode::Passthrough);
        assert_eq!(c.target_fps, 60);
        assert!(!c.gather_stats);
        assert!(!c.exit_on_ctrl_c);
        assert!(!c.auto_focus);
        assert!(!c.open_console_on_error);
        assert_eq!(c.kitty_flags, 5);
        assert_eq!(c.console_copy_key, "y");
        assert!(c.validate().is_ok());
    }

    #[test]
    fn fps_zero_errs() {
        let mut c = RendererConfig::default();
        c.target_fps = 0;
        assert_eq!(c.validate(), Err(RendererConfigError::InvalidFps(0)));
    }

    #[test]
    fn fps_300_errs() {
        let mut c = RendererConfig::default();
        c.target_fps = 300;
        assert_eq!(c.validate(), Err(RendererConfigError::InvalidFps(300)));
    }

    #[test]
    fn suspend_resume_cycle() {
        let s = Lifecycle::Created.start().unwrap();
        assert_eq!(s, Lifecycle::Running);
        let s = s.suspend().unwrap();
        assert_eq!(s, Lifecycle::Suspended);
        let s = s.resume().unwrap();
        assert_eq!(s, Lifecycle::Running);
        let s = s.destroy().unwrap();
        assert!(!s.needs_destroy());
    }

    #[test]
    fn double_destroy_errs() {
        let s = Lifecycle::Created.destroy().unwrap();
        assert_eq!(
            s.destroy(),
            Err(LifecycleError::IllegalTransition { from: Lifecycle::Destroyed, event: "destroy" })
        );
    }

    #[test]
    fn destroy_from_created_ok() {
        let s = Lifecycle::Created.destroy().unwrap();
        assert_eq!(s, Lifecycle::Destroyed);
        assert!(!s.needs_destroy());
        assert!(Lifecycle::Running.needs_destroy());
    }

    #[test]
    fn illegal_resume_errs() {
        assert_eq!(
            Lifecycle::Created.resume(),
            Err(LifecycleError::IllegalTransition { from: Lifecycle::Created, event: "resume" })
        );
        assert_eq!(
            Lifecycle::Running.resume(),
            Err(LifecycleError::IllegalTransition { from: Lifecycle::Running, event: "resume" })
        );
    }
}
