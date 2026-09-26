#![forbid(unsafe_code)]
//! Toast view slot (mirrors `packages/tui/src/ui/toast.tsx:15-102`).
//!
//! Reuses [`crate::toast::ToastOptions`] / `effective_duration_ms` /
//! `toast_width`; does NOT redefine them. Divergence: [`crate::toast::ToastQueue`]
//! is a bounded FIFO, but TS keeps a single `currentToast` (replace-on-new +
//! timeout clear, toast.tsx:54-67); [`ToastProvider`] mirrors the TS single slot.

use crate::toast::{ToastOptions, ToastVariant};

/// Fallback when the error carries no message (TS `:75-78` unknown-error branch).
pub const UNKNOWN_ERROR_MESSAGE: &str = "An unknown error has occurred";
/// `useToast` outside a provider (TS `:99`).
pub const OUTSIDE_PROVIDER: &str = "useToast must be used within a ToastProvider";

/// Box position (TS `:27-28` `top={2} right={2}`).
pub struct Position;

impl Position {
    pub const TOP: u32 = 2;
    pub const RIGHT: u32 = 2;
}

/// TS `error(err)` `:69-79`: Error.message else unknown-error fallback.
/// Empty string stands in for non-Error input (no `instanceof` in Rust).
#[must_use]
pub fn error_message(err: String) -> String {
    if err.is_empty() { UNKNOWN_ERROR_MESSAGE.to_string() } else { err }
}

/// Border theme key (TS `:35` `borderColor={theme[current().variant]}`).
#[must_use]
pub const fn border_color(variant: ToastVariant) -> &'static str {
    variant.as_str()
}

/// Single-slot holder mirroring TS `currentToast` (replace-on-new).
#[derive(Debug, Default, Clone)]
pub struct ToastProvider {
    pub current: Option<ToastOptions>,
}

impl ToastProvider {
    #[must_use]
    pub fn new() -> Self {
        Self { current: None }
    }

    /// TS `show`: replace current (TS also resets the dismiss timeout; the
    /// host drives expiry via `effective_duration_ms`).
    pub fn show(&mut self, toast: ToastOptions) {
        self.current = Some(toast);
    }

    /// TS `error(err)`: error-variant toast via [`error_message`].
    pub fn show_error(&mut self, err: String) {
        let message = error_message(err);
        if let Ok(t) = ToastOptions::new(None, &message, ToastVariant::Error, None) {
            self.show(t);
        }
    }

    pub fn clear(&mut self) {
        self.current = None;
    }

    #[must_use]
    pub const fn current(&self) -> Option<&ToastOptions> {
        self.current.as_ref()
    }
}

/// TS `useToast` `:96-102`: fail-closed outside a provider.
pub fn use_toast(provider: Option<&ToastProvider>) -> Result<&ToastProvider, &'static str> {
    provider.ok_or(OUTSIDE_PROVIDER)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(msg: &str) -> ToastOptions {
        ToastOptions::new(None, msg, ToastVariant::Info, None).unwrap()
    }

    #[test]
    fn error_message_passthrough() {
        assert_eq!(error_message("boom".to_string()), "boom");
    }

    #[test]
    fn error_message_empty_is_unknown() {
        assert_eq!(error_message(String::new()), UNKNOWN_ERROR_MESSAGE);
    }

    #[test]
    fn border_color_matches_variant_key() {
        assert_eq!(border_color(ToastVariant::Info), "info");
        assert_eq!(border_color(ToastVariant::Success), "success");
        assert_eq!(border_color(ToastVariant::Warning), "warning");
        assert_eq!(border_color(ToastVariant::Error), "error");
    }

    #[test]
    fn position_consts() {
        assert_eq!(Position::TOP, 2);
        assert_eq!(Position::RIGHT, 2);
    }

    #[test]
    fn provider_replaces_on_new() {
        let mut p = ToastProvider::new();
        p.show(info("first"));
        p.show(info("second"));
        assert_eq!(p.current().unwrap().message, "second");
        p.clear();
        assert!(p.current().is_none());
    }

    #[test]
    fn show_error_sets_error_variant() {
        let mut p = ToastProvider::new();
        p.show_error(String::new());
        let c = p.current().unwrap();
        assert_eq!(c.variant, ToastVariant::Error);
        assert_eq!(c.message, UNKNOWN_ERROR_MESSAGE);
    }

    #[test]
    fn use_toast_requires_provider() {
        assert_eq!(use_toast(None).unwrap_err(), OUTSIDE_PROVIDER);
        let p = ToastProvider::new();
        assert!(use_toast(Some(&p)).is_ok());
    }
}
