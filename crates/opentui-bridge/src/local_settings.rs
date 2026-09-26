#![forbid(unsafe_code)]
//! Per-directory local settings (model + theme), std only.
//!
//! Evidence (TS checkout a0d9b6c):
//! - `packages/tui/src/context/local.tsx:137-162` model store
//!   `{ready, model: Record<agentName, {providerID, modelID}>, recent[<=10],
//!   favorite[], variant: Record<"provider/model", string>}`, persisted to
//!   `state/model.json` as `{recent, favorite, variant}` (:164-180); the live
//!   `model` map itself is NOT persisted (session-scoped fallback chain
//!   :236-245). `parseModel` splits `"provider/rest"` on first `/` (:27-33).
//! - `packages/tui/src/context/theme.tsx:37-61` theme discovery walks
//!   `cwd -> /` collecting `.opencode/themes/*.json`; selection lives in KV
//!   (`useKV`), NOT per-directory.
//! - `packages/tui/src/context/kv.tsx:51-56` flat get/set persisted to
//!   `kv.json` via `writeJsonAtomic`; IO via `util/persistence.ts`.
//!
//! Divergences (lane contract): this port scopes `{model, theme}` per
//! directory key (TS has per-agent model map + global KV theme, neither
//! per-directory); bounds below follow the lane contract (128/64/1024/128),
//! NOT TS (recent slice 10, unbounded KV). IO reused from
//! `crate::persistence` (atomic tmp+rename, 1 MiB cap); no serde here.
//!
//! Line format (no serde): one entry per line,
//! `escape(dir) \t escape(model) \t escape(theme)`, where missing values
//! encode as empty fields. `escape` maps `\` -> `\\`, TAB -> `\t`,
//! LF -> `\n`, CR -> `\r`; `parse_line` inverts it fail-closed (`None` on
//! malformed escapes, wrong field count, or bound violations).

use crate::persistence::{PersistError, read_text, write_text};

/// Max model string chars (`"provider/model"`).
pub const MAX_MODEL_LEN: usize = 128;
/// Max theme name chars.
pub const MAX_THEME_LEN: usize = 64;
/// Max directory key chars.
pub const MAX_DIR_LEN: usize = 1024;
/// Max directory entries kept; `set` on a new key past this fails.
pub const MAX_ENTRIES: usize = 128;

/// Fail-closed bound/IO violations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalSettingsError {
    DirTooLong,
    DirEmpty,
    DirInvalid,
    ModelTooLong,
    ModelEmpty,
    ModelInvalid,
    ThemeTooLong,
    ThemeEmpty,
    ThemeInvalid,
    TooManyEntries,
    Persist(PersistError),
}

impl std::fmt::Display for LocalSettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DirTooLong => write!(f, "dir exceeds {MAX_DIR_LEN} chars"),
            Self::DirEmpty => write!(f, "dir empty"),
            Self::DirInvalid => write!(f, "dir has tab/newline/control"),
            Self::ModelTooLong => write!(f, "model exceeds {MAX_MODEL_LEN} chars"),
            Self::ModelEmpty => write!(f, "model empty"),
            Self::ModelInvalid => write!(f, "model has tab/newline/control"),
            Self::ThemeTooLong => write!(f, "theme exceeds {MAX_THEME_LEN} chars"),
            Self::ThemeEmpty => write!(f, "theme empty"),
            Self::ThemeInvalid => write!(f, "theme has tab/newline/control"),
            Self::TooManyEntries => write!(f, "exceeds {MAX_ENTRIES} entries"),
            Self::Persist(e) => write!(f, "persist: {e}"),
        }
    }
}

impl std::error::Error for LocalSettingsError {}

impl From<PersistError> for LocalSettingsError {
    fn from(e: PersistError) -> Self {
        Self::Persist(e)
    }
}

fn valid_field(s: &str, max: usize) -> bool {
    !s.is_empty() && s.chars().count() <= max && !s.chars().any(|c| c == '\t' || c == '\n' || c == '\r' || c.is_control())
}

/// Per-directory settings (TS `local.tsx` model entry + KV theme selection).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalSettings {
    pub model: Option<String>,
    pub theme: Option<String>,
}

impl LocalSettings {
    /// Validate bounds fail-closed (`None` fields skipped).
    pub fn validate(&self) -> Result<(), LocalSettingsError> {
        if let Some(m) = &self.model {
            if m.is_empty() {
                return Err(LocalSettingsError::ModelEmpty);
            }
            if m.chars().count() > MAX_MODEL_LEN {
                return Err(LocalSettingsError::ModelTooLong);
            }
            if !valid_field(m, MAX_MODEL_LEN) {
                return Err(LocalSettingsError::ModelInvalid);
            }
        }
        if let Some(t) = &self.theme {
            if t.is_empty() {
                return Err(LocalSettingsError::ThemeEmpty);
            }
            if t.chars().count() > MAX_THEME_LEN {
                return Err(LocalSettingsError::ThemeTooLong);
            }
            if !valid_field(t, MAX_THEME_LEN) {
                return Err(LocalSettingsError::ThemeInvalid);
            }
        }
        Ok(())
    }
}

fn check_dir(dir: &str) -> Result<(), LocalSettingsError> {
    if dir.is_empty() {
        return Err(LocalSettingsError::DirEmpty);
    }
    if dir.chars().count() > MAX_DIR_LEN {
        return Err(LocalSettingsError::DirTooLong);
    }
    if !valid_field(dir, MAX_DIR_LEN) {
        return Err(LocalSettingsError::DirInvalid);
    }
    Ok(())
}

fn escape(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
}

fn unescape(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next()? {
                '\\' => out.push('\\'),
                't' => out.push('\t'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                _ => return None,
            }
        } else if c == '\t' || c == '\n' || c == '\r' {
            return None;
        } else {
            out.push(c);
        }
    }
    Some(out)
}

/// Encode one entry as `dir \t model \t theme` (empty = `None`).
#[must_use]
pub fn serialize_line(dir: &str, s: &LocalSettings) -> String {
    let mut line = String::new();
    escape(dir, &mut line);
    line.push('\t');
    if let Some(m) = &s.model {
        escape(m, &mut line);
    }
    line.push('\t');
    if let Some(t) = &s.theme {
        escape(t, &mut line);
    }
    line
}

/// Decode one line fail-closed (`None` on bad shape/escapes/bounds).
#[must_use]
pub fn parse_line(line: &str) -> Option<(String, LocalSettings)> {
    let line = line.strip_suffix('\n').unwrap_or(line).strip_suffix('\r').unwrap_or(line);
    if line.is_empty() {
        return None;
    }
    let mut parts = line.splitn(3, '\t');
    let (d, m, t) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let dir = unescape(d)?;
    let model = unescape(m)?;
    let theme = unescape(t)?;
    check_dir(&dir).ok()?;
    let s = LocalSettings {
        model: if model.is_empty() { None } else { Some(model) },
        theme: if theme.is_empty() { None } else { Some(theme) },
    };
    s.validate().ok()?;
    Some((dir, s))
}

/// Per-directory keyed store (dir -> settings).
#[derive(Debug, Clone, Default)]
pub struct LocalSettingsStore {
    entries: Vec<(String, LocalSettings)>,
}

impl LocalSettingsStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn get(&self, dir: &str) -> Option<&LocalSettings> {
        self.entries.iter().find(|(k, _)| k == dir).map(|(_, v)| v)
    }

    /// Upsert; new keys past [`MAX_ENTRIES`] fail, bounds fail-closed.
    pub fn set(&mut self, dir: &str, s: LocalSettings) -> Result<(), LocalSettingsError> {
        check_dir(dir)?;
        s.validate()?;
        if let Some(slot) = self.entries.iter_mut().find(|(k, _)| k == dir) {
            slot.1 = s;
            return Ok(());
        }
        if self.entries.len() >= MAX_ENTRIES {
            return Err(LocalSettingsError::TooManyEntries);
        }
        self.entries.push((dir.to_string(), s));
        Ok(())
    }

    /// Remove one dir; `true` when an entry existed.
    pub fn clear(&mut self, dir: &str) -> bool {
        self.entries.iter().position(|(k, _)| k == dir).map(|i| self.entries.remove(i)).is_some()
    }

    /// Encode whole store (one line per entry, trailing `\n` each).
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for (d, s) in &self.entries {
            out.push_str(&serialize_line(d, s));
            out.push('\n');
        }
        out
    }

    /// Decode whole file fail-closed: first bad line aborts with `None`.
    #[must_use]
    pub fn from_text(text: &str) -> Option<Self> {
        let mut store = Self::new();
        for line in text.lines() {
            if line.is_empty() {
                continue;
            }
            let (dir, s) = parse_line(line)?;
            if store.get(&dir).is_some() {
                return None; // duplicate dir key
            }
            if store.entries.len() >= MAX_ENTRIES {
                return None;
            }
            store.entries.push((dir, s));
        }
        Some(store)
    }

    /// Atomic save via `crate::persistence::write_text`.
    pub fn save(&self, path: &std::path::Path) -> Result<(), LocalSettingsError> {
        write_text(path, &self.to_text()).map_err(LocalSettingsError::from)
    }

    /// Load via `crate::persistence::read_text`; malformed -> `Persist` err.
    pub fn load(path: &std::path::Path) -> Result<Self, LocalSettingsError> {
        let text = read_text(path).map_err(LocalSettingsError::from)?;
        Self::from_text(&text).ok_or(LocalSettingsError::Persist(PersistError::JsonShape(
            "malformed local-settings line".to_string(),
        )))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(model: Option<&str>, theme: Option<&str>) -> LocalSettings {
        LocalSettings {
            model: model.map(str::to_string),
            theme: theme.map(str::to_string),
        }
    }

    #[test]
    fn set_get_roundtrip() {
        let mut st = LocalSettingsStore::new();
        st.set("/repo", settings(Some("anthropic/claude"), Some("dark"))).unwrap();
        let got = st.get("/repo").unwrap();
        assert_eq!(got.model.as_deref(), Some("anthropic/claude"));
        assert_eq!(got.theme.as_deref(), Some("dark"));
        // overwrite same dir keeps single entry
        st.set("/repo", settings(None, Some("light"))).unwrap();
        assert_eq!(st.len(), 1);
        assert!(st.get("/repo").unwrap().model.is_none());
    }

    #[test]
    fn model_overlong_rejected() {
        let mut st = LocalSettingsStore::new();
        let big = "m".repeat(MAX_MODEL_LEN + 1);
        assert_eq!(
            st.set("/repo", settings(Some(&big), None)),
            Err(LocalSettingsError::ModelTooLong)
        );
        assert!(st.is_empty());
    }

    #[test]
    fn theme_bounds_rejected() {
        let mut st = LocalSettingsStore::new();
        let big = "t".repeat(MAX_THEME_LEN + 1);
        assert_eq!(
            st.set("/repo", settings(None, Some(&big))),
            Err(LocalSettingsError::ThemeTooLong)
        );
        assert_eq!(st.set("/repo", settings(None, Some(""))), Err(LocalSettingsError::ThemeEmpty));
        assert_eq!(
            st.set("/repo", settings(None, Some("a\tb"))),
            Err(LocalSettingsError::ThemeInvalid)
        );
    }

    #[test]
    fn dir_bounds_rejected() {
        let mut st = LocalSettingsStore::new();
        let big = "d".repeat(MAX_DIR_LEN + 1);
        assert_eq!(st.set(&big, settings(None, None)), Err(LocalSettingsError::DirTooLong));
        assert_eq!(st.set("", settings(None, None)), Err(LocalSettingsError::DirEmpty));
        assert_eq!(st.set("a\nb", settings(None, None)), Err(LocalSettingsError::DirInvalid));
    }

    #[test]
    fn entries_cap_fails_new_key() {
        let mut st = LocalSettingsStore::new();
        for i in 0..MAX_ENTRIES {
            st.set(&format!("/d{i}"), settings(None, None)).unwrap();
        }
        assert_eq!(
            st.set("/overflow", settings(None, None)),
            Err(LocalSettingsError::TooManyEntries)
        );
        // update of existing key still ok at cap
        st.set("/d0", settings(Some("p/m"), None)).unwrap();
        assert_eq!(st.len(), MAX_ENTRIES);
    }

    #[test]
    fn line_roundtrip_with_escapes() {
        let s = settings(Some("a\\b"), Some("th"));
        let line = serialize_line("/r epo", &s);
        let (dir, back) = parse_line(&line).unwrap();
        assert_eq!(dir, "/r epo");
        assert_eq!(back, s);
        // empty optionals encode as empty fields
        let (d2, b2) = parse_line(&serialize_line("/x", &settings(None, None))).unwrap();
        assert_eq!(d2, "/x");
        assert!(b2.model.is_none() && b2.theme.is_none());
    }

    #[test]
    fn parse_malformed_none() {
        assert!(parse_line("").is_none());
        assert!(parse_line("onlyone").is_none());
        assert!(parse_line("a\tb\tc\td").is_none()); // 4 fields
        assert!(parse_line("a\\q\tb\tc").is_none()); // bad escape
        assert!(parse_line(&format!("{}\tb\tc", "d".repeat(MAX_DIR_LEN + 1))).is_none());
    }

    #[test]
    fn clear_removes() {
        let mut st = LocalSettingsStore::new();
        st.set("/a", settings(Some("p/m"), None)).unwrap();
        assert!(st.clear("/a"));
        assert!(!st.clear("/a"));
        assert!(st.is_empty());
    }

    #[test]
    fn save_load_roundtrip() {
        let p = std::env::temp_dir().join(format!("bridge047-local-{}.txt", std::process::id()));
        let mut st = LocalSettingsStore::new();
        st.set("/repo", settings(Some("openai/gpt"), Some("dark"))).unwrap();
        st.save(&p).unwrap();
        let back = LocalSettingsStore::load(&p).unwrap();
        assert_eq!(back.get("/repo").unwrap().theme.as_deref(), Some("dark"));
        let _ = std::fs::remove_file(&p);
        // missing file -> Persist(Io)
        assert!(matches!(
            LocalSettingsStore::load(&p),
            Err(LocalSettingsError::Persist(PersistError::Io(_)))
        ));
    }
}
