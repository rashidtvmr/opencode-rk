#![forbid(unsafe_code)]
//! Toast notification (mirrors `packages/tui/src/ui/toast.tsx`).
//!
//! TS `ToastOptions { title?, message, variant, duration }`; `ToastInput`
//! omits duration (defaults 5000ms). Width clamp
//! `Math.min(60, dimensions().width - 6)` mirrored by [`toast_width`].

/// Max queued toasts (queue drops oldest on overflow).
pub const MAX_TOASTS: usize = 8;
/// Max toast message chars.
pub const MAX_TOAST: usize = 1024;
/// Max toast title chars.
pub const MAX_TITLE: usize = 128;
/// Default duration when `duration_ms` is `None` (TS `?? 5000`).
pub const DEFAULT_DURATION_MS: u64 = 5000;

/// TS `variant: "info" | "success" | "warning" | "error"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastVariant {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastVariant {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    /// Fail-closed `None` on unknown variant.
    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "info" => Some(Self::Info),
            "success" => Some(Self::Success),
            "warning" => Some(Self::Warning),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

/// Validated toast (`title?` mirrors optional TS title).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastOptions {
    pub title: Option<String>,
    pub message: String,
    pub variant: ToastVariant,
    pub duration_ms: Option<u64>,
}

impl ToastOptions {
    pub fn new(title: Option<&str>, message: &str, variant: ToastVariant, duration_ms: Option<u64>) -> Result<Self, &'static str> {
        match title {
            Some(t) if t.chars().count() > MAX_TITLE => return Err("title too long"),
            _ => {}
        }
        if message.is_empty() {
            return Err("message empty");
        }
        if message.chars().count() > MAX_TOAST {
            return Err("message too long");
        }
        Ok(Self {
            title: title.map(str::to_string),
            message: message.to_string(),
            variant,
            duration_ms,
        })
    }

    /// Effective duration (TS `duration ?? 5000`).
    #[must_use]
    pub const fn effective_duration_ms(&self) -> u64 {
        match self.duration_ms {
            Some(d) => d,
            None => DEFAULT_DURATION_MS,
        }
    }
}

/// Bounded FIFO; `push` drops oldest when full.
#[derive(Debug, Default, Clone)]
pub struct ToastQueue {
    items: Vec<ToastOptions>,
}

impl ToastQueue {
    #[must_use]
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn push(&mut self, toast: ToastOptions) {
        if self.items.len() >= MAX_TOASTS {
            self.items.remove(0);
        }
        self.items.push(toast);
    }

    /// Pop oldest (current) toast.
    pub fn pop_current(&mut self) -> Option<ToastOptions> {
        if self.items.is_empty() {
            return None;
        }
        Some(self.items.remove(0))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// TS `Math.min(60, width - 6)`; saturates on narrow terminals.
#[must_use]
pub const fn toast_width(term_width: u32) -> u32 {
    let inner = term_width.saturating_sub(6);
    if inner < 60 { inner } else { 60 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_clamp() {
        assert_eq!(toast_width(100), 60);
        assert_eq!(toast_width(66), 60);
        assert_eq!(toast_width(40), 34);
        assert_eq!(toast_width(6), 0);
        assert_eq!(toast_width(0), 0);
    }

    #[test]
    fn queue_overflow_drops_oldest() {
        let mut q = ToastQueue::new();
        for i in 0..MAX_TOASTS + 2 {
            q.push(ToastOptions::new(None, &format!("m{i}"), ToastVariant::Info, None).unwrap());
        }
        assert_eq!(q.len(), MAX_TOASTS);
        assert_eq!(q.pop_current().unwrap().message, "m2");
    }

    #[test]
    fn empty_and_oversize_message_err() {
        assert!(ToastOptions::new(None, "", ToastVariant::Error, None).is_err());
        assert!(ToastOptions::new(Some("t"), "x", ToastVariant::Info, None).is_ok());
        assert!(ToastOptions::new(None, &"y".repeat(MAX_TOAST + 1), ToastVariant::Info, None).is_err());
        assert!(ToastOptions::new(Some(&"t".repeat(MAX_TITLE + 1)), "x", ToastVariant::Info, None).is_err());
    }

    #[test]
    fn variant_roundtrip_and_default_duration() {
        for v in [ToastVariant::Info, ToastVariant::Success, ToastVariant::Warning, ToastVariant::Error] {
            assert_eq!(ToastVariant::from_str(v.as_str()), Some(v));
        }
        assert_eq!(ToastVariant::from_str("nope"), None);
        let t = ToastOptions::new(None, "m", ToastVariant::Success, None).unwrap();
        assert_eq!(t.effective_duration_ms(), DEFAULT_DURATION_MS);
        let t2 = ToastOptions::new(None, "m", ToastVariant::Success, Some(100)).unwrap();
        assert_eq!(t2.effective_duration_ms(), 100);
    }
}
