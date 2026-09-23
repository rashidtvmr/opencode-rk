#![forbid(unsafe_code)]
//! Renderer config: parser languages, logo marks, wordmark line count
//! (mirrors `packages/tui/src/parsers-config.ts:1-7` filetype list,
//! `packages/tui/src/logo.ts:1-11` logo/go/marks, and
//! `packages/tui/src/util/presentation.ts:1-27` 4-line wordmark,
//! TS checkout a0d9b6c).

use crate::core_events::KittyFlags;

/// Max registered parser languages (TS list ~34 entries; bounded headroom).
pub const MAX_LANGS: usize = 128;
/// Wordmark renders exactly 4 lines (presentation.ts `logo.left` len 4).
pub const WORDMARK_LINES: usize = 4;
/// Logo draw marks (mirrors `marks = "_^~,"` in logo.ts:11).
pub const LOGO_MARKS: &str = "_^~,";
/// Block glyphs used by the wordmark (presentation.ts draw fn).
pub const LOGO_BLOCK_FULL: char = '█';
pub const LOGO_BLOCK_HALF: char = '▀';

/// Builtin parser (opentui core) languages, no wasm fetch needed
/// (parsers-config.ts:2 "for markdown, javascript and typescript, we use
/// the opentui built-in parsers").
pub const BUILTIN_LANGS: &[&str] = &["markdown", "javascript", "typescript"];

/// Bounded parser-language registry (filetype strings from parsers-config.ts).
#[derive(Debug, Default, Clone)]
pub struct ParserConfig {
    langs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParserError {
    Full,
    BadName,
}

impl core::fmt::Display for ParserError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Full => write!(f, "parser registry full"),
            Self::BadName => write!(f, "bad language name"),
        }
    }
}

impl std::error::Error for ParserError {}

impl ParserConfig {
    #[must_use]
    pub const fn new() -> Self {
        Self { langs: Vec::new() }
    }

    /// Register a filetype; lowercase ASCII alphabetic, deduped.
    pub fn register(&mut self, lang: &str) -> Result<(), ParserError> {
        if lang.is_empty() || lang.len() > 32 || !lang.bytes().all(|b| b.is_ascii_lowercase()) {
            return Err(ParserError::BadName);
        }
        if self.langs.iter().any(|l| l == lang) {
            return Ok(());
        }
        if self.langs.len() >= MAX_LANGS {
            return Err(ParserError::Full);
        }
        self.langs.push(lang.to_string());
        Ok(())
    }

    #[must_use]
    pub fn contains(&self, lang: &str) -> bool {
        self.langs.iter().any(|l| l == lang)
    }

    #[must_use]
    pub fn is_builtin(lang: &str) -> bool {
        BUILTIN_LANGS.contains(&lang)
    }
}

/// Wordmark line count (mirrors `wordmark()` returning `logo.left.map`,
/// 4 lines; `sessionEpilogue` spreads those 4 + blank + 2 rows).
#[must_use]
pub const fn wordmark_line_count() -> usize {
    WORDMARK_LINES
}

/// Console/renderer options (mirrors `app.tsx:194-242` `createCliRenderer`
/// call and `bg-pulse.tsx:74-86` writable fps fields, TS checkout a0d9b6c).
/// Reuses [`crate::core_events::KittyFlags`]; not redefined here.
/// NOTE: renderer default kitty bits are 0, but `useKittyKeyboard: {}`
/// (app.tsx:199) builds flags 5 via `from_empty` (see core_events.rs:27-29).

/// Max console key bindings (TS passes 1; bounded headroom).
pub const MAX_KEY_BINDINGS: usize = 16;
/// Max chars for binding name/action ids.
pub const MAX_BINDING_STR: usize = 64;
/// Theme-mode wait timeout ms (`app.tsx:242` `waitForThemeMode(1000)`).
pub const WAIT_FOR_THEME_MODE_MS: u64 = 1000;
/// Console action ids.
pub const ACTION_COPY_SELECTION: &str = "copy-selection";
pub const ACTION_TOGGLE: &str = "toggle";
/// Valid fps range for [`MaxFps`] (`targetFps: 60` app.tsx:196, 30 in bg-pulse).
pub const MIN_FPS: u16 = 1;
pub const MAX_FPS: u16 = 240;

/// Console actions with stable action ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleAction {
    CopySelection,
    Toggle,
}

impl ConsoleAction {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CopySelection => ACTION_COPY_SELECTION,
            Self::Toggle => ACTION_TOGGLE,
        }
    }

    #[must_use]
    pub fn from_action(action: &str) -> Option<Self> {
        match action {
            ACTION_COPY_SELECTION => Some(Self::CopySelection),
            ACTION_TOGGLE => Some(Self::Toggle),
            _ => None,
        }
    }
}

/// One console key binding (`{name, ctrl, action}`, app.tsx:204).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    pub name: String,
    pub ctrl: bool,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingError {
    BadName,
    BadAction,
    Full,
}

impl core::fmt::Display for BindingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BadName => write!(f, "bad binding name"),
            Self::BadAction => write!(f, "bad binding action"),
            Self::Full => write!(f, "binding list full"),
        }
    }
}

impl std::error::Error for BindingError {}

impl KeyBinding {
    pub fn new(name: &str, ctrl: bool, action: &str) -> Result<Self, BindingError> {
        if name.is_empty() || name.len() > MAX_BINDING_STR {
            return Err(BindingError::BadName);
        }
        if action.is_empty() || action.len() > MAX_BINDING_STR {
            return Err(BindingError::BadAction);
        }
        Ok(Self { name: name.to_string(), ctrl, action: action.to_string() })
    }

    /// Opencode default: `y` + ctrl -> copy-selection (app.tsx:204).
    #[must_use]
    pub fn copy_selection() -> Self {
        Self { name: "y".to_string(), ctrl: true, action: ACTION_COPY_SELECTION.to_string() }
    }

    #[must_use]
    pub fn action_kind(&self) -> Option<ConsoleAction> {
        ConsoleAction::from_action(&self.action)
    }
}

/// Console options: key bindings + kitty flags.
#[derive(Debug, Clone)]
pub struct ConsoleOptions {
    pub key_bindings: Vec<KeyBinding>,
    pub kitty: KittyFlags,
}

impl ConsoleOptions {
    #[must_use]
    pub fn defaults() -> Self {
        Self { key_bindings: vec![KeyBinding::copy_selection()], kitty: KittyFlags::from_empty() }
    }

    pub fn push(&mut self, b: KeyBinding) -> Result<(), BindingError> {
        if self.key_bindings.len() >= MAX_KEY_BINDINGS {
            return Err(BindingError::Full);
        }
        self.key_bindings.push(b);
        Ok(())
    }
}

/// Validated fps value (writable `targetFps`/`maxFps`, bg-pulse.tsx:74-86).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaxFps(u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpsError {
    OutOfRange,
}

impl core::fmt::Display for FpsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "fps out of range 1..=240")
    }
}

impl std::error::Error for FpsError {}

impl MaxFps {
    pub const fn new(fps: u16) -> Result<Self, FpsError> {
        if fps < MIN_FPS || fps > MAX_FPS {
            return Err(FpsError::OutOfRange);
        }
        Ok(Self(fps))
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    pub fn set_max_fps(&mut self, fps: u16) -> Result<(), FpsError> {
        self.0 = Self::new(fps)?.0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_contains_dedupes() {
        let mut c = ParserConfig::new();
        c.register("rust").unwrap();
        c.register("rust").unwrap();
        assert!(c.contains("rust"));
        assert!(!c.contains("go"));
        assert_eq!(c.langs.len(), 1);
    }

    #[test]
    fn rejects_bad_names() {
        let mut c = ParserConfig::new();
        for bad in ["", "Rust", "c++", "with space"] {
            assert_eq!(c.register(bad), Err(ParserError::BadName), "{bad:?}");
        }
    }

    #[test]
    fn builtin_langs_need_no_wasm() {
        for lang in BUILTIN_LANGS {
            assert!(ParserConfig::is_builtin(lang));
        }
        assert!(!ParserConfig::is_builtin("rust"));
    }

    #[test]
    fn wordmark_consts_match_ts() {
        assert_eq!(wordmark_line_count(), 4);
        assert_eq!(LOGO_MARKS, "_^~,");
        assert_eq!((LOGO_BLOCK_FULL, LOGO_BLOCK_HALF), ('█', '▀'));
    }

    #[test]
    fn defaults_match_app_tsx() {
        let o = ConsoleOptions::defaults();
        assert_eq!(o.key_bindings.len(), 1);
        assert_eq!(o.key_bindings[0].name, "y");
        assert!(o.key_bindings[0].ctrl);
        assert_eq!(o.key_bindings[0].action, "copy-selection");
        assert_eq!(o.kitty.bits(), 5);
        assert_eq!(WAIT_FOR_THEME_MODE_MS, 1000);
    }

    #[test]
    fn action_ids_roundtrip() {
        assert_eq!(ConsoleAction::CopySelection.as_str(), "copy-selection");
        assert_eq!(ConsoleAction::Toggle.as_str(), "toggle");
        assert_eq!(ConsoleAction::from_action("copy-selection"), Some(ConsoleAction::CopySelection));
        assert_eq!(ConsoleAction::from_action("toggle"), Some(ConsoleAction::Toggle));
        assert_eq!(ConsoleAction::from_action("nope"), None);
    }

    #[test]
    fn binding_bounds_rejected() {
        assert_eq!(KeyBinding::new("", true, "copy-selection"), Err(BindingError::BadName));
        assert_eq!(KeyBinding::new("y", true, ""), Err(BindingError::BadAction));
        let long = "x".repeat(65);
        assert_eq!(KeyBinding::new(&long, false, "toggle"), Err(BindingError::BadName));
        assert_eq!(KeyBinding::new("y", false, &long), Err(BindingError::BadAction));
    }

    #[test]
    fn bindings_cap_at_16() {
        let mut o = ConsoleOptions::defaults();
        for i in 0..15 {
            o.push(KeyBinding::new(&format!("k{i}"), false, "toggle").unwrap()).unwrap();
        }
        assert_eq!(o.key_bindings.len(), 16);
        assert_eq!(
            o.push(KeyBinding::new("over", false, "toggle").unwrap()),
            Err(BindingError::Full)
        );
    }

    #[test]
    fn fps_validates_1_to_240() {
        assert!(MaxFps::new(0).is_err());
        assert!(MaxFps::new(241).is_err());
        let mut f = MaxFps::new(60).unwrap();
        assert_eq!(f.get(), 60);
        f.set_max_fps(30).unwrap();
        assert_eq!(f.get(), 30);
        assert!(f.set_max_fps(0).is_err());
        assert_eq!(f.get(), 30);
    }

    #[test]
    fn copy_selection_kind_matches() {
        let b = KeyBinding::copy_selection();
        assert_eq!(b.action_kind(), Some(ConsoleAction::CopySelection));
        assert!(KeyBinding::new("z", false, "custom").unwrap().action_kind().is_none());
    }
}
