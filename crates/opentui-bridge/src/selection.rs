#![forbid(unsafe_code)]
//! Selection copy semantics (mirrors
//! `packages/tui/src/util/selection.ts:1-79`, TS checkout a0d9b6c).
//!
//! TS `copy(renderer, toast, clipboard)`: empty selection -> `false` (and
//! ctrl-c path clears); focused renderable in `selectedRenderables` may
//! rewrite via `getClipboardText`; success writes to clipboard, toasts
//! "Copied to clipboard", clears selection, returns `true`.

/// Max selectable text bytes (fail-closed bound; TS unbounded string).
pub const MAX_SELECTION_BYTES: usize = 1024 * 1024;

/// Rewrite hook for the focused target (mirrors `getClipboardText?:`).
pub trait ClipboardTextProvider {
    fn transform(&self, text: &str) -> String;
}

/// Frozen selection inputs for one copy attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionSnapshot {
    /// `renderer.getSelection()` non-null.
    pub has_selection: bool,
    /// `selection.getSelectedText()`; empty -> no copy (TS `if (!text)`).
    pub text: Option<String>,
    /// Focused renderable participates (mirrors `selectedRenderables.includes(focus)`).
    pub focus_in_selection: bool,
}

impl SelectionSnapshot {
    #[must_use]
    pub const fn empty() -> Self {
        Self { has_selection: false, text: None, focus_in_selection: false }
    }
}

/// Fail-closed copy errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionError {
    NoSelection,
    EmptyText,
    TooLarge,
}

impl core::fmt::Display for SelectionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoSelection => write!(f, "no selection"),
            Self::EmptyText => write!(f, "empty selection text"),
            Self::TooLarge => write!(f, "selection exceeds 1MB"),
        }
    }
}

impl std::error::Error for SelectionError {}

/// Copy outcome: clipboard payload plus whether caller must clear selection.
/// TS clears on both success and failed ctrl-c; escape always clears, so
/// `clear` is true on `Ok` and on `NoSelection`/`EmptyText`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyOutcome {
    pub clipboard_text: String,
    pub clear: bool,
}

/// Mirror of TS `copy`: validate, apply focus transform, bound size.
/// `provider` is `Some` only when focus is in selection and exposes
/// `getClipboardText` (TS `focus?.getClipboardText && includes(focus)`).
pub fn copy_selection(
    snapshot: &SelectionSnapshot,
    provider: Option<&dyn ClipboardTextProvider>,
) -> Result<CopyOutcome, SelectionError> {
    if !snapshot.has_selection {
        return Err(SelectionError::NoSelection);
    }
    let text = snapshot.text.as_deref().unwrap_or("");
    if text.is_empty() {
        return Err(SelectionError::EmptyText);
    }
    let out = if snapshot.focus_in_selection {
        provider.map(|p| p.transform(text)).unwrap_or_else(|| text.to_string())
    } else {
        text.to_string()
    };
    if out.len() > MAX_SELECTION_BYTES {
        return Err(SelectionError::TooLarge);
    }
    if out.is_empty() {
        return Err(SelectionError::EmptyText);
    }
    Ok(CopyOutcome { clipboard_text: out, clear: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Upper;
    impl ClipboardTextProvider for Upper {
        fn transform(&self, text: &str) -> String {
            text.to_uppercase()
        }
    }

    fn snap(text: &str) -> SelectionSnapshot {
        SelectionSnapshot { has_selection: true, text: Some(text.to_string()), focus_in_selection: false }
    }

    #[test]
    fn no_selection_or_empty_fails() {
        assert_eq!(
            copy_selection(&SelectionSnapshot::empty(), None),
            Err(SelectionError::NoSelection)
        );
        assert_eq!(copy_selection(&snap(""), None), Err(SelectionError::EmptyText));
    }

    #[test]
    fn plain_copy_returns_text_and_clear() {
        let out = copy_selection(&snap("hi"), None).unwrap();
        assert_eq!(out, CopyOutcome { clipboard_text: "hi".to_string(), clear: true });
    }

    #[test]
    fn focus_transform_applies_only_in_selection() {
        let mut s = snap("hi");
        s.focus_in_selection = true;
        assert_eq!(copy_selection(&s, Some(&Upper)).unwrap().clipboard_text, "HI");
        s.focus_in_selection = false;
        assert_eq!(copy_selection(&s, Some(&Upper)).unwrap().clipboard_text, "hi");
    }

    #[test]
    fn oversize_and_emptied_by_transform_fail() {
        struct Blank;
        impl ClipboardTextProvider for Blank {
            fn transform(&self, _: &str) -> String {
                String::new()
            }
        }
        let big = "x".repeat(MAX_SELECTION_BYTES + 1);
        assert_eq!(copy_selection(&snap(&big), None), Err(SelectionError::TooLarge));
        let mut s = snap("hi");
        s.focus_in_selection = true;
        assert_eq!(copy_selection(&s, Some(&Blank)), Err(SelectionError::EmptyText));
    }
}
