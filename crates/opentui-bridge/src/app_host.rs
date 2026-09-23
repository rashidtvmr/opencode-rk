//! App-host lifecycle contract: startup acquire order + shutdown finalizer order.
//!
//! Upstream: `packages/tui/src/app.tsx` `run` (TS checkout a0d9b6c, NOT
//! pinned 95daf90; lines against a0d9b6c). Reuses `RendererConfig`/
//! `Lifecycle` from `renderer_lifecycle` (mirrors `createCliRenderer` opts +
//! `destroyRenderer` guard); does NOT redefine them.

#![forbid(unsafe_code)]

use std::error::Error;
use std::fmt;

use crate::renderer_lifecycle::{Lifecycle, LifecycleError};

/// One startup acquire, in `run` order (app.tsx:186-236).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupStep {
    /// `Effect.acquireRelease(createCliRenderer(...))`, release =
    /// `destroyRenderer` (app.tsx:191-213). `exitOnCtrlC: false` (app.tsx:198).
    Renderer,
    /// `win32DisableProcessedInput()` (app.tsx:214).
    Win32Input,
    /// `createDefaultOpenTuiKeymap` + `acquireRelease(registerOpencodeKeymap)`
    /// (app.tsx:215-219).
    Keymap,
    /// `addFinalizer(pluginHost.dispose)` (app.tsx:220-228).
    PluginFinalizer,
    /// `addFinalizer(TuiAudio.dispose)` (app.tsx:229).
    AudioFinalizer,
    /// `Deferred.make` shutdown gate + SIGHUP hook + `renderer.once("destroy")`
    /// (app.tsx:230-236).
    ShutdownGate,
}

/// Exact acquire sequence of `run`. Prewarm/palette + `render()` + first draw
/// (`getPalette` app.tsx:241, `waitForThemeMode(1000)` app.tsx:242,
/// `<TimeToFirstDraw/>` app.tsx:1108) happen after the gate is armed.
pub const STARTUP_ORDER: &[StartupStep] = &[
    StartupStep::Renderer,
    StartupStep::Win32Input,
    StartupStep::Keymap,
    StartupStep::PluginFinalizer,
    StartupStep::AudioFinalizer,
    StartupStep::ShutdownGate,
];

/// Shutdown finalizers, innermost-first (reverse of acquisition):
/// plugins (app.tsx:220-228) -> audio (app.tsx:229) -> renderer destroy
/// (app.tsx:209-212 release).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownStep {
    Plugins,
    Audio,
    Renderer,
}

pub const SHUTDOWN_ORDER: &[ShutdownStep] =
    &[ShutdownStep::Plugins, ShutdownStep::Audio, ShutdownStep::Renderer];

/// Minimal host input mirror: `useMouse` flag (app.tsx:202) + resolved theme
/// mode string (`waitForThemeMode(...) ?? "dark"`, app.tsx:242).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiInput {
    pub mouse: bool,
    pub theme: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiInputError {
    EmptyTheme,
}

impl fmt::Display for TuiInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTheme => f.write_str("theme must not be empty"),
        }
    }
}

impl Error for TuiInputError {}

impl TuiInput {
    pub fn validate(&self) -> Result<(), TuiInputError> {
        if self.theme.is_empty() {
            return Err(TuiInputError::EmptyTheme);
        }
        Ok(())
    }
}

/// First-draw deadline. Default mirrors `waitForThemeMode(1000)` (app.tsx:242).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirstDraw {
    pub deadline_ms: u64,
}

impl Default for FirstDraw {
    fn default() -> Self {
        Self { deadline_ms: 1000 }
    }
}

impl FirstDraw {
    #[must_use]
    pub fn breached(self, elapsed_ms: u64) -> bool {
        elapsed_ms > self.deadline_ms
    }
}

/// Host owning the renderer [`Lifecycle`]; start is one-shot.
#[derive(Debug, Default)]
pub struct AppHost {
    lifecycle: Lifecycle,
}

impl AppHost {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&mut self) -> Result<(), LifecycleError> {
        self.lifecycle = self.lifecycle.start()?;
        Ok(())
    }

    pub fn destroy(&mut self) -> Result<(), LifecycleError> {
        self.lifecycle = self.lifecycle.destroy()?;
        Ok(())
    }

    #[must_use]
    pub fn lifecycle(self) -> Lifecycle {
        self.lifecycle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_matches_app_tsx_sequence() {
        assert_eq!(
            STARTUP_ORDER,
            &[
                StartupStep::Renderer,
                StartupStep::Win32Input,
                StartupStep::Keymap,
                StartupStep::PluginFinalizer,
                StartupStep::AudioFinalizer,
                StartupStep::ShutdownGate,
            ]
        );
    }

    #[test]
    fn shutdown_is_reverse_of_finalizer_acquire() {
        // Acquired: renderer release, plugin finalizer, audio finalizer.
        // Released innermost-first: plugins -> audio -> renderer destroy.
        assert_eq!(
            SHUTDOWN_ORDER,
            &[ShutdownStep::Plugins, ShutdownStep::Audio, ShutdownStep::Renderer]
        );
        let acquired = [ShutdownStep::Renderer, ShutdownStep::Audio, ShutdownStep::Plugins];
        let mut rev = acquired.to_vec();
        rev.reverse();
        assert_eq!(SHUTDOWN_ORDER, rev.as_slice());
    }

    #[test]
    fn double_start_errs_via_lifecycle() {
        let mut host = AppHost::new();
        host.start().unwrap();
        assert_eq!(
            host.start(),
            Err(LifecycleError::IllegalTransition { from: Lifecycle::Running, event: "start" })
        );
    }

    #[test]
    fn start_then_destroy_ok() {
        let mut host = AppHost::new();
        host.start().unwrap();
        host.destroy().unwrap();
        assert_eq!(host.lifecycle(), Lifecycle::Destroyed);
    }

    #[test]
    fn deadline_breach() {
        let fd = FirstDraw::default();
        assert_eq!(fd.deadline_ms, 1000);
        assert!(!fd.breached(1000));
        assert!(fd.breached(1001));
    }

    #[test]
    fn tui_input_validate() {
        let ok = TuiInput { mouse: true, theme: String::from("dark") };
        assert!(ok.validate().is_ok());
        let bad = TuiInput { mouse: false, theme: String::new() };
        assert_eq!(bad.validate(), Err(TuiInputError::EmptyTheme));
    }
}
