#![forbid(unsafe_code)]
//! External-editor spawn plan + IDE socket pick + byte-offset mapping.
//!
//! Mirrors `packages/tui/src/editor.ts:26-54` (`openEditor`: `$VISUAL||$EDITOR`
//! split on space, spawn on tmp `<Date.now()>.md`, `renderer.suspend()` before
//! spawn / `resume()`+`requestRender()` after), `editor.ts:56-96`
//! (`discoverEditorConnection`: `~/.claude/ide/*.lock` parse, port range,
//! `transport=="ws"` gate, workspace-folder `contains` score, sort
//! `right.score-left.score || right.mtime-left.mtime`, take [0]),
//! `editor.ts:98-101` (`editorIntegration` facade), `editor-zed.ts:41-87`
//! (`resolveZedSelection` byte-range normalize+sort), `editor-zed.ts:223-243`
//! (`offsetToPosition` via `utf8ByteOffsetToStringIndex`), `editor-zed.ts:245-273`
//! (`offsetsToSelection`/`position`: 1-based line, 1-based `character`).
//! Divergence note: TS checkout at a0d9b6c, not pinned 95daf90.
//! Divergence: NO spawn/exec/sqlite in lib. This file is plan types only;
//! the host executes the plan (spawn, socket IO, sqlite query).
//! Reuse boundary: text-buffer ops stay in `crate::editor` (`OpenEditorRequest`);
//! zed terminal probe + `to_open_request` stay in `crate::editor_zed`.
//! ponytail: `character` counts UTF-16 units (exact TS rule); upgrade: chars.

/// Byte cap for the editor command (`$VISUAL || $EDITOR` full string).
pub const MAX_EDITOR_BYTES: usize = 256;
/// Byte cap for the tmp edit file path (`os.tmpdir()/Date.now().md`).
pub const MAX_FILE_BYTES: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnError {
    EmptyEditor,
    EditorTooLarge,
    EmptyFile,
    FileTooLarge,
    BadPosition,
    RangeInverted,
}

impl std::fmt::Display for SpawnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::EmptyEditor => "editor command is empty",
            Self::EditorTooLarge => "editor command exceeds size limit",
            Self::EmptyFile => "edit file path is empty",
            Self::FileTooLarge => "edit file path exceeds size limit",
            Self::BadPosition => "position line/character must be >= 1",
            Self::RangeInverted => "selection start is after end",
        };
        f.write_str(s)
    }
}

impl std::error::Error for SpawnError {}

/// `openEditor` spawn plan (editor.ts:26-54). Host splits `editor` on space
/// (editor.ts:35), suspends the renderer iff `suspend_renderer`, spawns the
/// child on `file`, and resumes + re-renders after exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorSpawn {
    pub editor: String,
    pub file: String,
    pub suspend_renderer: bool,
}

impl EditorSpawn {
    pub fn validate(&self) -> Result<(), SpawnError> {
        if self.editor.is_empty() {
            return Err(SpawnError::EmptyEditor);
        }
        if self.editor.len() > MAX_EDITOR_BYTES {
            return Err(SpawnError::EditorTooLarge);
        }
        if self.file.is_empty() {
            return Err(SpawnError::EmptyFile);
        }
        if self.file.len() > MAX_FILE_BYTES {
            return Err(SpawnError::FileTooLarge);
        }
        Ok(())
    }
}

/// One parsed `~/.claude/ide/*.lock` candidate (editor.ts:78-85).
/// `score` is the `contains` value: `max(0, ...resolved.length)` over
/// workspace folders. mtime tie-break (editor.ts:91) is a host concern:
/// pass sockets newest-first; ties keep input order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdeSocket {
    pub path: String,
    pub score: u32,
}

impl IdeSocket {
    pub fn validate(&self) -> Result<(), SpawnError> {
        if self.file_len() == 0 {
            return Err(SpawnError::EmptyFile);
        }
        if self.file_len() > MAX_FILE_BYTES {
            return Err(SpawnError::FileTooLarge);
        }
        Ok(())
    }

    fn file_len(&self) -> usize {
        self.path.len()
    }
}

/// Score-desc pick (editor.ts:91 `right.score - left.score`, take [0]).
/// First wins ties, so hosts pass newest-first to preserve the mtime rule.
#[must_use]
pub fn pick_best(sockets: &[IdeSocket]) -> Option<&IdeSocket> {
    let mut best: Option<&IdeSocket> = None;
    for s in sockets {
        if best.is_none_or(|b| s.score > b.score) {
            best = Some(s);
        }
    }
    best
}

/// `offsetToPosition` (editor-zed.ts:223-243): `byte_offset` is a UTF-8 byte
/// offset. Exact TS rule: `byteOffset <= 0` maps to string start; otherwise
/// accumulate per-char UTF-8 lengths and snap UP to the first char whose
/// end reaches/passes the offset (mid-char offsets round forward); past-end
/// clamps to end. Returns 1-based `(line, character)` with `character` in
/// UTF-16 code units (editor-zed.ts:268-273). `None` when `byte_offset` is
/// past the text or line/col overflow u32 (stricter than TS clamping).
#[must_use]
pub fn offset_to_position(text: &str, byte_offset: usize) -> Option<(u32, u32)> {
    if byte_offset > text.len() {
        return None;
    }
    // Snap up to the next char boundary (exact utf8ByteOffsetToStringIndex rule).
    let mut snapped = text.len();
    let mut bytes = 0usize;
    for (b, ch) in text.char_indices() {
        if bytes >= byte_offset {
            snapped = b;
            break;
        }
        bytes += ch.len_utf8();
        if bytes >= byte_offset {
            snapped = b + ch.len_utf8();
            break;
        }
    }
    if byte_offset == 0 {
        snapped = 0;
    }
    let line = text[..snapped].bytes().filter(|&b| b == b'\n').count();
    let line_start = text[..snapped].rfind('\n').map_or(0, |i| i + 1);
    let character = text[line_start..snapped].encode_utf16().count();
    Some((u32::try_from(line + 1).ok()?, u32::try_from(character + 1).ok()?))
}

/// 1-based `(line, character)` selection range with `start <= end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionRange {
    pub start: (u32, u32),
    pub end: (u32, u32),
}

impl SelectionRange {
    pub fn validate(&self) -> Result<(), SpawnError> {
        let ((sl, sc), (el, ec)) = (self.start, self.end);
        if sl == 0 || sc == 0 || el == 0 || ec == 0 {
            return Err(SpawnError::BadPosition);
        }
        if (sl, sc) > (el, ec) {
            return Err(SpawnError::RangeInverted);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_plan_ok() {
        let p = EditorSpawn { editor: "code --wait".into(), file: "/tmp/1.md".into(), suspend_renderer: true };
        assert_eq!(p.validate(), Ok(()));
    }

    #[test]
    fn spawn_bounds_err() {
        let e = EditorSpawn { editor: String::new(), file: "/tmp/1.md".into(), suspend_renderer: true };
        assert_eq!(e.validate(), Err(SpawnError::EmptyEditor));
        let e = EditorSpawn { editor: "x".repeat(MAX_EDITOR_BYTES + 1), file: "/tmp/1.md".into(), suspend_renderer: false };
        assert_eq!(e.validate(), Err(SpawnError::EditorTooLarge));
        let e = EditorSpawn { editor: "vim".into(), file: String::new(), suspend_renderer: false };
        assert_eq!(e.validate(), Err(SpawnError::EmptyFile));
        let e = EditorSpawn { editor: "vim".into(), file: "x".repeat(MAX_FILE_BYTES + 1), suspend_renderer: false };
        assert_eq!(e.validate(), Err(SpawnError::FileTooLarge));
    }

    #[test]
    fn best_socket_wins() {
        let ss = [
            IdeSocket { path: "a.lock".into(), score: 3 },
            IdeSocket { path: "b.lock".into(), score: 9 },
            IdeSocket { path: "c.lock".into(), score: 5 },
        ];
        assert_eq!(pick_best(&ss).unwrap().path, "b.lock");
        let empty: [IdeSocket; 0] = [];
        assert_eq!(pick_best(&empty), None);
    }

    #[test]
    fn best_socket_tie_keeps_first() {
        let ss = [
            IdeSocket { path: "new.lock".into(), score: 7 },
            IdeSocket { path: "old.lock".into(), score: 7 },
        ];
        assert_eq!(pick_best(&ss).unwrap().path, "new.lock");
    }

    #[test]
    fn offset_ascii_lines() {
        assert_eq!(offset_to_position("ab\ncd", 0), Some((1, 1)));
        assert_eq!(offset_to_position("ab\ncd", 2), Some((1, 3)));
        assert_eq!(offset_to_position("ab\ncd", 3), Some((2, 1)));
        assert_eq!(offset_to_position("ab\ncd", 5), Some((2, 3)));
    }

    #[test]
    fn offset_multibyte_snaps_up() {
        // "a" (1B) + "é" (2B: bytes 1..3) + "\n" + "b".
        assert_eq!(offset_to_position("aé\nb", 1), Some((1, 2)));
        // Mid-char (byte 2 of é) rounds forward to end of é, like TS `bytes >= byteOffset`.
        assert_eq!(offset_to_position("aé\nb", 2), Some((1, 3)));
        assert_eq!(offset_to_position("aé\nb", 3), Some((1, 3)));
        assert_eq!(offset_to_position("aé\nb", 4), Some((2, 1)));
        // Past-end is out of range (stricter than TS clamp).
        assert_eq!(offset_to_position("aé\nb", 99), None);
        // Surrogate pair counts 2 UTF-16 units, matching TS `character`.
        assert_eq!(offset_to_position("😀x", 4), Some((1, 3)));
    }

    #[test]
    fn range_validate() {
        let ok = SelectionRange { start: (1, 1), end: (2, 1) };
        assert_eq!(ok.validate(), Ok(()));
        let same = SelectionRange { start: (2, 3), end: (2, 3) };
        assert_eq!(same.validate(), Ok(()));
        let inv = SelectionRange { start: (2, 1), end: (1, 9) };
        assert_eq!(inv.validate(), Err(SpawnError::RangeInverted));
        let zero = SelectionRange { start: (0, 1), end: (1, 1) };
        assert_eq!(zero.validate(), Err(SpawnError::BadPosition));
    }

    #[test]
    fn socket_validate() {
        let ok = IdeSocket { path: "x.lock".into(), score: 1 };
        assert_eq!(ok.validate(), Ok(()));
        let e = IdeSocket { path: String::new(), score: 0 };
        assert_eq!(e.validate(), Err(SpawnError::EmptyFile));
    }
}
