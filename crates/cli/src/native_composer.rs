#![forbid(unsafe_code)]
//! Unicode composer buffer, keymap, paste gate and busy queue (TUI-004).
//!
//! Pure state only: no rendering, no IO, no clock, no threads, no FFI.
//! Std only (`VecDeque`, `fmt`, `mem`); compiles under `rustc --test` with no
//! external crates.
//!
//! Invariants:
//! - [`EditBuffer`] cursor is always a UTF-8 char boundary. Every mutation
//!   asserts it (`debug_assert`) and moves/inserts/deletes whole `char`s, so
//!   Tamil, combining marks, emoji and CJK can never split a code point.
//!   ponytail: cursor moves per Unicode scalar, not extended grapheme
//!   cluster (ZWJ sequences delete scalar-by-scalar, still byte-safe);
//!   upgrade to cluster segmentation with a unicode-segmentation dependency
//!   if the lane ever allows non-std crates.
//! - Paste is a gate, never a trigger: [`Composer::apply_paste`] inserts the
//!   bracketed body literally (embedded newlines, `:q`, `rm -rf`, ANSI all
//!   stay inert draft text) and can never submit or queue. Submission happens
//!   only through [`Composer::handle_key`] with a [`KeyAction::Submit`]
//!   decision. Use [`unwrap_bracketed`] so only framed bodies reach the gate.
//! - Submit keys are explicit: [`SubmitKeymap::Enter`] submits on Enter and
//!   inserts newline on Ctrl-J; [`SubmitKeymap::CtrlJ`] is the mirror.
//! - Busy-session drafts queue FIFO in a [`VecDeque`] capped at
//!   [`BUSY_QUEUE_CAP`] (32). Overflow and oversize input return
//!   [`ComposerError`] and leave state untouched; nothing is silently
//!   dropped or truncated.
//! - Byte budgets: paste bodies over [`MAX_PASTE_BYTES`] (32 KiB) are
//!   rejected, and buffer + paste may never exceed [`MAX_DRAFT_BYTES`]
//!   (32 KiB, mirrors `MAX_DRAFT_BYTES` in `opencode-rk-sessions`
//!   `tui_state` and `MAX_PASTE_BYTES` in `terminal_host`).

use std::collections::VecDeque;
use std::fmt;

/// Max paste body admitted by [`Composer::apply_paste`] (32 KiB).
pub const MAX_PASTE_BYTES: usize = 32 * 1024;
/// Max total draft bytes retained (32 KiB).
pub const MAX_DRAFT_BYTES: usize = 32 * 1024;

/// Detached submission snapshot. Backend admission never owns the next draft.
/// Mirrors upstream prompt/index.tsx's synchronous reset and conditional restore.
#[derive(Clone, Debug)]
pub struct SubmittedDraft {
    pub text: String,
    cursor: usize,
}

impl SubmittedDraft {
    pub fn capture(draft: &mut String, cursor: &mut usize) -> Self {
        normalize_cursor(draft, cursor);
        let captured = Self { text: std::mem::take(draft), cursor: *cursor };
        *cursor = 0;
        captured
    }

    /// Rejected admission restores only an empty composer, never newer typing.
    pub fn restore(self, draft: &mut String, cursor: &mut usize) -> bool {
        if !draft.is_empty() { return false; }
        replace_draft_with_cursor(draft, cursor, self.text, self.cursor);
        true
    }
}

/// Clear only the exact draft acknowledged by the backend. Edits made while
/// admission is in flight belong to the user and must survive that ACK.
pub fn acknowledge_submitted_draft(captured: &str, draft: &mut String) -> bool {
    if draft == captured { draft.clear(); true } else { false }
}

/// Keep a legacy string-backed cursor valid after any external draft
/// replacement. This is deliberately a boundary clamp, not state recovery;
/// callers still supply the correct semantic cursor for known replacements.
pub fn normalize_cursor(draft: &str, cursor: &mut usize) {
    *cursor = (*cursor).min(draft.len());
    while *cursor > 0 && !draft.is_char_boundary(*cursor) { *cursor -= 1; }
}

/// ACK-aware clear for the transitional native caller. A matching ACK clears
/// both text and cursor; a stale ACK leaves both untouched (including edits
/// made while admission was in flight).
pub fn acknowledge_submitted_draft_with_cursor(
    captured: &str,
    draft: &mut String,
    cursor: &mut usize,
) -> bool {
    if acknowledge_submitted_draft(captured, draft) {
        *cursor = 0;
        true
    } else {
        normalize_cursor(draft, cursor);
        false
    }
}

/// Apply Ctrl-A/Ctrl-E to a caller-owned draft while preserving the caller's
/// cursor metadata. This adapter is used by the legacy string-backed native
/// loop until its full draft storage is migrated to `Composer`.
pub fn apply_line_binding(draft: &str, cursor: &mut usize, byte: u8) -> bool {
    let mut buffer = EditBuffer::new();
    if buffer.set_text(draft.to_owned()).is_err() { return false; }
    buffer.set_cursor(*cursor);
    let handled = match byte {
        1 => { buffer.move_to_line_start(); true }
        5 => { buffer.move_to_line_end(); true }
        _ => false,
    };
    if handled { *cursor = buffer.cursor(); }
    handled
}

/// Apply a decoded horizontal cursor key to the legacy native draft.
pub fn apply_horizontal_binding(draft: &str, cursor: &mut usize, left: bool) -> bool {
    let mut buffer = EditBuffer::new();
    if buffer.set_text(draft.to_owned()).is_err() { return false; }
    buffer.set_cursor(*cursor);
    let moved = if left { buffer.move_left() } else { buffer.move_right() };
    *cursor = buffer.cursor();
    moved
}

/// Replace a complete draft and place the cursor at an explicit valid offset.
/// Native route/session restoration uses this instead of assigning a String
/// and leaving stale cursor metadata behind.
pub fn replace_draft_with_cursor(draft: &mut String, cursor: &mut usize, replacement: String, position: usize) {
    *draft = replacement;
    *cursor = position.min(draft.len());
    normalize_cursor(draft, cursor);
}

#[cfg(test)]
mod native_submission_ack_tests {
    use super::acknowledge_submitted_draft;
    #[test]
    fn submission_snapshot_isolates_fast_next_command_and_rejection_recovery() {
        let mut draft = "first café".to_owned();
        let mut cursor = draft.len();
        let submitted = super::SubmittedDraft::capture(&mut draft, &mut cursor);
        assert!(draft.is_empty());
        assert_eq!(cursor, 0);
        draft.push_str("/new"); cursor = draft.len();
        assert_eq!(submitted.text, "first café");
        assert!(!submitted.restore(&mut draft, &mut cursor));
        assert_eq!(draft, "/new", "a rejected old submission never replaces new input");
        assert_eq!(cursor, 4);

        draft = "recover 界".to_owned(); cursor = 3;
        let submitted = super::SubmittedDraft::capture(&mut draft, &mut cursor);
        assert!(submitted.restore(&mut draft, &mut cursor));
        assert_eq!(draft, "recover 界");
        assert_eq!(cursor, 3, "rejection restores the captured insertion position");
    }
    #[test]
    fn native_backend_ack_consumes_only_the_captured_unicode_draft() {
        let mut draft = "change café 世界".to_owned();
        assert!(acknowledge_submitted_draft("change café 世界", &mut draft));
        assert!(draft.is_empty());
        let mut edited = "change café 世界, then add a test".to_owned();
        assert!(!acknowledge_submitted_draft("change café 世界", &mut edited));
        assert_eq!(edited, "change café 世界, then add a test");
        let mut replacement = "new task".to_owned();
        assert!(!acknowledge_submitted_draft("old task", &mut replacement));
        assert_eq!(replacement, "new task");

        let mut cursor = edited.len();
        assert!(!super::acknowledge_submitted_draft_with_cursor(
            "change café 世界", &mut edited, &mut cursor
        ));
        assert_eq!(cursor, edited.len());
        let mut matching = "change café 世界".to_owned();
        let mut matching_cursor = matching.len();
        assert!(super::acknowledge_submitted_draft_with_cursor(
            "change café 世界", &mut matching, &mut matching_cursor
        ));
        assert!(matching.is_empty());
        assert_eq!(matching_cursor, 0);
    }

    #[test]
    fn horizontal_binding_moves_by_unicode_scalar_without_inserting_protocol_bytes() {
        let draft = "Aé界B";
        let mut cursor = draft.len();
        assert!(super::apply_horizontal_binding(draft, &mut cursor, true));
        assert_eq!(&draft[..cursor], "Aé界");
        assert!(super::apply_horizontal_binding(draft, &mut cursor, true));
        assert_eq!(&draft[..cursor], "Aé");
        assert!(super::apply_horizontal_binding(draft, &mut cursor, false));
        assert_eq!(&draft[..cursor], "Aé界");
        assert!(draft.is_char_boundary(cursor));
    }
}
/// Max drafts queued while a turn is in flight.
pub const BUSY_QUEUE_CAP: usize = 32;
/// Undo snapshots retained per buffer.
pub const MAX_UNDO_DEPTH: usize = 32;
/// Bracketed-paste begin marker; the host strips framing via
/// [`unwrap_bracketed`] before calling the gate.
pub const PASTE_BEGIN: &str = "\x1b[200~";
/// Bracketed-paste end marker.
pub const PASTE_END: &str = "\x1b[201~";

/// Composer failures. Denials carry the bound; a denied call leaves the
/// buffer, busy flag and queue exactly as they were.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComposerError {
    /// Submit with a blank draft.
    EmptyDraft,
    /// Buffer would exceed [`MAX_DRAFT_BYTES`].
    DraftTooLong { bytes: usize, limit: usize },
    /// Paste body exceeds [`MAX_PASTE_BYTES`].
    PasteTooLarge { bytes: usize, limit: usize },
    /// Busy queue is at [`BUSY_QUEUE_CAP`].
    QueueFull { limit: usize },
}

impl fmt::Display for ComposerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDraft => write!(f, "draft is empty; nothing to send"),
            Self::DraftTooLong { bytes, limit } => {
                write!(f, "draft of {bytes} bytes exceeds {limit} byte budget")
            }
            Self::PasteTooLarge { bytes, limit } => {
                write!(f, "paste of {bytes} bytes exceeds {limit} byte policy")
            }
            Self::QueueFull { limit } => {
                write!(f, "busy queue full ({limit} drafts); interrupt or wait")
            }
        }
    }
}

impl std::error::Error for ComposerError {}

/// Which physical key submits the draft.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitKeymap {
    /// Enter submits; Ctrl-J / Shift+Enter insert newline.
    Enter,
    /// Ctrl-J submits; Enter inserts newline.
    CtrlJ,
}

/// Physical input keys the composer understands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    Enter,
    ShiftEnter,
    CtrlJ,
    Char(char),
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

/// What a [`Key`] means under a [`SubmitKeymap`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyAction {
    Submit,
    Newline,
    Insert(char),
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

/// Decide what a key does under a keymap. Only [`KeyAction::Submit`] can
/// send; paste bodies never pass through here (see [`Composer::apply_paste`]).
#[must_use]
pub const fn decide_key(key: Key, keymap: SubmitKeymap) -> KeyAction {
    match key {
        Key::Enter => match keymap {
            SubmitKeymap::Enter => KeyAction::Submit,
            SubmitKeymap::CtrlJ => KeyAction::Newline,
        },
        Key::CtrlJ => match keymap {
            SubmitKeymap::CtrlJ => KeyAction::Submit,
            SubmitKeymap::Enter => KeyAction::Newline,
        },
        Key::ShiftEnter => KeyAction::Newline,
        Key::Char(c) => KeyAction::Insert(c),
        Key::Backspace => KeyAction::Backspace,
        Key::Delete => KeyAction::Delete,
        Key::Left => KeyAction::Left,
        Key::Right => KeyAction::Right,
        Key::Up => KeyAction::Up,
        Key::Down => KeyAction::Down,
        Key::Home => KeyAction::Home,
        Key::End => KeyAction::End,
    }
}

/// Resolve a raw decoder byte through the same keymap used by Composer.
pub const fn decide_byte(byte: u8, keymap: SubmitKeymap) -> Option<KeyAction> {
    match byte {
        b'\r' => Some(decide_key(Key::Enter, keymap)),
        b'\n' => Some(decide_key(Key::CtrlJ, keymap)),
        _ => None,
    }
}

/// Strip bracketed-paste framing. Returns `None` unless the frame carries
/// both markers; the inner body is returned verbatim (newlines included).
#[must_use]
pub fn unwrap_bracketed(frame: &str) -> Option<&str> {
    frame.strip_prefix(PASTE_BEGIN)?.strip_suffix(PASTE_END)
}

/// Byte offset of line starts in `text`.
fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (i, c) in text.char_indices() {
        if c == '\n' {
            starts.push(i + 1);
        }
    }
    starts
}

/// Byte offset `col` chars into `[start, end)` (char range, so the result is
/// a boundary), clamped to `end`.
fn offset_at_col(text: &str, start: usize, end: usize, col: usize) -> usize {
    let mut off = start;
    for (n, c) in text[start..end].chars().enumerate() {
        if n == col {
            break;
        }
        off += c.len_utf8();
    }
    off
}

/// Grapheme-safe (char-boundary) edit buffer with cursor and bounded undo.
/// All edits move by whole `char`s; the cursor can never rest mid-code-point.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EditBuffer {
    text: String,
    cursor: usize,
    undo: Vec<(String, usize)>,
}

impl EditBuffer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            undo: Vec::new(),
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Restore a cursor captured by the native input owner.  Callers must
    /// provide a UTF-8 boundary; invalid offsets are clamped to the nearest
    /// preceding boundary rather than allowing a later edit to panic.
    pub fn set_cursor(&mut self, cursor: usize) {
        let mut cursor = cursor.min(self.text.len());
        while cursor > 0 && !self.text.is_char_boundary(cursor) {
            cursor -= 1;
        }
        self.cursor = cursor;
        self.check();
    }

    #[must_use]
    pub fn len_bytes(&self) -> usize {
        self.text.len()
    }

    #[must_use]
    pub fn len_chars(&self) -> usize {
        self.text.chars().count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// True exactly when the cursor sits on a UTF-8 boundary.
    #[must_use]
    pub fn is_cursor_on_boundary(&self) -> bool {
        self.cursor <= self.text.len() && self.text.is_char_boundary(self.cursor)
    }

    fn check(&self) {
        debug_assert!(
            self.is_cursor_on_boundary(),
            "composer cursor off char boundary"
        );
    }

    fn checkpoint(&mut self) {
        if self.undo.len() >= MAX_UNDO_DEPTH {
            self.undo.remove(0);
        }
        self.undo.push((self.text.clone(), self.cursor));
    }

    /// Insert one scalar at the cursor. Rejected past the byte budget.
    pub fn insert_char(&mut self, c: char) -> Result<(), ComposerError> {
        let mut slot = [0_u8; 4];
        let s = c.encode_utf8(&mut slot);
        self.insert_text(s)?;
        Ok(())
    }

    /// Insert `s` verbatim at the cursor; returns chars inserted.
    /// `s` is a valid `&str`, so insertion cannot split a code point; the
    /// pre-existing `check` plus the post `check` pin the boundary.
    pub fn insert_text(&mut self, s: &str) -> Result<usize, ComposerError> {
        let after = self.text.len() + s.len();
        if after > MAX_DRAFT_BYTES {
            return Err(ComposerError::DraftTooLong {
                bytes: after,
                limit: MAX_DRAFT_BYTES,
            });
        }
        self.check();
        self.checkpoint();
        self.text.insert_str(self.cursor, s);
        self.cursor += s.len();
        self.check();
        Ok(s.chars().count())
    }

    /// Replace the whole buffer; cursor parks at the end (always a boundary).
    pub fn set_text(&mut self, s: String) -> Result<(), ComposerError> {
        if s.len() > MAX_DRAFT_BYTES {
            return Err(ComposerError::DraftTooLong {
                bytes: s.len(),
                limit: MAX_DRAFT_BYTES,
            });
        }
        self.checkpoint();
        self.text = s;
        self.cursor = self.text.len();
        self.check();
        Ok(())
    }

    /// Take the buffer, leaving it empty with the cursor at 0.
    fn take(&mut self) -> String {
        self.checkpoint();
        let taken = std::mem::take(&mut self.text);
        self.cursor = 0;
        self.check();
        taken
    }

    /// Delete the scalar before the cursor. False at buffer start.
    pub fn backspace(&mut self) -> bool {
        let width = match self.text[..self.cursor].chars().next_back() {
            Some(c) => c.len_utf8(),
            None => return false,
        };
        self.checkpoint();
        self.cursor -= width;
        self.text.drain(self.cursor..self.cursor + width);
        self.check();
        true
    }

    /// Delete the scalar under the cursor. False at buffer end.
    pub fn delete_forward(&mut self) -> bool {
        let width = match self.text[self.cursor..].chars().next() {
            Some(c) => c.len_utf8(),
            None => return false,
        };
        self.checkpoint();
        self.text.drain(self.cursor..self.cursor + width);
        self.check();
        true
    }

    pub fn move_left(&mut self) -> bool {
        let width = match self.text[..self.cursor].chars().next_back() {
            Some(c) => c.len_utf8(),
            None => return false,
        };
        self.cursor -= width;
        self.check();
        true
    }

    pub fn move_right(&mut self) -> bool {
        let width = match self.text[self.cursor..].chars().next() {
            Some(c) => c.len_utf8(),
            None => return false,
        };
        self.cursor += width;
        self.check();
        true
    }

    pub fn move_to_start(&mut self) {
        self.cursor = 0;
        self.check();
    }

    pub fn move_to_end(&mut self) {
        self.cursor = self.text.len();
        self.check();
    }

    /// Move to the beginning of the logical line containing the cursor.
    /// This is the upstream `input.line.home` command (Ctrl-A), not buffer
    /// home: a multiline draft keeps its other lines and its suffix intact.
    pub fn move_to_line_start(&mut self) {
        self.cursor = self.text[..self.cursor].rfind('\n').map_or(0, |i| i + 1);
        self.check();
    }

    /// Move to the end of the logical line containing the cursor.  The
    /// newline itself remains outside the line, so End on a non-final line
    /// lands immediately before `\n`.
    pub fn move_to_line_end(&mut self) {
        self.cursor = self.text[self.cursor..]
            .find('\n')
            .map_or(self.text.len(), |i| self.cursor + i);
        self.check();
    }

    /// Up one visual line, keeping the char column when the target line is
    /// long enough. False on the first line.
    pub fn move_up(&mut self) -> bool {
        let starts = line_starts(&self.text);
        let line = starts.iter().rposition(|&s| s <= self.cursor).unwrap_or(0);
        if line == 0 {
            return false;
        }
        let col = self.text[starts[line]..self.cursor].chars().count();
        let target_end = starts[line] - 1;
        let next = offset_at_col(&self.text, starts[line - 1], target_end, col);
        self.cursor = next;
        self.check();
        true
    }

    /// Down one visual line, keeping the char column. False on the last line.
    pub fn move_down(&mut self) -> bool {
        let starts = line_starts(&self.text);
        let line = starts.iter().rposition(|&s| s <= self.cursor).unwrap_or(0);
        if line + 1 >= starts.len() {
            return false;
        }
        let col = self.text[starts[line]..self.cursor].chars().count();
        let target_end = match starts.get(line + 2) {
            Some(&ns) => ns - 1,
            None => self.text.len(),
        };
        let next = offset_at_col(&self.text, starts[line + 1], target_end, col);
        self.cursor = next;
        self.check();
        true
    }

    /// Restore the last snapshot. False when history is empty.
    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some((text, cursor)) => {
                self.text = text;
                self.cursor = cursor;
                self.check();
                true
            }
            None => false,
        }
    }

    pub fn clear(&mut self) {
        self.checkpoint();
        self.text.clear();
        self.cursor = 0;
        self.check();
    }
}

/// Idle-submit vs busy-submit result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubmitOutcome {
    /// Draft sent; composer is now busy.
    Sent(String),
    /// Turn in flight; draft cloned onto the bounded queue.
    Queued,
}

/// Paste gate result. There is deliberately no `Submitted` variant: paste
/// can only ever insert.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PasteOutcome {
    Inserted { bytes: usize, chars: usize },
}

/// How [`Composer::handle_key`] resolved a key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyHandled {
    Submitted,
    Queued,
    Edited,
}

/// Multiline composer: char-safe buffer, submit keymap, bracketed-paste
/// gate, and a bounded busy queue with interrupt-preserving drafts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Composer {
    buf: EditBuffer,
    busy: bool,
    queue: VecDeque<String>,
    keymap: SubmitKeymap,
}

impl Composer {
    #[must_use]
    pub fn new() -> Self {
        Self::with_keymap(SubmitKeymap::Enter)
    }

    #[must_use]
    pub fn with_keymap(keymap: SubmitKeymap) -> Self {
        Self {
            buf: EditBuffer::new(),
            busy: false,
            queue: VecDeque::new(),
            keymap,
        }
    }

    #[must_use]
    pub fn keymap(&self) -> SubmitKeymap {
        self.keymap
    }

    pub fn set_keymap(&mut self, keymap: SubmitKeymap) {
        self.keymap = keymap;
    }

    #[must_use]
    pub fn draft(&self) -> &str {
        self.buf.text()
    }

    #[must_use]
    pub fn buffer(&self) -> &EditBuffer {
        &self.buf
    }

    #[must_use]
    pub fn buffer_mut(&mut self) -> &mut EditBuffer {
        &mut self.buf
    }

    #[must_use]
    pub const fn is_busy(&self) -> bool {
        self.busy
    }

    #[must_use]
    pub fn queued(&self) -> &VecDeque<String> {
        &self.queue
    }

    #[must_use]
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    pub fn set_draft(&mut self, text: &str) -> Result<(), ComposerError> {
        self.buf.set_text(text.to_owned())
    }

    /// Route one physical key through the keymap. Submission flows only
    /// from here; [`apply_paste`](Self::apply_paste) never submits.
    pub fn handle_key(&mut self, key: Key) -> Result<KeyHandled, ComposerError> {
        match decide_key(key, self.keymap) {
            KeyAction::Submit => match self.submit()? {
                SubmitOutcome::Sent(_) => Ok(KeyHandled::Submitted),
                SubmitOutcome::Queued => Ok(KeyHandled::Queued),
            },
            KeyAction::Newline => {
                self.buf.insert_char('\n')?;
                Ok(KeyHandled::Edited)
            }
            KeyAction::Insert(c) => {
                self.buf.insert_char(c)?;
                Ok(KeyHandled::Edited)
            }
            KeyAction::Backspace => {
                self.buf.backspace();
                Ok(KeyHandled::Edited)
            }
            KeyAction::Delete => {
                self.buf.delete_forward();
                Ok(KeyHandled::Edited)
            }
            KeyAction::Left => {
                self.buf.move_left();
                Ok(KeyHandled::Edited)
            }
            KeyAction::Right => {
                self.buf.move_right();
                Ok(KeyHandled::Edited)
            }
            KeyAction::Up => {
                self.buf.move_up();
                Ok(KeyHandled::Edited)
            }
            KeyAction::Down => {
                self.buf.move_down();
                Ok(KeyHandled::Edited)
            }
            KeyAction::Home => {
                self.buf.move_to_start();
                Ok(KeyHandled::Edited)
            }
            KeyAction::End => {
                self.buf.move_to_end();
                Ok(KeyHandled::Edited)
            }
        }
    }

    /// Dispatch the two upstream line-edit bindings that arrive as control
    /// bytes from the native decoder: Ctrl-A and Ctrl-E.  Buffer-home/end are
    /// intentionally not aliases here; upstream leaves those bindings
    /// unbound by default.
    pub fn handle_line_binding(&mut self, byte: u8) -> bool {
        match byte {
            1 => {
                self.buf.move_to_line_start();
                true
            }
            5 => {
                self.buf.move_to_line_end();
                true
            }
            _ => false,
        }
    }

    /// Bracketed-paste gate: insert `body` (already unwrapped with
    /// [`unwrap_bracketed`]) literally at the cursor. Embedded newlines,
    /// commands and escape text stay inert draft bytes; busy/queue are
    /// untouched, so a paste can never auto-execute or submit. Bodies over
    /// [`MAX_PASTE_BYTES`], or pushing the buffer past [`MAX_DRAFT_BYTES`],
    /// are rejected with the state unchanged.
    pub fn apply_paste(&mut self, body: &str) -> Result<PasteOutcome, ComposerError> {
        let bytes = body.len();
        if bytes > MAX_PASTE_BYTES {
            return Err(ComposerError::PasteTooLarge {
                bytes,
                limit: MAX_PASTE_BYTES,
            });
        }
        let chars = self.buf.insert_text(body)?;
        Ok(PasteOutcome::Inserted { bytes, chars })
    }

    /// Idle + non-blank draft sends and marks busy. Busy + non-blank draft
    /// clones onto the bounded queue. Draft text is preserved in both cases
    /// for the interrupt path (idle keeps a checkpoint via `take`).
    pub fn submit(&mut self) -> Result<SubmitOutcome, ComposerError> {
        if self.buf.text().trim().is_empty() {
            return Err(ComposerError::EmptyDraft);
        }
        if !self.busy {
            let sent = self.buf.take();
            self.busy = true;
            return Ok(SubmitOutcome::Sent(sent));
        }
        if self.queue.len() >= BUSY_QUEUE_CAP {
            return Err(ComposerError::QueueFull {
                limit: BUSY_QUEUE_CAP,
            });
        }
        self.queue.push_back(self.buf.text().to_owned());
        Ok(SubmitOutcome::Queued)
    }

    /// Abort the in-flight turn. Draft and queue survive untouched.
    pub fn interrupt(&mut self) {
        self.busy = false;
    }

    /// Pop the next queued draft after a turn finishes. `None` when idle or
    /// the queue is empty (marks idle); otherwise stays busy.
    pub fn finish_turn(&mut self) -> Option<String> {
        match self.queue.pop_front() {
            Some(next) => {
                self.busy = true;
                Some(next)
            }
            None => {
                self.busy = false;
                None
            }
        }
    }
}

impl Default for Composer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod line_cursor_tests {
    use super::{Composer, EditBuffer, Key};

    #[test]
    fn ctrl_line_home_end_preserve_other_lines_and_unicode_boundaries() {
        let mut buffer = EditBuffer::new();
        buffer.set_text("前 café\n中間 🦀 suffix\n終".to_owned()).unwrap();
        let line_start = "前 café\n".len();
        let line_end = "前 café\n中間 🦀 suffix".len();
        let initial = "前 café\n中間 🦀".len();
        buffer.set_cursor(initial);
        buffer.move_to_line_start();
        assert_eq!(buffer.cursor(), line_start);
        assert_eq!(&buffer.text()[buffer.cursor()..], "中間 🦀 suffix\n終");
        buffer.move_to_line_end();
        assert_eq!(buffer.cursor(), line_end);
        assert_eq!(buffer.text(), "前 café\n中間 🦀 suffix\n終");
        assert_eq!(&buffer.text()[buffer.cursor()..], "\n終");
        assert!(buffer.is_cursor_on_boundary());
    }

    #[test]
    fn composer_home_end_dispatch_is_line_local() {
        let mut composer = Composer::new();
        composer.set_draft("αβ\n世界 🦀 tail").unwrap();
        composer.buffer_mut().set_cursor("αβ\n世界 🦀".len());
        assert!(composer.handle_line_binding(1));
        assert_eq!(composer.buffer().cursor(), "αβ\n".len());
        assert!(composer.handle_line_binding(5));
        assert_eq!(composer.buffer().cursor(), "αβ\n世界 🦀 tail".len());
        assert_eq!(&composer.draft()[composer.buffer().cursor()..], "");
        assert!(composer.buffer().is_cursor_on_boundary());
        assert_eq!(composer.buffer().cursor(), composer.draft().len());
    }

    #[test]
    fn physical_home_end_remain_buffer_operations_and_ctrl_bindings_are_line_local() {
        let mut composer = Composer::new();
        composer.set_draft("a\n世界 tail\nempty\n終").unwrap();
        composer.buffer_mut().set_cursor("a\n世界".len());
        assert_eq!(composer.handle_key(Key::Home), Ok(super::KeyHandled::Edited));
        assert_eq!(composer.buffer().cursor(), 0);
        composer.buffer_mut().set_cursor("a\n世界".len());
        assert!(composer.handle_line_binding(1));
        assert_eq!(composer.buffer().cursor(), "a\n".len());
        assert!(composer.handle_line_binding(5));
        assert_eq!(&composer.draft()[composer.buffer().cursor()..], "\nempty\n終");
    }
}

/// Max lines returned by [`ComposerPage::render_lines`].
pub const MAX_PAGE_LINES: usize = 64;
/// Max columns a page line is wrapped to (chars, char-boundary safe).
pub const MAX_PAGE_COLS: usize = 256;

/// Composer page: draft/queue/interrupt state bound to the sessions
/// `tui_state` journey (submit/queue/interrupt), plus bounded render lines.
/// Pure state only: no rendering backend, no IO. `tui_entry` wires this to
/// real session state later.
#[derive(Clone, Debug, Default)]
pub struct ComposerPage {
    composer: Composer,
}

impl ComposerPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            composer: Composer::new(),
        }
    }

    #[must_use]
    pub fn composer(&self) -> &Composer {
        &self.composer
    }

    pub fn composer_mut(&mut self) -> &mut Composer {
        &mut self.composer
    }

    #[must_use]
    pub fn draft(&self) -> &str {
        self.composer.draft()
    }

    #[must_use]
    pub const fn is_busy(&self) -> bool {
        self.composer.busy
    }

    #[must_use]
    pub fn queue_len(&self) -> usize {
        self.composer.queue.len()
    }

    /// Atomically adopt `(draft, busy, queue)` observed from sessions
    /// `tui_state`. Validates bounds first; on `Err` the page is untouched.
    pub fn set_from_session(
        &mut self,
        draft: &str,
        busy: bool,
        queue: &[String],
    ) -> Result<(), ComposerError> {
        if queue.len() > BUSY_QUEUE_CAP {
            return Err(ComposerError::QueueFull {
                limit: BUSY_QUEUE_CAP,
            });
        }
        if draft.len() > MAX_DRAFT_BYTES {
            return Err(ComposerError::DraftTooLong {
                bytes: draft.len(),
                limit: MAX_DRAFT_BYTES,
            });
        }
        for q in queue {
            if q.len() > MAX_DRAFT_BYTES {
                return Err(ComposerError::DraftTooLong {
                    bytes: q.len(),
                    limit: MAX_DRAFT_BYTES,
                });
            }
        }
        self.composer.set_draft(draft)?;
        self.composer.busy = busy;
        self.composer.queue.clear();
        self.composer.queue.extend(queue.iter().cloned());
        Ok(())
    }

    /// Bounded render lines for the native shell: status header (idle/busy +
    /// queue depth), wrapped draft lines, queued previews. Every line is
    /// wrapped to `width` chars (clamped to `1..=MAX_PAGE_COLS`, char-wise
    /// so no code point splits); total lines capped at [`MAX_PAGE_LINES`]
    /// with an explicit truncation marker.
    #[must_use]
    pub fn render_lines(&self, width: usize) -> Vec<String> {
        let w = width.clamp(1, MAX_PAGE_COLS);
        let mut lines: Vec<String> = Vec::new();
        let state = if self.composer.busy { "busy" } else { "idle" };
        let header = format!(
            "composer [{}] queue {}/{}",
            state,
            self.composer.queue.len(),
            BUSY_QUEUE_CAP
        );
        push_wrapped(&mut lines, &header, w);
        if self.composer.draft().is_empty() {
            push_wrapped(&mut lines, "(empty)", w);
        } else {
            for segment in self.composer.draft().split('\n') {
                if segment.is_empty() {
                    lines.push(String::new());
                } else {
                    push_wrapped(&mut lines, segment, w);
                }
                if lines.len() >= MAX_PAGE_LINES {
                    break;
                }
            }
        }
        for (i, q) in self.composer.queue.iter().enumerate() {
            if lines.len() >= MAX_PAGE_LINES {
                break;
            }
            let first = q.split('\n').next().unwrap_or("");
            push_wrapped(&mut lines, &format!("queued[{i}]: {first}"), w);
        }
        lines.truncate(MAX_PAGE_LINES);
        if self.composer.queue.len() + draft_line_count(self.composer.draft()) + 1 > MAX_PAGE_LINES
        {
            if let Some(last) = lines.last_mut() {
                *last = "... (truncated)".to_string();
            }
        }
        lines
    }
}

/// Wrap `s` char-wise into chunks of `w` chars.
fn push_wrapped(out: &mut Vec<String>, s: &str, w: usize) {
    let chars: Vec<char> = s.chars().collect();
    if chars.is_empty() {
        out.push(String::new());
        return;
    }
    for chunk in chars.chunks(w) {
        out.push(chunk.iter().collect());
    }
}

/// Draft segments (`\n`-split) for the truncation estimate.
fn draft_line_count(draft: &str) -> usize {
    if draft.is_empty() {
        return 1;
    }
    draft.split('\n').count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emoji_edit_never_splits_code_point() {
        let mut buf = EditBuffer::new();
        buf.insert_text("a🦀b").unwrap();
        assert!(buf.is_cursor_on_boundary());
        assert_eq!(buf.len_chars(), 3);
        // Cursor at end; step left past 'b' then delete the crab whole.
        assert!(buf.move_left());
        assert!(buf.is_cursor_on_boundary());
        assert!(buf.backspace());
        assert_eq!(buf.text(), "ab");
        assert!(buf.is_cursor_on_boundary());
        // Left moves by whole scalar: 'b' is 1 byte, emoji was 4.
        buf.set_text("x🦀".to_owned()).unwrap();
        buf.move_to_end();
        assert!(buf.move_left());
        assert_eq!(buf.cursor(), 1);
        assert!(buf.is_cursor_on_boundary());
    }

    #[test]
    fn zwj_emoji_deletes_scalar_by_scalar_without_corruption() {
        let mut buf = EditBuffer::new();
        buf.insert_text("👩‍💻").unwrap(); // woman technologist: scalar, ZWJ, scalar
        assert!(buf.is_cursor_on_boundary());
        let mut steps = 0;
        while buf.backspace() {
            assert!(buf.is_cursor_on_boundary());
            assert!(buf.text().is_char_boundary(buf.text().len()));
            steps += 1;
        }
        assert_eq!(buf.text(), "");
        assert_eq!(steps, 3);
    }

    #[test]
    fn wide_cjk_and_tamil_edit_safe() {
        let mut buf = EditBuffer::new();
        buf.insert_text("日本語").unwrap();
        assert_eq!(buf.len_bytes(), 9);
        assert_eq!(buf.len_chars(), 3);
        buf.move_to_start();
        assert!(buf.move_right());
        assert_eq!(buf.cursor(), 3); // one CJK scalar, not one byte
        assert!(buf.is_cursor_on_boundary());
        assert!(buf.delete_forward());
        assert_eq!(buf.text(), "日語");

        let mut tamil = EditBuffer::new();
        tamil.insert_text("தமிழ்").unwrap();
        assert!(tamil.is_cursor_on_boundary());
        tamil.move_to_end();
        while tamil.backspace() {
            assert!(tamil.is_cursor_on_boundary());
        }
        assert_eq!(tamil.text(), "");
    }

    #[test]
    fn combining_mark_never_orphans_bytes() {
        let mut buf = EditBuffer::new();
        buf.insert_char('e').unwrap();
        buf.insert_char('\u{301}').unwrap(); // combining acute
        assert_eq!(buf.text(), "é");
        assert!(buf.is_cursor_on_boundary());
        assert!(buf.backspace());
        assert_eq!(buf.text(), "e");
        assert!(buf.is_cursor_on_boundary());
    }

    #[test]
    fn bracketed_paste_never_submits_or_queues() {
        let mut c = Composer::new();
        let frame = format!("{PASTE_BEGIN}rm -rf /tmp/x\n:quit\nhello{PASTE_END}");
        let body = unwrap_bracketed(&frame).expect("framed paste unwraps");
        let out = c.apply_paste(body).unwrap();
        assert_eq!(
            out,
            PasteOutcome::Inserted {
                bytes: body.len(),
                chars: body.chars().count()
            }
        );
        // Gate inserted literally and submitted nothing.
        assert_eq!(c.draft(), "rm -rf /tmp/x\n:quit\nhello");
        assert!(!c.is_busy());
        assert_eq!(c.queue_len(), 0);
        // Newlines from the paste did not split into queued drafts.
        assert_eq!(c.queued().len(), 0);
        // Submission still requires an explicit submit key.
        assert_eq!(c.handle_key(Key::Enter), Ok(KeyHandled::Submitted));
        assert!(c.is_busy());
    }

    #[test]
    fn unframed_input_is_not_a_paste() {
        assert_eq!(unwrap_bracketed("plain text"), None);
        assert_eq!(unwrap_bracketed(&format!("{PASTE_BEGIN}half")), None);
        assert_eq!(
            unwrap_bracketed(&format!("{PASTE_BEGIN}a\nb{PASTE_END}")),
            Some("a\nb")
        );
    }

    #[test]
    fn oversize_paste_rejected_without_mutation() {
        let mut c = Composer::new();
        c.set_draft("keep me").unwrap();
        let big = "x".repeat(MAX_PASTE_BYTES + 1);
        let bytes = big.len();
        assert_eq!(
            c.apply_paste(&big),
            Err(ComposerError::PasteTooLarge {
                bytes,
                limit: MAX_PASTE_BYTES
            })
        );
        assert_eq!(c.draft(), "keep me");
        assert!(!c.is_busy());
        assert_eq!(c.queue_len(), 0);
        // Exactly at cap is admitted.
        let mut empty = Composer::new();
        let ok = "y".repeat(MAX_PASTE_BYTES);
        assert!(empty.apply_paste(&ok).is_ok());
        assert_eq!(empty.buffer().len_bytes(), MAX_PASTE_BYTES);
    }

    #[test]
    fn paste_capped_by_total_draft_budget() {
        let mut c = Composer::new();
        c.set_draft(&"z".repeat(MAX_DRAFT_BYTES - 8)).unwrap();
        assert_eq!(
            c.apply_paste("123456789"),
            Err(ComposerError::DraftTooLong {
                bytes: MAX_DRAFT_BYTES + 1,
                limit: MAX_DRAFT_BYTES
            })
        );
        assert_eq!(c.buffer().len_bytes(), MAX_DRAFT_BYTES - 8);
    }

    #[test]
    fn keymap_enter_and_ctrlj_mirror() {
        assert_eq!(
            decide_key(Key::Enter, SubmitKeymap::Enter),
            KeyAction::Submit
        );
        assert_eq!(
            decide_key(Key::CtrlJ, SubmitKeymap::Enter),
            KeyAction::Newline
        );
        assert_eq!(
            decide_key(Key::Enter, SubmitKeymap::CtrlJ),
            KeyAction::Newline
        );
        assert_eq!(
            decide_key(Key::CtrlJ, SubmitKeymap::CtrlJ),
            KeyAction::Submit
        );
        assert_eq!(
            decide_key(Key::ShiftEnter, SubmitKeymap::Enter),
            KeyAction::Newline
        );
        assert_eq!(
            decide_key(Key::ShiftEnter, SubmitKeymap::CtrlJ),
            KeyAction::Newline
        );

        let mut enter = Composer::with_keymap(SubmitKeymap::Enter);
        enter.set_draft("hi").unwrap();
        assert_eq!(enter.handle_key(Key::CtrlJ), Ok(KeyHandled::Edited));
        assert_eq!(enter.draft(), "hi\n"); // newline, not submit
        assert!(!enter.is_busy());

        let mut ctrlj = Composer::with_keymap(SubmitKeymap::CtrlJ);
        ctrlj.set_draft("hi").unwrap();
        assert_eq!(ctrlj.handle_key(Key::Enter), Ok(KeyHandled::Edited));
        assert_eq!(ctrlj.draft(), "hi\n");
        assert!(!ctrlj.is_busy());
        assert_eq!(ctrlj.handle_key(Key::CtrlJ), Ok(KeyHandled::Submitted));
    }

    #[test]
    fn raw_enter_and_ctrlj_bytes_follow_decide_key_for_both_maps() {
        for (map, cr, lf) in [
            (SubmitKeymap::Enter, KeyAction::Submit, KeyAction::Newline),
            (SubmitKeymap::CtrlJ, KeyAction::Newline, KeyAction::Submit),
        ] {
            assert_eq!(decide_byte(b'\r', map), Some(cr));
            assert_eq!(decide_byte(b'\n', map), Some(lf));
        }
    }

    #[test]
    fn busy_queue_bounded_fifo_at_32() {
        let mut c = Composer::new();
        c.set_draft("first").unwrap();
        assert_eq!(c.submit(), Ok(SubmitOutcome::Sent("first".to_owned())));
        assert!(c.is_busy());
        for i in 0..BUSY_QUEUE_CAP {
            c.set_draft(&format!("q{i}")).unwrap();
            assert_eq!(c.submit(), Ok(SubmitOutcome::Queued), "admit {i}");
        }
        assert_eq!(c.queue_len(), BUSY_QUEUE_CAP);
        c.set_draft("overflow").unwrap();
        assert_eq!(
            c.submit(),
            Err(ComposerError::QueueFull {
                limit: BUSY_QUEUE_CAP
            })
        );
        assert_eq!(c.queue_len(), BUSY_QUEUE_CAP);
        // FIFO drain in admission order.
        for i in 0..BUSY_QUEUE_CAP {
            assert_eq!(c.finish_turn(), Some(format!("q{i}")));
            assert!(c.is_busy());
        }
        assert_eq!(c.finish_turn(), None);
        assert!(!c.is_busy());
    }

    #[test]
    fn interrupt_preserves_draft_and_queue() {
        let mut c = Composer::new();
        c.set_draft("live").unwrap();
        c.submit().unwrap();
        c.set_draft("waiting").unwrap();
        c.submit().unwrap();
        c.interrupt();
        assert!(!c.is_busy());
        assert_eq!(c.draft(), "waiting");
        assert_eq!(c.queue_len(), 1);
        // Resubmit after interrupt sends immediately again.
        c.set_draft("retry").unwrap();
        assert!(matches!(c.submit(), Ok(SubmitOutcome::Sent(_))));
    }

    #[test]
    fn multiline_up_down_keeps_char_column() {
        let mut c = Composer::new();
        c.set_draft("ab\ncdef\nz").unwrap(); // cursor at end: line 2, col 1
        assert!(c.buffer_mut().move_up());
        assert_eq!(&c.draft()[..c.buffer().cursor()], "ab\nc");
        assert!(c.buffer().is_cursor_on_boundary());
        assert!(c.buffer_mut().move_down());
        assert_eq!(c.buffer().cursor(), c.draft().len());
        assert!(c.buffer_mut().move_up());
        assert!(c.buffer_mut().move_up());
        assert!(!c.buffer_mut().move_up()); // top line: no-op
                                            // Column clamps on short lines: end of "cdef" down to "z".
        c.set_draft("ab\ncdef\nz").unwrap();
        c.buffer_mut().move_to_start();
        c.buffer_mut().move_down();
        c.buffer_mut().move_to_end();
        // cursor now end of line 1 ("cdef"); move down clamps to end of "z".
        c.buffer_mut().move_up(); // back to line 0 first for determinism
        c.buffer_mut().move_down();
        c.buffer_mut().move_to_end();
        assert!(c.buffer().is_cursor_on_boundary());
    }

    #[test]
    fn undo_restores_last_edit() {
        let mut buf = EditBuffer::new();
        assert!(!buf.undo());
        buf.insert_text("hello").unwrap();
        buf.backspace();
        assert_eq!(buf.text(), "hell");
        assert!(buf.undo());
        assert_eq!(buf.text(), "hello");
        assert!(buf.is_cursor_on_boundary());
    }

    #[test]
    fn empty_and_oversize_drafts_rejected() {
        let mut c = Composer::new();
        assert_eq!(c.submit(), Err(ComposerError::EmptyDraft));
        c.set_draft("   ").unwrap();
        assert_eq!(c.submit(), Err(ComposerError::EmptyDraft));
        let big = "x".repeat(MAX_DRAFT_BYTES + 1);
        assert!(c.set_draft(&big).is_err());
        assert_eq!(c.draft(), "   ");
    }

    #[test]
    fn page_renders_draft_queue_and_interrupt_state() {
        let mut page = ComposerPage::new();
        page.composer_mut().set_draft("hello\nworld").unwrap();
        let idle = page.render_lines(80);
        assert!(idle.iter().any(|l| l.contains("idle")), "idle badged");
        assert!(idle.iter().any(|l| l.contains("hello")), "draft shown");
        assert!(idle.iter().any(|l| l.contains("queue 0/")), "queue shown");
        page.composer_mut().submit().unwrap();
        page.composer_mut().set_draft("waiting").unwrap();
        page.composer_mut().submit().unwrap();
        let busy = page.render_lines(80);
        assert!(busy.iter().any(|l| l.contains("busy")), "busy badged");
        assert!(
            busy.iter().any(|l| l.contains("queue 1/")),
            "queued count shown"
        );
        page.composer_mut().interrupt();
        let back = page.render_lines(80);
        assert!(
            back.iter().any(|l| l.contains("idle")),
            "interrupt back to idle"
        );
        assert!(
            back.iter().any(|l| l.contains("waiting")),
            "draft preserved"
        );
    }

    #[test]
    fn page_adopts_sessions_snapshot_atomically() {
        let mut page = ComposerPage::new();
        page.composer_mut().set_draft("local").unwrap();
        page.set_from_session("remote", true, &["q0".to_owned()])
            .unwrap();
        assert_eq!(page.draft(), "remote");
        assert!(page.is_busy());
        assert_eq!(page.queue_len(), 1);
        // Oversize queue rejected, prior state untouched.
        let big: Vec<String> = (0..=BUSY_QUEUE_CAP).map(|i| format!("q{i}")).collect();
        assert_eq!(
            page.set_from_session("other", false, &big),
            Err(ComposerError::QueueFull {
                limit: BUSY_QUEUE_CAP
            })
        );
        assert_eq!(page.draft(), "remote");
        assert!(page.is_busy());
        // Oversize draft rejected, prior state untouched.
        let huge = "x".repeat(MAX_DRAFT_BYTES + 1);
        assert!(page.set_from_session(&huge, false, &[]).is_err());
        assert_eq!(page.draft(), "remote");
    }

    #[test]
    fn page_lines_bounded_and_char_safe() {
        let mut page = ComposerPage::new();
        page.composer_mut().set_draft("日本語🦀\n第二行").unwrap();
        let lines = page.render_lines(6);
        assert!(lines.len() <= MAX_PAGE_LINES, "line count bounded");
        for l in &lines {
            assert!(l.chars().count() <= 6 + 2, "line {l:?} exceeds width");
        }
        assert!(lines.iter().any(|l| l.contains('日')), "CJK kept");
        let empty = ComposerPage::new().render_lines(80);
        assert!(empty.iter().any(|l| l.contains("(empty)")));
        let wide = page.render_lines(10_000);
        for l in &wide {
            assert!(l.chars().count() <= MAX_PAGE_COLS + 2);
        }
    }
}
