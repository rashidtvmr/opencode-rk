#![forbid(unsafe_code)]
//! Managed textarea editor state.
//!
//! Mirrors `packages/tui/src/prompt/traits.ts:1` (`EditorTraits`),
//! `packages/tui/src/editor.ts:26` (`openEditor` input `{value, cwd}`),
//! `packages/tui/src/keymap.tsx:175-178` (`hasManagedTextareaFocus`).
//! Divergence note: TS checkout at a0d9b6c, not pinned 95daf90.
//! Pure, bounded, no IO/FFI. Cursor is a char offset, never a byte index.
//! ponytail: char (not grapheme) boundaries; upgrade: unicode-segmentation.

/// Byte cap for editor text (matches 64 KiB draw cap).
pub const MAX_EDITOR_BYTES: usize = 65536;

/// TS `EditorTraits` subset: multiline + readonly flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorTraits {
    pub multiline: bool,
    pub readonly: bool,
}

/// Single-line (`InputRenderable`) vs multi-line (`TextareaRenderable`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorKind {
    SingleLine,
    MultiLine,
}

impl EditorKind {
    #[must_use]
    pub const fn traits(self, readonly: bool) -> EditorTraits {
        EditorTraits { multiline: matches!(self, Self::MultiLine), readonly }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorError {
    TextTooLarge,
    EmptyCwd,
    NewlineRejected,
    ReadOnly,
}

impl std::fmt::Display for EditorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::TextTooLarge => "editor text exceeds size limit",
            Self::EmptyCwd => "editor cwd is empty",
            Self::NewlineRejected => "single-line editor rejects newline",
            Self::ReadOnly => "editor is read-only",
        };
        f.write_str(s)
    }
}

impl std::error::Error for EditorError {}

/// `openEditor` input: initial `value` plus working dir (editor.ts:26).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenEditorRequest {
    pub value: String,
    pub cwd: String,
}

impl OpenEditorRequest {
    pub fn validate(&self) -> Result<(), EditorError> {
        if self.cwd.is_empty() {
            return Err(EditorError::EmptyCwd);
        }
        if self.value.len() > MAX_EDITOR_BYTES {
            return Err(EditorError::TextTooLarge);
        }
        Ok(())
    }
}

/// Bounded text plus char-offset cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorState {
    pub kind: EditorKind,
    pub readonly: bool,
    pub text: String,
    pub cursor: usize,
}

impl EditorState {
    #[must_use]
    pub const fn new(kind: EditorKind) -> Self {
        Self { kind, readonly: false, text: String::new(), cursor: 0 }
    }

    pub fn with_text(kind: EditorKind, text: &str) -> Result<Self, EditorError> {
        if text.len() > MAX_EDITOR_BYTES {
            return Err(EditorError::TextTooLarge);
        }
        if matches!(kind, EditorKind::SingleLine) && text.contains(['\n', '\r']) {
            return Err(EditorError::NewlineRejected);
        }
        let cursor = text.chars().count();
        Ok(Self { kind, readonly: false, text: text.to_string(), cursor })
    }

    #[must_use]
    pub fn len_chars(&self) -> usize {
        self.text.chars().count()
    }

    fn byte_offset(&self, cursor: usize) -> usize {
        self.text.char_indices().nth(cursor).map_or(self.text.len(), |(b, _)| b)
    }

    fn check_write(&self, extra: usize) -> Result<(), EditorError> {
        if self.readonly {
            return Err(EditorError::ReadOnly);
        }
        if self.text.len() + extra > MAX_EDITOR_BYTES {
            return Err(EditorError::TextTooLarge);
        }
        Ok(())
    }

    pub fn insert_str(&mut self, s: &str) -> Result<(), EditorError> {
        if matches!(self.kind, EditorKind::SingleLine) && s.contains(['\n', '\r']) {
            return Err(EditorError::NewlineRejected);
        }
        self.check_write(s.len())?;
        let at = self.byte_offset(self.cursor);
        self.text.insert_str(at, s);
        self.cursor += s.chars().count();
        Ok(())
    }

    pub fn backspace(&mut self) -> Result<(), EditorError> {
        if self.readonly {
            return Err(EditorError::ReadOnly);
        }
        if self.cursor == 0 {
            return Ok(());
        }
        let end = self.byte_offset(self.cursor);
        let start = self.text[..end].char_indices().last().map_or(0, |(b, _)| b);
        self.text.drain(start..end);
        self.cursor -= 1;
        Ok(())
    }

    pub fn delete_fwd(&mut self) -> Result<(), EditorError> {
        if self.readonly {
            return Err(EditorError::ReadOnly);
        }
        if self.cursor >= self.len_chars() {
            return Ok(());
        }
        let start = self.byte_offset(self.cursor);
        let end = self.text[start..].char_indices().nth(1).map_or(self.text.len(), |(b, _)| start + b);
        self.text.drain(start..end);
        Ok(())
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.len_chars());
    }

    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursor = cursor.min(self.len_chars());
    }
}

/// `hasManagedTextareaFocus` (keymap.tsx:175-178): a focused `TextareaRenderable`
/// that is not an `InputRenderable`, i.e. focused multi-line.
#[must_use]
pub const fn is_managed_textarea(kind: EditorKind, focused: bool) -> bool {
    focused && matches!(kind, EditorKind::MultiLine)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emoji_backspace_safe() {
        let mut e = EditorState::with_text(EditorKind::MultiLine, "a😀b").unwrap();
        e.set_cursor(2);
        e.backspace().unwrap();
        assert_eq!(e.text, "ab");
        assert_eq!(e.cursor, 1);
        assert!(e.text.is_char_boundary(e.byte_offset(e.cursor)));
    }

    #[test]
    fn single_line_rejects_newline() {
        let mut e = EditorState::new(EditorKind::SingleLine);
        assert_eq!(e.insert_str("hi\n"), Err(EditorError::NewlineRejected));
        assert_eq!(e.insert_str("hi\r"), Err(EditorError::NewlineRejected));
        assert_eq!(EditorState::with_text(EditorKind::SingleLine, "a\nb"), Err(EditorError::NewlineRejected));
        e.insert_str("ok").unwrap();
        assert_eq!(e.text, "ok");
    }

    #[test]
    fn cursor_clamps() {
        let mut e = EditorState::with_text(EditorKind::MultiLine, "ab").unwrap();
        e.set_cursor(99);
        assert_eq!(e.cursor, 2);
        e.move_right();
        assert_eq!(e.cursor, 2);
        e.set_cursor(0);
        e.move_left();
        assert_eq!(e.cursor, 0);
        e.delete_fwd().unwrap();
        assert_eq!(e.text, "b");
    }

    #[test]
    fn over_cap_errs() {
        let big = "x".repeat(MAX_EDITOR_BYTES + 1);
        assert_eq!(EditorState::with_text(EditorKind::MultiLine, &big), Err(EditorError::TextTooLarge));
        let mut e = EditorState::with_text(EditorKind::MultiLine, &"x".repeat(MAX_EDITOR_BYTES - 1)).unwrap();
        assert_eq!(e.insert_str("xx"), Err(EditorError::TextTooLarge));
    }

    #[test]
    fn empty_cwd_errs() {
        let r = OpenEditorRequest { value: "hi".into(), cwd: String::new() };
        assert_eq!(r.validate(), Err(EditorError::EmptyCwd));
        let r = OpenEditorRequest { value: "hi".into(), cwd: "/tmp".into() };
        assert_eq!(r.validate(), Ok(()));
    }

    #[test]
    fn focus_truth_table() {
        assert!(is_managed_textarea(EditorKind::MultiLine, true));
        assert!(!is_managed_textarea(EditorKind::MultiLine, false));
        assert!(!is_managed_textarea(EditorKind::SingleLine, true));
        assert!(!is_managed_textarea(EditorKind::SingleLine, false));
        assert!(EditorKind::MultiLine.traits(false).multiline);
        assert!(!EditorKind::SingleLine.traits(false).multiline);
    }
}
