#![forbid(unsafe_code)]
//! Prompt `/editor` handoff (mirrors `prompt.editor.ts`: slash value resolve,
//! editor mention realign; host owns spawn/IO).
//!
//! ponytail: plan types only, host spawns editor; upgrade: wire `editor_spawn`.

/// Char cap for the edit file path.
pub const MAX_FILE_CHARS: usize = 512;
/// Char cap for edited text returned by [`finish_edit`].
pub const MAX_EDIT_CHARS: usize = 64 * 1024;

/// External editor handoff: file to open + 0-based line hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorRequest {
    pub file: String,
    pub line: u32,
}

/// Build a handoff; errs on empty/overlong file. Any `line` ok (0 = top).
pub fn start_edit(file: &str, line: u32) -> Result<EditorRequest, String> {
    if file.is_empty() {
        return Err("edit file is empty".to_string());
    }
    if file.chars().count() > MAX_FILE_CHARS {
        return Err("edit file exceeds size limit".to_string());
    }
    Ok(EditorRequest {
        file: file.to_string(),
        line,
    })
}

/// Stable key for dedupe/cache: `"file:line"`.
#[must_use]
pub fn editor_key(req: &EditorRequest) -> String {
    format!("{}:{}", req.file, req.line)
}

/// Take editor output; truncate to 64KiB chars (never grow past).
#[must_use]
pub fn finish_edit(_req: &EditorRequest, text: &str) -> String {
    if text.chars().count() <= MAX_EDIT_CHARS {
        return text.to_string();
    }
    text.chars().take(MAX_EDIT_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_errs() {
        assert!(start_edit("", 1).is_err());
    }

    #[test]
    fn overlong_errs() {
        let f = "x".repeat(MAX_FILE_CHARS + 1);
        assert!(start_edit(&f, 0).is_err());
    }

    #[test]
    fn key_format() {
        let r = start_edit("a.md", 3).unwrap();
        assert_eq!(editor_key(&r), "a.md:3");
    }

    #[test]
    fn truncate_cap() {
        let r = start_edit("a.md", 0).unwrap();
        let big = "y".repeat(MAX_EDIT_CHARS + 10);
        let out = finish_edit(&r, &big);
        assert_eq!(out.chars().count(), MAX_EDIT_CHARS);
    }

    #[test]
    fn line_zero_ok() {
        let r = start_edit("a.md", 0).unwrap();
        assert_eq!(r.line, 0);
    }

    #[test]
    fn roundtrip() {
        let r = start_edit("a.md", 7).unwrap();
        let out = finish_edit(&r, "hello");
        assert_eq!(out, "hello");
        assert_eq!(editor_key(&r), "a.md:7");
    }
}
