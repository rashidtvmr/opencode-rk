#![forbid(unsafe_code)]
//! Zed external-editor contract.
//!
//! Mirrors `packages/tui/src/editor-zed.ts:187-199` (`resolveZedDbPath`,
//! `isZedTerminal`), `packages/tui/src/context/editor.ts:121` (`zedTerminal`),
//! `packages/tui/src/editor.ts:26-27` (`openEditor` input, `VISUAL || EDITOR`).
//! Divergence note: TS checkout at a0d9b6c, not pinned 95daf90.
//! Pure, bounded, std only. No spawn in lib; caller spawns with [`wait_flag`].
//! ponytail: `zed file:line` form only; upgrade: `file:line:col` when needed.

use crate::editor::{EditorError, OpenEditorRequest};

/// Open-target: file path plus optional 1-based line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZedRequest {
    pub path: String,
    pub line: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZedError {
    EmptyPath,
    InvalidLine,
}

impl std::fmt::Display for ZedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::EmptyPath => "zed request path is empty",
            Self::InvalidLine => "zed request line must be >= 1",
        };
        f.write_str(s)
    }
}

impl std::error::Error for ZedError {}

impl ZedRequest {
    pub fn validate(&self) -> Result<(), ZedError> {
        if self.path.is_empty() {
            return Err(ZedError::EmptyPath);
        }
        if matches!(self.line, Some(0)) {
            return Err(ZedError::InvalidLine);
        }
        Ok(())
    }

    /// `zed path` / `zed path:line` location argument.
    #[must_use]
    pub fn location(&self) -> String {
        match self.line {
            Some(n) => format!("{}:{n}", self.path),
            None => self.path.clone(),
        }
    }

    /// Reuse [`OpenEditorRequest`] (editor.ts:26 `{value, cwd}`) without redefining it.
    pub fn to_open_request(&self, cwd: &str, value: &str) -> Result<OpenEditorRequest, ZedError> {
        self.validate()?;
        let req = OpenEditorRequest { value: value.to_string(), cwd: cwd.to_string() };
        req.validate().map_err(|e| match e {
            EditorError::EmptyCwd => ZedError::EmptyPath,
            _ => ZedError::InvalidLine,
        })?;
        Ok(req)
    }
}

/// Resolved editor: explicit Zed, `$VISUAL`/`$EDITOR` (editor.ts:27), or default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorChoice {
    Zed,
    Env(String),
    Default,
}

/// `editor-zed.ts:198`: `ZED_TERM === "true" || TERM_PROGRAM == "zed"`.
#[must_use]
pub fn is_zed_terminal() -> bool {
    std::env::var("ZED_TERM").as_deref() == Ok("true")
        || std::env::var("TERM_PROGRAM").map(|v| v.to_lowercase()).as_deref() == Ok("zed")
}

fn classify(raw: &str) -> Option<EditorChoice> {
    let cmd = raw.split_whitespace().next().unwrap_or("");
    let base = cmd.rsplit('/').next().unwrap_or(cmd);
    if base == "zed" || base.starts_with("zed-") {
        Some(EditorChoice::Zed)
    } else if raw.trim().is_empty() {
        None
    } else {
        Some(EditorChoice::Env(raw.trim().to_string()))
    }
}

/// Preference wins, then `$VISUAL || $EDITOR`, then Zed-terminal probe, else default.
#[must_use]
pub fn resolve_editor(preference: Option<&str>) -> EditorChoice {
    if let Some(p) = preference {
        if let Some(c) = classify(p) {
            return c;
        }
    }
    for key in ["VISUAL", "EDITOR"] {
        if let Ok(v) = std::env::var(key) {
            if let Some(c) = classify(&v) {
                return c;
            }
        }
    }
    if is_zed_terminal() { EditorChoice::Zed } else { EditorChoice::Default }
}

/// Blocking flag per editor: Zed must be spawned with `--wait` so `openEditor`
/// (editor.ts:34-46, awaits child exit) sees the edited file.
#[must_use]
pub fn wait_flag(choice: &EditorChoice) -> Option<&'static str> {
    match choice {
        EditorChoice::Zed => Some("--wait"),
        EditorChoice::Env(_) | EditorChoice::Default => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zed_path_carry() {
        let r = ZedRequest { path: "src/main.rs".into(), line: Some(12) };
        assert_eq!(r.validate(), Ok(()));
        assert_eq!(r.location(), "src/main.rs:12");
        let bare = ZedRequest { path: "src/main.rs".into(), line: None };
        assert_eq!(bare.location(), "src/main.rs");
    }

    #[test]
    fn bad_path_errs() {
        let r = ZedRequest { path: String::new(), line: None };
        assert_eq!(r.validate(), Err(ZedError::EmptyPath));
    }

    #[test]
    fn line_zero_errs() {
        let r = ZedRequest { path: "a.rs".into(), line: Some(0) };
        assert_eq!(r.validate(), Err(ZedError::InvalidLine));
    }

    #[test]
    fn env_fallback() {
        assert_eq!(resolve_editor(Some("zed")), EditorChoice::Zed);
        assert_eq!(resolve_editor(Some("code --wait")), EditorChoice::Env("code --wait".into()));
        assert_eq!(wait_flag(&EditorChoice::Zed), Some("--wait"));
        assert_eq!(wait_flag(&EditorChoice::Default), None);
        // to_open_request reuses OpenEditorRequest validation (empty cwd errs).
        let r = ZedRequest { path: "a.rs".into(), line: None };
        assert!(r.to_open_request("", "hi").is_err());
        assert!(r.to_open_request("/tmp", "hi").is_ok());
    }
}
