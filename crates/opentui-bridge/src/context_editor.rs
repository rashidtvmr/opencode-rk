#![forbid(unsafe_code)]
//! Editor context schemas (editor.ts WS): selection + file mention.
//!
//! Pure, bounded, no IO/FFI. File paths capped at 512 bytes.
//! ponytail: `rsplit_once(':')` parse (colon paths); upgrade: real URI parser.

/// Byte cap for file paths in selection/mention schemas.
pub const MAX_FILE_LEN: usize = 512;

/// WS selection: file plus 1-based inclusive line range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorSelection {
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
}

/// WS mention: file plus 1-based line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    pub file: String,
    pub line: u32,
}

impl EditorSelection {
    pub fn validate_selection(&self) -> Result<(), String> {
        if self.file.len() > MAX_FILE_LEN {
            return Err("selection file exceeds size limit".to_string());
        }
        if self.start_line == 0 || self.end_line == 0 {
            return Err("selection lines must be > 0".to_string());
        }
        if self.start_line > self.end_line {
            return Err("selection start must be <= end".to_string());
        }
        Ok(())
    }
}

/// Canonical key: `"file:start-end"`.
#[must_use]
pub fn selection_key(sel: &EditorSelection) -> String {
    format!("{}:{}-{}", sel.file, sel.start_line, sel.end_line)
}

/// Parse `"file:line"`: split at last colon, both parts non-empty, line > 0.
#[must_use]
pub fn parse_mention(s: &str) -> Option<Mention> {
    let (file, line_s) = s.rsplit_once(':')?;
    if file.is_empty() || line_s.is_empty() {
        return None;
    }
    if file.len() > MAX_FILE_LEN {
        return None;
    }
    let line: u32 = line_s.parse().ok()?;
    if line == 0 {
        return None;
    }
    Some(Mention {
        file: file.to_string(),
        line,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_selection_ok() {
        let sel = EditorSelection {
            file: "src/main.rs".into(),
            start_line: 2,
            end_line: 5,
        };
        assert_eq!(sel.validate_selection(), Ok(()));
    }

    #[test]
    fn inverted_selection_errs() {
        let sel = EditorSelection {
            file: "a.ts".into(),
            start_line: 9,
            end_line: 3,
        };
        assert!(sel.validate_selection().is_err());
        let zero = EditorSelection {
            file: "a.ts".into(),
            start_line: 0,
            end_line: 0,
        };
        assert!(zero.validate_selection().is_err());
    }

    #[test]
    fn key_format() {
        let sel = EditorSelection {
            file: "f.ts".into(),
            start_line: 1,
            end_line: 4,
        };
        assert_eq!(selection_key(&sel), "f.ts:1-4");
    }

    #[test]
    fn mention_parses() {
        assert_eq!(
            parse_mention("src/a.ts:12"),
            Some(Mention {
                file: "src/a.ts".into(),
                line: 12
            })
        );
    }

    #[test]
    fn mention_bad_is_none() {
        assert_eq!(parse_mention(""), None);
        assert_eq!(parse_mention("file"), None);
        assert_eq!(parse_mention(":5"), None);
        assert_eq!(parse_mention("f.ts:"), None);
        assert_eq!(parse_mention("f.ts:0"), None);
        assert_eq!(parse_mention("f.ts:abc"), None);
    }
}
