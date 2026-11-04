#![forbid(unsafe_code)]
//! Small, bounded adapter for the native TUI's persisted presentation settings.
//!
//! Paths are supplied by the caller so this module never guesses a home/config
//! directory or reads a user's files as a side effect of construction.

use rustix::{fs as rfs, process::getuid};
use serde_json::{Map, Value};
use std::{
    fs::{self, File},
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_CONFIG_BYTES: usize = 1024 * 1024;
const MAX_STATE_BYTES: usize = 64 * 1024;
/// Bounds for the persisted tab store (`state/tabs.json`): at most this many
/// scopes are retained, and each session id / title is length-capped so the
/// store stays far below `MAX_STATE_BYTES`.
const MAX_TAB_SCOPES: usize = 8;
const MAX_SESSION_ID_BYTES: usize = 128;
const MAX_TAB_TITLE_BYTES: usize = 200;
/// Persisted MCP sidebar disclosure key (SIDE-09). Upstream namespaces every
/// plugin store as `plugin.${input.id}.${key}` (`plugin/api.tsx:201`) with
/// `id = "opencode.sidebar.mcp"` (`sidebar/mcp.tsx:96`) and `key = "view"`
/// (`sidebar/mcp.tsx:6`), and the value shape is `{ open: bool }`. A bare
/// `"view"` key would collide with `feature-plugins/sidebar/todo.tsx:7`,
/// which stores `view` on its own `opencode.sidebar.todo` namespace.
const MCP_SIDEBAR_VIEW_KEY: &str = "plugin.opencode.sidebar.mcp.view";
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// Resolved `tabs.scope` (`config/index.tsx:193-195,334`, default `cwd`) and
/// the store key it selects (`context/session-tabs.tsx:105-113,115-118`):
/// `global` persists one shared layout, `cwd` a separate set per working
/// directory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabPersistenceScope<'a> {
    Global,
    Cwd(&'a str),
}

impl<'a> From<&'a str> for TabPersistenceScope<'a> {
    fn from(value: &'a str) -> Self {
        TabPersistenceScope::Cwd(value)
    }
}

/// A `tui.prompt.max_width` value. `None` means use the pinned upstream default
/// of 75 columns; `Auto` means max(75, floor(terminal width * .7)).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromptWidth {
    Auto,
    Fixed(u16),
}

/// Values read from the ordered TUI config files and OpenTUI's `kv.json` state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UiPreferences {
    pub prompt_width: Option<PromptWidth>,
    /// `prompt.paste` (`config/index.tsx:144`,
    /// `Schema.Literals(["compact", "full"])`). The live source for the CP-36
    /// fold decision (`prompt/index.tsx:1463`): only the exact value `"full"`
    /// disables folding; every other value (including unset) folds.
    pub prompt_paste: Option<String>,
    /// `tui.json` theme takes precedence over the persisted `kv.json` choice.
    pub theme: Option<String>,
    pub sidebar_hidden: bool,
    /// SIDE-09: persisted MCP sidebar disclosure (`sidebar/mcp.tsx:6/25-31`
    /// `storage.store("view", { initial: { open: true } })`, namespaced
    /// `plugin.opencode.sidebar.mcp.view` per `plugin/api.tsx:201`). `None`
    /// means never persisted, so the caller keeps the upstream default
    /// `open: true`; `Some(bool)` seeds the view before the first `apply_mcp`.
    pub mcp_sidebar_open: Option<bool>,
    pub theme_mode: Option<String>,
    pub theme_mode_lock: Option<String>,
    pub vertical_tabs_width: Option<u16>,
    /// Getting-started onboarding dismissal (`footer.tsx:11-16`
    /// `storage.store("getting-started", { initial: { dismissed: false } })`,
    /// set by the `sidebar.footer.getting-started.dismiss` mouse-up). Persisted
    /// beside the other TUI state under the exact upstream key/value shape
    /// `"getting-started": {"dismissed": bool}`.
    pub onboarding_dismissed: bool,
    /// Raw `tui.cursor` object from the merged config files, when present.
    /// Kept as JSON so this module stays independent of the theme runtime's
    /// `CursorConfig`; the caller parses it with
    /// [`crate::native_theme_runtime::CursorConfig::from_json`] (THEME-14).
    pub cursor: Option<Value>,
    /// `tui.animations` (`config/index.tsx:250` `animations?: boolean`,
    /// consumed as `config.animations ?? true`). `None` means the config did
    /// not set it, so the caller resolves to the upstream default `true`.
    /// Drives reduced-motion tab/hover paint (TAB-24) and the existing
    /// spinner/status fallbacks.
    pub animations: Option<bool>,
    /// TAB-23: raw TUI `keybinds` overrides (`config/index.tsx:78`,
    /// `Schema.optional(TuiKeybind.KeybindOverrides)`). Kept as raw JSON like
    /// `cursor`; the caller validates entries and resolves the live
    /// `session.tab.*` chord table with
    /// [`crate::native_navigation::tab_bindings`].
    pub keybinds: Option<Value>,
    /// Bounded diagnostics for config/state files skipped during startup.
    pub diagnostics: Vec<String>,
}

impl UiPreferences {
    pub fn markdown_mode(cli_path: Option<&Path>, inline: Option<&str>) -> (Option<String>, Vec<String>) {
        fn apply(text: &str, selected: &mut Option<String>, diagnostics: &mut Vec<String>, label: &str) {
            if text.len() > MAX_CONFIG_BYTES { diagnostics.push(format!("{label} configuration exceeds size limit")); return; }
            let clean = match strip_jsonc(text) { Ok(clean) => clean, Err(_) => { diagnostics.push(format!("invalid {label} configuration")); return; } };
            let value = match serde_json::from_str::<Value>(&clean) {
                Ok(value) => value,
                Err(_) => { diagnostics.push(format!("invalid {label} configuration")); return; }
            };
            match value.get("session").and_then(|session| session.get("markdown")) {
                Some(Value::String(mode)) if mode == "source" || mode == "rendered" => *selected = Some(mode.clone()),
                Some(_) => diagnostics.push("invalid session.markdown value".to_owned()),
                None => {}
            }
        }
        let mut selected = None; let mut diagnostics = Vec::new();
        if let Some(path) = cli_path { let mut budget = crate::native_ui_budget::ReadBudget::default(); match budget.read(path, MAX_CONFIG_BYTES, "CLI config") { Ok(Some(text)) => apply(&text, &mut selected, &mut diagnostics, "disk CLI"), Ok(None) => {}, Err(_) => diagnostics.push("unable to read bounded global CLI configuration".into()) } }
        if let Some(text) = inline { apply(text, &mut selected, &mut diagnostics, "inline CLI"); }
        (selected, diagnostics)
    }

    /// `config.session.scrollbar` (`routes/session/index.tsx:243`). Upstream
    /// defaults to `false`; an explicit non-boolean is reported as the same
    /// bounded diagnostic style used for the other session settings.
    pub fn session_scrollbar(cli_path: Option<&Path>, inline: Option<&str>) -> (bool, Vec<String>) {
        fn apply(text: &str, selected: &mut bool, diagnostics: &mut Vec<String>, label: &str) {
            if text.len() > MAX_CONFIG_BYTES { diagnostics.push(format!("{label} configuration exceeds size limit")); return; }
            let clean = match strip_jsonc(text) { Ok(clean) => clean, Err(_) => { diagnostics.push(format!("invalid {label} configuration")); return; } };
            let value = match serde_json::from_str::<Value>(&clean) {
                Ok(value) => value,
                Err(_) => { diagnostics.push(format!("invalid {label} configuration")); return; }
            };
            match value.get("session").and_then(|session| session.get("scrollbar")) {
                Some(Value::Bool(shown)) => *selected = *shown,
                Some(_) => diagnostics.push("invalid session.scrollbar value".to_owned()),
                None => {}
            }
        }
        let mut selected = false;
        let mut diagnostics = Vec::new();
        if let Some(path) = cli_path { let mut budget = crate::native_ui_budget::ReadBudget::default(); match budget.read(path, MAX_CONFIG_BYTES, "CLI config") { Ok(Some(text)) => apply(&text, &mut selected, &mut diagnostics, "disk CLI"), Ok(None) => {}, Err(_) => diagnostics.push("unable to read bounded global CLI configuration".into()) } }
        if let Some(text) = inline { apply(text, &mut selected, &mut diagnostics, "inline CLI"); }
        (selected, diagnostics)
    }

    /// `config.session.tps` (`config/index.tsx:171,328`, consumed at
    /// `routes/session/index.tsx:2061`). Upstream defaults to `true`; an
    /// explicit non-boolean is reported as a bounded diagnostic.
    pub fn session_tps(cli_path: Option<&Path>, inline: Option<&str>) -> (bool, Vec<String>) {
        fn apply(text: &str, selected: &mut bool, diagnostics: &mut Vec<String>, label: &str) {
            if text.len() > MAX_CONFIG_BYTES { diagnostics.push(format!("{label} configuration exceeds size limit")); return; }
            let clean = match strip_jsonc(text) { Ok(clean) => clean, Err(_) => { diagnostics.push(format!("invalid {label} configuration")); return; } };
            let value = match serde_json::from_str::<Value>(&clean) {
                Ok(value) => value,
                Err(_) => { diagnostics.push(format!("invalid {label} configuration")); return; }
            };
            match value.get("session").and_then(|session| session.get("tps")) {
                Some(Value::Bool(enabled)) => *selected = *enabled,
                Some(_) => diagnostics.push("invalid session.tps value".to_owned()),
                None => {}
            }
        }
        let mut selected = true;
        let mut diagnostics = Vec::new();
        if let Some(path) = cli_path { let mut budget = crate::native_ui_budget::ReadBudget::default(); match budget.read(path, MAX_CONFIG_BYTES, "CLI config") { Ok(Some(text)) => apply(&text, &mut selected, &mut diagnostics, "disk CLI"), Ok(None) => {}, Err(_) => diagnostics.push("unable to read bounded global CLI configuration".into()) } }
        if let Some(text) = inline { apply(text, &mut selected, &mut diagnostics, "inline CLI"); }
        (selected, diagnostics)
    }

    /// `config.prompt.image_preview` (`config/index.tsx:147`, consumed at
    /// `component/prompt/index.tsx:1699`). Upstream defaults to `true`; an
    /// explicit non-boolean is reported as a bounded diagnostic.
    pub fn prompt_image_preview(cli_path: Option<&Path>, inline: Option<&str>) -> (bool, Vec<String>) {
        fn apply(text: &str, selected: &mut bool, diagnostics: &mut Vec<String>, label: &str) {
            if text.len() > MAX_CONFIG_BYTES { diagnostics.push(format!("{label} configuration exceeds size limit")); return; }
            let clean = match strip_jsonc(text) { Ok(clean) => clean, Err(_) => { diagnostics.push(format!("invalid {label} configuration")); return; } };
            let value = match serde_json::from_str::<Value>(&clean) {
                Ok(value) => value,
                Err(_) => { diagnostics.push(format!("invalid {label} configuration")); return; }
            };
            match value.get("prompt").and_then(|prompt| prompt.get("image_preview")) {
                Some(Value::Bool(enabled)) => *selected = *enabled,
                Some(_) => diagnostics.push("invalid prompt.image_preview value".to_owned()),
                None => {}
            }
        }
        let mut selected = true;
        let mut diagnostics = Vec::new();
        if let Some(path) = cli_path { let mut budget = crate::native_ui_budget::ReadBudget::default(); match budget.read(path, MAX_CONFIG_BYTES, "CLI config") { Ok(Some(text)) => apply(&text, &mut selected, &mut diagnostics, "disk CLI"), Ok(None) => {}, Err(_) => diagnostics.push("unable to read bounded global CLI configuration".into()) } }
        if let Some(text) = inline { apply(text, &mut selected, &mut diagnostics, "inline CLI"); }
        (selected, diagnostics)
    }

    /// `config.diffs.view` (`config/index.tsx:126`). Upstream defaults to
    /// `auto`; disk config applies first and inline config overrides it.
    pub fn diffs_view(
        cli_path: Option<&Path>,
        inline: Option<&str>,
    ) -> (crate::native_diff_view::DiffViewPreference, Vec<String>) {
        fn apply(
            text: &str,
            selected: &mut crate::native_diff_view::DiffViewPreference,
            diagnostics: &mut Vec<String>,
            label: &str,
        ) {
            if text.len() > MAX_CONFIG_BYTES {
                diagnostics.push(format!("{label} configuration exceeds size limit"));
                return;
            }
            let clean = match strip_jsonc(text) {
                Ok(clean) => clean,
                Err(_) => {
                    diagnostics.push(format!("invalid {label} configuration"));
                    return;
                }
            };
            let value = match serde_json::from_str::<Value>(&clean) {
                Ok(value) => value,
                Err(_) => {
                    diagnostics.push(format!("invalid {label} configuration"));
                    return;
                }
            };
            match value.get("diffs").and_then(|diffs| diffs.get("view")) {
                Some(Value::String(view)) => match view.as_str() {
                    "auto" => *selected = crate::native_diff_view::DiffViewPreference::Auto,
                    "split" => *selected = crate::native_diff_view::DiffViewPreference::Split,
                    "unified" => *selected = crate::native_diff_view::DiffViewPreference::Unified,
                    _ => diagnostics.push("invalid diffs.view value".to_owned()),
                },
                Some(_) => diagnostics.push("invalid diffs.view value".to_owned()),
                None => {}
            }
        }
        let mut selected = crate::native_diff_view::DiffViewPreference::Auto;
        let mut diagnostics = Vec::new();
        if let Some(path) = cli_path {
            let mut budget = crate::native_ui_budget::ReadBudget::default();
            match budget.read(path, MAX_CONFIG_BYTES, "CLI config") {
                Ok(Some(text)) => apply(&text, &mut selected, &mut diagnostics, "disk CLI"),
                Ok(None) => {}
                Err(_) => diagnostics.push("unable to read bounded global CLI configuration".into()),
            }
        }
        if let Some(text) = inline {
            apply(text, &mut selected, &mut diagnostics, "inline CLI");
        }
        (selected, diagnostics)
    }
    /// Load the V2 global/inline CLI theme. The inline document overlays the
    /// global document, matching upstream CLI config merge semantics. Legacy
    /// `tui.json`/kv theme remains the caller's lower-precedence fallback.
    pub fn cli_theme(
        cli_path: Option<&Path>,
        inline: Option<&str>,
    ) -> (Option<(String, String)>, Vec<String>) {
        let mut merged: Option<(Option<String>, Option<String>)> = None;
        let mut diagnostics = Vec::new();
        if let Some(path) = cli_path {
            let mut budget = crate::native_ui_budget::ReadBudget::default();
            match budget.read(path, MAX_CONFIG_BYTES, "CLI config") {
                Ok(Some(text)) => match parse_cli_theme(&text) {
                    Ok(value) => merge_cli_theme(&mut merged, value),
                    Err(error) => diagnostics.push(error),
                },
                Ok(None) => {}
                Err(_) => diagnostics.push("unable to read bounded global CLI configuration".into()),
            }
        }
        if let Some(text) = inline.filter(|text| !text.is_empty()) {
            if text.len() > MAX_CONFIG_BYTES {
                diagnostics.push("inline CLI configuration exceeds size limit".into());
            } else {
                match parse_cli_theme(text) {
                Ok(value) => merge_cli_theme(&mut merged, value),
                    Err(error) => diagnostics.push(error),
                }
            }
        }
        let selected = merged.and_then(|(name, mode)| {
            Some((name.unwrap_or_else(|| "opencode".into()), mode.unwrap_or_else(|| "system".into())))
        });
        (selected, diagnostics)
    }

    /// THEME-10: resolve the live "new-designs" experiment flag
    /// (`config/index.tsx:383-386` `useNewDesigns()`, i.e. the root
    /// `experimental["new-designs"]` boolean of the CLI config). Disk config
    /// applies first, an inline override second, and the last valid value wins.
    /// This is the exact document the TUI context's `useConfig().data` resolves
    /// (`packages/cli/src/config/config.ts:29-31,68-83`), so it gates the
    /// temporary `opencode2` theme source precisely as upstream does.
    pub fn new_designs_enabled(cli_path: Option<&Path>, inline: Option<&str>) -> bool {
        fn read<F: FnOnce(&Value) -> bool>(text: &str, get: F) -> Option<bool> {
            let value: Value = serde_json::from_str(&strip_jsonc(text).ok()?).ok()?;
            Some(get(&value))
        }
        let mut selected: Option<bool> = None;
        if let Some(path) = cli_path {
            let mut budget = crate::native_ui_budget::ReadBudget::default();
            if let Ok(Some(text)) = budget.read(path, MAX_CONFIG_BYTES, "CLI config") {
                if let Some(value) =
                    read(&text, crate::native_theme_runtime::config_new_designs)
                {
                    selected = Some(value);
                }
            }
        }
        if let Some(text) = inline.filter(|text| !text.is_empty() && text.len() <= MAX_CONFIG_BYTES)
        {
            if let Some(value) = read(text, crate::native_theme_runtime::config_new_designs) {
                selected = Some(value);
            }
        }
        selected.unwrap_or(false)
    }

    /// DIALOG-16: the number of root `experimental` entries in the CLI config,
    /// the live source for the experiments picker (`config/index.tsx`). Disk
    /// config applies first, an inline override second; the last document that
    /// carries an `experimental` object wins. An absent map counts as zero.
    pub fn experimental_count(cli_path: Option<&Path>, inline: Option<&str>) -> usize {
        fn read(text: &str) -> Option<usize> {
            let value: Value = serde_json::from_str(&strip_jsonc(text).ok()?).ok()?;
            value
                .get("experimental")
                .and_then(Value::as_object)
                .map(serde_json::Map::len)
        }
        let mut selected: Option<usize> = None;
        if let Some(path) = cli_path {
            let mut budget = crate::native_ui_budget::ReadBudget::default();
            if let Ok(Some(text)) = budget.read(path, MAX_CONFIG_BYTES, "CLI config") {
                if let Some(value) = read(&text) {
                    selected = Some(value);
                }
            }
        }
        if let Some(text) = inline.filter(|text| !text.is_empty() && text.len() <= MAX_CONFIG_BYTES)
        {
            if let Some(value) = read(text) {
                selected = Some(value);
            }
        }
        selected.unwrap_or(0)
    }

    /// TAB-04: resolve pinned upstream `tabs.indicators` (`config/index.tsx:199`,
    /// default `"status"` at `:336`; `session-tabs.tsx:525`). Disk config
    /// applies first, an inline override second, and the last document that
    /// actually carries the key wins — a document that omits it does not clobber
    /// a lower-precedence value.
    pub fn tab_indicators(
        cli_path: Option<&Path>,
        inline: Option<&str>,
    ) -> crate::native_tabs::TabIndicators {
        let mut selected: Option<crate::native_tabs::TabIndicators> = None;
        if let Some(path) = cli_path {
            let mut budget = crate::native_ui_budget::ReadBudget::default();
            if let Ok(Some(text)) = budget.read(path, MAX_CONFIG_BYTES, "CLI config") {
                if let Some(value) = parse_cli_tab_indicators(&text) {
                    selected = Some(value);
                }
            }
        }
        if let Some(text) = inline.filter(|text| !text.is_empty() && text.len() <= MAX_CONFIG_BYTES)
        {
            if let Some(value) = parse_cli_tab_indicators(text) {
                selected = Some(value);
            }
        }
        selected.unwrap_or(crate::native_tabs::TabIndicators::Status)
    }

    pub fn tab_orientation(
        cli_path: Option<&Path>,
        inline: Option<&str>,
    ) -> (crate::native_navigation::TabOrientation, Vec<String>) {
        let mut orientation = crate::native_navigation::TabOrientation::Horizontal;
        let mut diagnostics = Vec::new();
        if let Some(path) = cli_path {
            let mut budget = crate::native_ui_budget::ReadBudget::default();
            match budget.read(path, MAX_CONFIG_BYTES, "CLI config") {
                Ok(Some(text)) => match parse_cli_tab_orientation(&text) {
                    Ok(Some(layout)) => orientation = layout,
                    Ok(None) => {}
                    Err(()) => diagnostics.push("invalid global CLI tab configuration".into()),
                },
                Ok(None) => {}
                Err(_) => diagnostics.push("unable to read bounded global CLI configuration".into()),
            }
        }
        if let Some(text) = inline.filter(|text| !text.is_empty()) {
            match parse_cli_tab_orientation(text) {
                Ok(Some(layout)) => orientation = layout,
                Ok(None) => {}
                Err(()) => diagnostics.push("invalid or oversized inline CLI tab configuration".into()),
            }
        }
        (orientation, diagnostics)
    }

    /// Resolve pinned upstream `tabs.scope` (`config/index.tsx:193-195`, default
    /// `cwd` at `:334`). Disk config applies first, an inline override second;
    /// the last valid value wins and an invalid value only adds a diagnostic.
    pub fn tab_scope<'a>(
        cli_path: Option<&Path>,
        inline: Option<&str>,
        cwd: &'a str,
    ) -> (TabPersistenceScope<'a>, Vec<String>) {
        let mut global = false;
        let mut diagnostics = Vec::new();
        if let Some(path) = cli_path {
            let mut budget = crate::native_ui_budget::ReadBudget::default();
            match budget.read(path, MAX_CONFIG_BYTES, "CLI config") {
                Ok(Some(text)) => match parse_cli_tab_scope(&text) {
                    Ok(Some(value)) => global = value,
                    Ok(None) => {}
                    Err(()) => diagnostics.push("invalid global CLI tab scope configuration".into()),
                },
                Ok(None) => {}
                Err(_) => diagnostics.push("unable to read bounded global CLI configuration".into()),
            }
        }
        if let Some(text) = inline.filter(|text| !text.is_empty()) {
            match parse_cli_tab_scope(text) {
                Ok(Some(value)) => global = value,
                Ok(None) => {}
                Err(()) => diagnostics.push("invalid or oversized inline CLI tab scope configuration".into()),
            }
        }
        (
            if global {
                TabPersistenceScope::Global
            } else {
                TabPersistenceScope::Cwd(cwd)
            },
            diagnostics,
        )
    }

    pub fn appearance(state_path: &Path) -> Result<crate::native_box_style::BoxAppearance, String> {
        let state = read_state(state_path)?;
        state
            .get("box_appearance")
            .cloned()
            .map(|value| {
                serde_json::from_value(value)
                    .map_err(|error| format!("invalid box appearance: {error}"))
            })
            .transpose()
            .map(|value| value.unwrap_or_default())
    }
    pub fn persist_appearance(
        state_path: &Path,
        appearance: &crate::native_box_style::BoxAppearance,
    ) -> Result<(), String> {
        appearance.validate()?;
        let value = serde_json::to_value(appearance).map_err(|error| error.to_string())?;
        update_state(state_path, "box_appearance", Some(value))
    }
    /// Load configs in increasing precedence (global first, project last) and
    /// then read OpenTUI state from the explicit `.../opencode/kv.json` path.
    /// Invalid files are skipped independently, matching upstream's last-good
    /// config merge behavior; diagnostics are returned for the caller to log.
    pub fn load(config_paths: &[PathBuf], state_path: &Path) -> Result<Self, String> {
        crate::native_ui_budget::check_paths(config_paths)?;
        let mut budget = crate::native_ui_budget::ReadBudget::default();
        let mut prompt_width = None;
        let mut prompt_paste = None;
        let mut configured_theme = None;
        let mut cursor: Option<Value> = None;
        let mut animations: Option<bool> = None;
        let mut keybinds: Option<Value> = None;
        let mut diagnostics = Vec::new();
        for path in config_paths {
            let loaded = (|| -> Result<Option<(Option<PromptWidth>, Option<String>, Option<String>, Option<Value>, Option<bool>, Option<Value>)>, String> {
                let Some(text) = budget.read(path, MAX_CONFIG_BYTES, "TUI config")? else {
                    return Ok(None);
                };
                let mut value: Value = serde_json::from_str(&strip_jsonc(&text)?)
                    .map_err(|_| format!("invalid TUI config: {}", path.display()))?;
                if let Some(nested) = value.as_object_mut().and_then(|o| o.remove("tui")) {
                    if let Some(nested) = nested.as_object() {
                        let mut merged = nested.clone();
                        if let Some(outer) = value.as_object() {
                            merged.extend(outer.clone());
                        }
                        value = Value::Object(merged);
                    }
                }
                let Some(object) = value.as_object() else {
                    return Err(format!(
                        "TUI config root is not an object: {}",
                        path.display()
                    ));
                };
                let theme = object
                    .get("theme")
                    .map(|theme| {
                        theme
                            .as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| format!("invalid TUI config theme: {}", path.display()))
                    })
                    .transpose()?;
                let width = object
                    .get("prompt")
                    .map(|prompt| {
                        let prompt = prompt.as_object().ok_or_else(|| {
                            format!("invalid TUI config prompt: {}", path.display())
                        })?;
                        prompt
                            .get("max_width")
                            .map(|max_width| match max_width {
                                Value::String(s) if s == "auto" => Ok(PromptWidth::Auto),
                                Value::Number(n) => n
                                    .as_u64()
                                    .filter(|n| *n > 0 && *n <= u16::MAX as u64)
                                    .map(|n| PromptWidth::Fixed(n as u16))
                                    .ok_or_else(|| {
                                        format!("invalid prompt.max_width: {}", path.display())
                                    }),
                                _ => Err(format!("invalid prompt.max_width: {}", path.display())),
                            })
                            .transpose()
                    })
                    .transpose()?
                    .flatten();
                // `prompt.paste` (`config/index.tsx:144`). The same merged
                // `prompt` object that already carries `max_width`; only the
                // two literal values are accepted, everything else is a
                // malformed-config diagnostic exactly like `max_width`.
                let paste = object
                    .get("prompt")
                    .map(|prompt| {
                        let prompt = prompt.as_object().ok_or_else(|| {
                            format!("invalid TUI config prompt: {}", path.display())
                        })?;
                        prompt
                            .get("paste")
                            .map(|value| match value {
                                Value::String(s) if s == "compact" || s == "full" => {
                                    Ok(s.clone())
                                }
                                _ => Err(format!("invalid prompt.paste: {}", path.display())),
                            })
                            .transpose()
                    })
                    .transpose()?
                    .flatten();
                // `tui.cursor` (`config/index.tsx:60-67`). Stored raw so the
                // caller parses it with the theme runtime's `CursorConfig`
                // (validated there, THEME-14).
                let cursor = object.get("cursor").cloned();
                // TAB-23: raw `keybinds` overrides (`config/index.tsx:78`
                // `TuiKeybind.KeybindOverrides`). Raw like `cursor`;
                // per-entry validation happens in the
                // `native_navigation::tab_bindings` resolver.
                let keybinds = object.get("keybinds").cloned();
                // `tui.animations` (`config/index.tsx:250`). A non-boolean is a
                // malformed config value, matching the other typed keys.
                let animations = object
                    .get("animations")
                    .map(|value| {
                        value.as_bool().ok_or_else(|| {
                            format!("invalid TUI config animations: {}", path.display())
                        })
                    })
                    .transpose()?;
                Ok(Some((width, paste, theme, cursor, animations, keybinds)))
            })();

            match loaded {
                Ok(Some((width, paste, theme, cursor_value, animations_value, keybinds_value))) => {
                    if let Some(width) = width {
                        prompt_width = Some(width);
                    }
                    if paste.is_some() {
                        prompt_paste = paste;
                    }
                    if let Some(theme) = theme {
                        configured_theme = Some(theme);
                    }
                    if cursor_value.is_some() {
                        cursor = cursor_value;
                    }
                    if animations_value.is_some() {
                        animations = animations_value;
                    }
                    if keybinds_value.is_some() {
                        keybinds = keybinds_value;
                    }
                }
                Ok(None) => {}
                Err(error) => diagnostics.push(error),
            }
        }

        let state = match read_state(state_path) {
            Ok(state) => state,
            Err(error) => {
                diagnostics.push(error);
                Map::new()
            }
        };
        let stored_theme = state
            .get("theme")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let sidebar_hidden = state.get("sidebar").and_then(Value::as_str) == Some("hide");
        // SIDE-12: default `false`, exactly upstream's `initial: { dismissed: false }`.
        let onboarding_dismissed = state
            .get("getting-started")
            .and_then(Value::as_object)
            .and_then(|value| value.get("dismissed"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        // SIDE-09: persisted MCP disclosure. `None` when never stored, so the
        // view keeps the upstream `initial: { open: true }` default rather than
        // a fabricated collapse. Read from the plugin-namespaced object the
        // writer below produces: `{"plugin.opencode.sidebar.mcp.view": {"open": bool}}`.
        let mcp_sidebar_open = state
            .get(MCP_SIDEBAR_VIEW_KEY)
            .and_then(Value::as_object)
            .and_then(|value| value.get("open"))
            .and_then(Value::as_bool);
        Ok(Self {
            prompt_width,
            prompt_paste,
            theme: configured_theme.or(stored_theme),
            sidebar_hidden,
            mcp_sidebar_open,
            theme_mode: state
                .get("theme_mode")
                .and_then(Value::as_str)
                .map(str::to_owned),
            theme_mode_lock: state
                .get("theme_mode_lock")
                .and_then(Value::as_str)
                .map(str::to_owned),
            vertical_tabs_width: state.get("layout").and_then(Value::as_object)
                .and_then(|layout| layout.get("verticalTabsWidth"))
                .and_then(Value::as_u64)
                .map(|width| width.clamp(5, 72) as u16),
            onboarding_dismissed,
            cursor,
            animations,
            keybinds,
            diagnostics,
        })
    }

    /// Persist the exact upstream sidebar value (`hide` or `auto`) in `kv.json`.
    pub fn persist_sidebar(state_path: &Path, hidden: bool) -> Result<(), String> {
        update_state(
            state_path,
            "sidebar",
            Some(Value::String(if hidden { "hide" } else { "auto" }.into())),
        )
    }

    /// SIDE-09: persist the MCP sidebar disclosure under the exact upstream
    /// plugin-namespaced key and value shape
    /// (`sidebar/mcp.tsx:6/25-31` `updateView((draft) => { draft.open = !draft.open })`
    /// over `storage.store("view", { initial: { open: true } })`, namespaced by
    /// `plugin.${input.id}.${key}` at `plugin/api.tsx:201` with
    /// `id = "opencode.sidebar.mcp"` at `sidebar/mcp.tsx:96`): the entry
    /// `"plugin.opencode.sidebar.mcp.view": {"open": bool}`. Writes only that
    /// key, preserving every unrelated KV entry, so a `todo.tsx:7` collapse
    /// (its own namespace) is never clobbered and vice versa.
    pub fn persist_mcp_sidebar_open(state_path: &Path, open: bool) -> Result<(), String> {
        update_state(
            state_path,
            MCP_SIDEBAR_VIEW_KEY,
            Some(serde_json::json!({ "open": open })),
        )
    }

    /// SIDE-12: persist the getting-started dismissal under the exact upstream
    /// key and value shape (`footer.tsx:11-16` `storage.store("getting-started",
    /// { initial: { dismissed: false } })`): `"getting-started": {"dismissed":
    /// bool}`. Writes only that key, preserving every unrelated KV entry, and
    /// mirrors the atomic floored state path used by the other preferences.
    pub fn dismiss_onboarding(state_path: &Path, dismissed: bool) -> Result<(), String> {
        update_state(
            state_path,
            "getting-started",
            Some(serde_json::json!({ "dismissed": dismissed })),
        )
    }

    /// Persist an upstream theme selection (or `None` to clear it), preserving
    /// unrelated KV data. Theme validity remains the theme chooser's concern.
    pub fn persist_theme(state_path: &Path, theme: Option<&str>) -> Result<(), String> {
        update_state(
            state_path,
            "theme",
            theme.map(|s| Value::String(s.to_owned())),
        )
    }

    pub fn persist_theme_selection(
        state_path: &Path,
        theme: &str,
        mode: &str,
    ) -> Result<(), String> {
        if theme.is_empty()
            || theme.len() > 64
            || theme.chars().any(char::is_control)
            || !matches!(mode, "dark" | "light")
        {
            return Err("invalid theme selection".into());
        }
        mutate_state(state_path, |state| {
            state.insert("theme".into(), Value::String(theme.into()));
            state.insert("theme_mode".into(), Value::String(mode.into()));
        })
    }

    pub fn persist_tab_orientation(
        cli_path: &Path,
        orientation: crate::native_navigation::TabOrientation,
    ) -> Result<(), String> {
        let location = StateLocation::open(cli_path, true)?.ok_or("CLI config parent unavailable")?;
        let mut state = location.read_with_limit(MAX_CONFIG_BYTES, true)?;
        let tabs = state.entry("tabs").or_insert_with(|| Value::Object(Map::new()));
        let tabs = tabs.as_object_mut().ok_or("CLI tabs configuration is not an object")?;
        tabs.insert("layout".into(), Value::String(match orientation {
            crate::native_navigation::TabOrientation::Horizontal => "horizontal",
            crate::native_navigation::TabOrientation::Vertical => "vertical",
        }.into()));
        location.write_with_limit(state, MAX_CONFIG_BYTES)
    }

    pub fn persist_vertical_tabs_width(state_path: &Path, width: u16) -> Result<(), String> {
        if !(5..=72).contains(&width) { return Err("invalid vertical tabs width".into()); }
        mutate_state(state_path, |state| {
            let layout = state.entry("layout").or_insert_with(|| Value::Object(Map::new()));
            if let Some(layout) = layout.as_object_mut() { layout.insert("verticalTabsWidth".into(), Value::from(width)); }
        })
    }

    /// Load the persisted open-tab layout for `scope` from `state/tabs.json`.
    /// The on-disk shape mirrors upstream `context/session-tabs.tsx:72-102`
    /// (`{"cwd": {"<cwd>": {"tabs": [{"sessionID","title"}]}}}`); only order
    /// and titles are read back, and malformed rows are skipped rather than
    /// failing the whole restore.
    pub fn load_session_tabs<'a, S: Into<TabPersistenceScope<'a>>>(
        path: &Path,
        scope: S,
    ) -> Result<crate::native_navigation::PersistedTabs, String> {
        let scope = scope.into();
        use crate::native_navigation::{PersistedTabs, SessionRef};
        let Some(location) = StateLocation::open(path, false)? else {
            return Ok(PersistedTabs::default());
        };
        let state = location.read()?;
        let rows = match scope {
            TabPersistenceScope::Global => state.get("global"),
            TabPersistenceScope::Cwd(cwd) => state
                .get("cwd")
                .and_then(Value::as_object)
                .and_then(|scopes| scopes.get(cwd)),
        }
        .and_then(Value::as_object)
        .and_then(|entry| entry.get("tabs"))
        .and_then(Value::as_array);
        let Some(rows) = rows else {
            return Ok(PersistedTabs::default());
        };
        let mut persisted = PersistedTabs::default();
        for row in rows.iter().take(crate::native_navigation::MAX_TABS) {
            let Some(id) = row.get("sessionID").and_then(Value::as_str) else {
                continue;
            };
            if !Self::valid_tab_field(id, MAX_SESSION_ID_BYTES) {
                continue;
            }
            let session = SessionRef(id.to_owned());
            if persisted.order.iter().any(|open| *open == session) {
                continue;
            }
            persisted.order.push(session.clone());
            if let Some(title) = row.get("title").and_then(Value::as_str) {
                if Self::valid_tab_field(title, MAX_TAB_TITLE_BYTES) {
                    persisted.titles.push((session, title.to_owned()));
                }
            }
        }
        Ok(persisted)
    }

    /// Replace the persisted tab layout for `scope`, retaining other scopes
    /// up to `MAX_TAB_SCOPES`. Invalid entries are dropped at the boundary so
    /// the store can never grow without bound.
    pub fn persist_session_tabs<'a, S: Into<TabPersistenceScope<'a>>>(
        path: &Path,
        scope: S,
        persisted: &crate::native_navigation::PersistedTabs,
    ) -> Result<(), String> {
        let scope = scope.into();
        if let TabPersistenceScope::Cwd(cwd) = scope {
            if cwd.is_empty() || cwd.len() > 4096 || cwd.chars().any(char::is_control) {
                return Err("invalid tabs scope".into());
            }
        }
        let location = StateLocation::open(path, true)?.ok_or("tabs state parent unavailable")?;
        let mut state = location.read()?;
        let rows: Vec<Value> = persisted
            .order
            .iter()
            .take(crate::native_navigation::MAX_TABS)
            .filter(|session| Self::valid_tab_field(&session.0, MAX_SESSION_ID_BYTES))
            .map(|session| {
                let mut row = Map::new();
                row.insert("sessionID".into(), Value::String(session.0.clone()));
                if let Some((_, title)) = persisted
                    .titles
                    .iter()
                    .find(|(open, _)| open == session)
                    .filter(|(_, title)| Self::valid_tab_field(title, MAX_TAB_TITLE_BYTES))
                {
                    row.insert("title".into(), Value::String(title.clone()));
                }
                Value::Object(row)
            })
            .collect();
        let mut entry = Map::new();
        entry.insert("tabs".into(), Value::Array(rows));
        entry.insert("unread".into(), Value::Object(Map::new()));
        match scope {
            TabPersistenceScope::Global => {
                state.insert("global".into(), Value::Object(entry));
            }
            TabPersistenceScope::Cwd(cwd) => {
                let scopes = state.entry("cwd").or_insert_with(|| Value::Object(Map::new()));
                let scopes = scopes.as_object_mut().ok_or("tabs state cwd is not an object")?;
                while scopes.len() >= MAX_TAB_SCOPES && !scopes.contains_key(cwd) {
                    let Some(oldest) = scopes.keys().next().cloned() else {
                        break;
                    };
                    scopes.remove(&oldest);
                }
                scopes.insert(cwd.to_owned(), Value::Object(entry));
            }
        }
        location.write(state)
    }

    fn valid_tab_field(value: &str, limit: usize) -> bool {
        !value.is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
    }

}

pub(crate) fn read_bounded(
    path: &Path,
    limit: usize,
    label: &str,
) -> Result<Option<String>, String> {
    let meta = match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(format!("unable to inspect {label}: {}", path.display())),
        Ok(m) => m,
    };
    if !meta.is_file() || meta.len() > limit as u64 {
        return Err(format!(
            "{label} is not a bounded regular file: {}",
            path.display()
        ));
    }
    let file =
        File::open(path).map_err(|_| format!("unable to read {label}: {}", path.display()))?;
    let opened = file
        .metadata()
        .map_err(|_| format!("unable to inspect {label}: {}", path.display()))?;
    if !opened.is_file() || opened.dev() != meta.dev() || opened.ino() != meta.ino() {
        return Err(format!("{label} changed while opening: {}", path.display()));
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| format!("unable to read {label}: {}", path.display()))?;
    if bytes.len() > limit {
        return Err(format!("{label} exceeds size limit: {}", path.display()));
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| format!("{label} is not UTF-8: {}", path.display()))
}

fn read_state(path: &Path) -> Result<Map<String, Value>, String> {
    match StateLocation::open(path, false)? {
        Some(location) => location.read(),
        None => Ok(Map::new()),
    }
}

fn update_state(path: &Path, key: &str, value: Option<Value>) -> Result<(), String> {
    mutate_state(path, |state| {
        if let Some(value) = value {
            state.insert(key.to_owned(), value);
        } else {
            state.remove(key);
        }
    })
}

fn mutate_state(path: &Path, change: impl FnOnce(&mut Map<String, Value>)) -> Result<(), String> {
    let location = StateLocation::open(path, true)?.ok_or("TUI state parent unavailable")?;
    let mut state = location.read()?;
    change(&mut state);
    location.write(state)
}

/// All state access is relative to a retained no-follow directory descriptor.
/// A path replacement cannot redirect open/create/rename/unlink operations.
struct StateLocation {
    path: PathBuf,
    directory: File,
    name: std::ffi::OsString,
    original: Option<(u64, u64)>,
}

impl StateLocation {
    fn open(path: &Path, create: bool) -> Result<Option<Self>, String> {
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|_| "TUI state working directory unavailable")?
                .join(path)
        };
        // macOS exposes these system-owned aliases as symlinks. Rewrite only
        // their exact leading components; arbitrary caller symlinks still go
        // through the anchored NOFOLLOW walk below and are rejected.
        #[cfg(target_os = "macos")]
        let path = {
            let mut physical = path.clone();
            for (alias, target) in [("/var", "/private/var"), ("/tmp", "/private/tmp")] {
                if let Ok(relative) = path.strip_prefix(alias) {
                    physical = Path::new(target).join(relative);
                    break;
                }
            }
            physical
        };
        if path.as_os_str().len() > 32768 || path.components().count() > 128 {
            return Err("TUI state path exceeds bounds".into());
        }
        let name = path
            .file_name()
            .ok_or("invalid TUI state filename")?
            .to_owned();
        let parent = path.parent().ok_or("TUI state parent unavailable")?;
        let flags = rfs::OFlags::RDONLY
            | rfs::OFlags::DIRECTORY
            | rfs::OFlags::NOFOLLOW
            | rfs::OFlags::CLOEXEC;
        let mut directory = File::from(
            rfs::open("/", flags, rfs::Mode::empty())
                .map_err(|_| "unable to anchor TUI state directory")?,
        );
        for component in parent.components() {
            let component = match component {
                Component::RootDir | Component::CurDir => continue,
                Component::Normal(name) => name,
                _ => return Err("TUI state path traversal refused".into()),
            };
            let opened = match rfs::openat(&directory, component, flags, rfs::Mode::empty()) {
                Ok(fd) => fd,
                Err(rustix::io::Errno::NOENT) if create => {
                    match rfs::mkdirat(&directory, component, rfs::Mode::from_raw_mode(0o700)) {
                        Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                        Err(_) => {
                            return Err("unable to create anchored TUI state directory".into());
                        }
                    }
                    rfs::openat(&directory, component, flags, rfs::Mode::empty())
                        .map_err(|_| "unsafe TUI state directory replacement")?
                }
                Err(rustix::io::Errno::NOENT) => return Ok(None),
                Err(_) => return Err("unsafe TUI state directory component".into()),
            };
            directory = File::from(opened);
        }
        let metadata = directory
            .metadata()
            .map_err(|_| "unable to inspect TUI state parent")?;
        if metadata.uid() != getuid().as_raw() || metadata.mode() & 0o022 != 0 {
            return Err("TUI state parent ownership or permissions refused".into());
        }
        let original = Self::identity(&directory, &name)?;
        Ok(Some(Self {
            path,
            directory,
            name,
            original,
        }))
    }

    fn identity(directory: &File, name: &std::ffi::OsStr) -> Result<Option<(u64, u64)>, String> {
        match rfs::statat(directory, name, rfs::AtFlags::SYMLINK_NOFOLLOW) {
            Err(rustix::io::Errno::NOENT) => Ok(None),
            Err(_) => Err("unable to inspect anchored TUI state".into()),
            Ok(stat) => {
                if rfs::FileType::from_raw_mode(stat.st_mode) != rfs::FileType::RegularFile
                    || stat.st_nlink != 1
                    || stat.st_uid != getuid().as_raw()
                {
                    return Err("unsafe TUI state destination".into());
                }
                Ok(Some((stat.st_dev as u64, stat.st_ino as u64)))
            }
        }
    }

    fn read(&self) -> Result<Map<String, Value>, String> {
        self.read_with_limit(MAX_STATE_BYTES, false)
    }

    fn read_with_limit(&self, limit: usize, jsonc: bool) -> Result<Map<String, Value>, String> {
        if self.original.is_none() { return Ok(Map::new()); }
        let file = File::from(rfs::openat(&self.directory, &self.name,
            rfs::OFlags::RDONLY | rfs::OFlags::NOFOLLOW | rfs::OFlags::NONBLOCK | rfs::OFlags::CLOEXEC,
            rfs::Mode::empty()).map_err(|_| "unable to open anchored state")?);
        let metadata = file.metadata().map_err(|_| "unable to inspect opened state")?;
        if !metadata.is_file() || metadata.nlink() != 1 || metadata.uid() != getuid().as_raw()
            || Some((metadata.dev(), metadata.ino())) != self.original || metadata.len() > limit as u64 {
            return Err("state changed or exceeds bounds".into());
        }
        let mut bytes = Vec::new();
        file.take(limit as u64 + 1).read_to_end(&mut bytes).map_err(|_| "unable to read anchored state")?;
        if bytes.len() > limit { return Err("state exceeds bounds".into()); }
        if jsonc {
            let text = String::from_utf8(bytes).map_err(|_| "CLI configuration is not UTF-8")?;
            return parse_jsonc_value(&text)?.as_object().cloned().ok_or("CLI configuration root is not an object".into());
        }
        serde_json::from_slice(&bytes).map_err(|_| "invalid TUI state JSON".into())
    }

    fn validate(&self) -> Result<(), String> {
        let current = Self::open(&self.path, false)?.ok_or("TUI state parent disappeared")?;
        let current_meta = current
            .directory
            .metadata()
            .map_err(|_| "unable to inspect TUI state parent")?;
        let held_meta = self
            .directory
            .metadata()
            .map_err(|_| "unable to inspect anchored parent")?;
        if (current_meta.dev(), current_meta.ino()) != (held_meta.dev(), held_meta.ino())
            || current.original != self.original
        {
            return Err("TUI state destination changed".into());
        }
        Ok(())
    }

    fn write(&self, state: Map<String, Value>) -> Result<(), String> {
        self.write_with_limit(state, MAX_STATE_BYTES)
    }

    fn write_with_limit(&self, state: Map<String, Value>, limit: usize) -> Result<(), String> {
        let bytes = serde_json::to_vec(&state).map_err(|_| "unable to encode TUI state")?;
        if bytes.len() > limit {
            return Err("TUI state exceeds size limit".into());
        }
        self.validate()?;
        let temporary = format!(
            ".kv.json.{}.{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        );
        let file = rfs::openat(
            &self.directory,
            temporary.as_str(),
            rfs::OFlags::WRONLY
                | rfs::OFlags::CREATE
                | rfs::OFlags::EXCL
                | rfs::OFlags::NOFOLLOW
                | rfs::OFlags::CLOEXEC,
            rfs::Mode::from_raw_mode(0o600),
        )
        .map_err(|_| "unable to create private anchored TUI state")?;
        let mut file = File::from(file);
        let result = (|| {
            file.write_all(&bytes)
                .map_err(|_| "unable to write TUI state")?;
            file.sync_all().map_err(|_| "unable to sync TUI state")?;
            self.validate()?;
            rfs::renameat(
                &self.directory,
                temporary.as_str(),
                &self.directory,
                &self.name,
            )
            .map_err(|_| "unable to replace anchored TUI state")?;
            self.directory
                .sync_all()
                .map_err(|_| "unable to sync TUI state parent")?;
            Ok(())
        })();
        if result.is_err() {
            let _ = rfs::unlinkat(&self.directory, temporary.as_str(), rfs::AtFlags::empty());
        }
        result
    }
}

#[cfg(test)]
mod anchored_race_tests {
    use super::*;
    use std::os::unix::fs::symlink;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let directory = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "oc-ui-anchor-race-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&directory).unwrap();
            Self(directory)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn parent_replacement_after_anchor_does_not_redirect_or_commit() {
        let fixture = Fixture::new();
        let path = fixture.0.join("state/kv.json");
        UiPreferences::persist_sidebar(&path, false).unwrap();
        let location = StateLocation::open(&path, false).unwrap().unwrap();
        let mut value = location.read().unwrap();
        value.insert("sidebar".into(), Value::String("hide".into()));
        let original_bytes = fs::read(&path).unwrap();
        let held = fixture.0.join("held");
        fs::rename(path.parent().unwrap(), &held).unwrap();
        let outside = fixture.0.join("outside");
        fs::create_dir(&outside).unwrap();
        symlink(&outside, path.parent().unwrap()).unwrap();
        assert!(location.write(value).is_err());
        assert!(!outside.join("kv.json").exists());
        assert_eq!(fs::read(held.join("kv.json")).unwrap(), original_bytes);
    }

    #[test]
    fn appearance_roundtrip_preserves_other_preferences_and_per_box_override() {
        let fixture = Fixture::new();
        let path = fixture.0.join("kv.json");
        fs::write(
            &path,
            br#"{"sidebar":"hide","theme":"user-theme","unrelated":{"keep":42}}"#,
        )
        .unwrap();
        let mut style = crate::native_box_style::BoxAppearance::default();
        style.preset = crate::native_box_style::RadiusPreset::IosSquircle;
        style.overrides.insert(
            crate::native_box_style::BoxId::ChatComposer,
            crate::native_box_style::BoxOverride {
                radius: Some(17),
                preset: Some(crate::native_box_style::RadiusPreset::Custom),
                ..Default::default()
            },
        );
        UiPreferences::persist_appearance(&path, &style).unwrap();
        assert_eq!(UiPreferences::appearance(&path).unwrap(), style);
        let saved: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(saved["sidebar"], "hide");
        assert_eq!(saved["theme"], "user-theme");
        assert_eq!(saved["unrelated"]["keep"], 42);
    }
    #[test]
    fn invalid_appearance_never_replaces_good_preferences() {
        let fixture = Fixture::new();
        let path = fixture.0.join("kv.json");
        let bytes = br#"{"theme":"preserved"}"#;
        fs::write(&path, bytes).unwrap();
        let mut style = crate::native_box_style::BoxAppearance::default();
        style.radius = 51;
        assert!(UiPreferences::persist_appearance(&path, &style).is_err());
        assert_eq!(fs::read(path).unwrap(), bytes);
    }

    #[test]
    fn destination_replacement_after_anchor_does_not_touch_symlink_target() {
        let fixture = Fixture::new();
        let path = fixture.0.join("kv.json");
        UiPreferences::persist_sidebar(&path, false).unwrap();
        let location = StateLocation::open(&path, false).unwrap().unwrap();
        let value = location.read().unwrap();
        let target = fixture.0.join("outside.json");
        let bytes = br#"{"untouched":true}"#;
        fs::write(&target, bytes).unwrap();
        fs::remove_file(&path).unwrap();
        symlink(&target, &path).unwrap();
        assert!(location.write(value).is_err());
        assert_eq!(fs::read(&target).unwrap(), bytes);
        assert!(fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink());
    }
}

/// Strip JSONC comments and trailing commas without interpreting comment-like
/// characters inside JSON strings. `serde_json` remains the sole JSON parser.
/// Parse one OpenCode TUI JSON/JSONC document with the same bounded parser
/// used by preference loading. Callers must not use plain JSON parsing here.
pub(crate) fn parse_jsonc_value(input: &str) -> Result<Value, String> {
    let cleaned = strip_jsonc(input)?;
    serde_json::from_str(&cleaned).map_err(|_| "invalid JSONC configuration".into())
}

fn strip_jsonc(input: &str) -> Result<String, String> {
    let b = input.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let (mut i, mut string, mut escape) = (0, false, false);
    while i < b.len() {
        if string {
            out.push(b[i]);
            if escape {
                escape = false;
            } else if b[i] == b'\\' {
                escape = true;
            } else if b[i] == b'"' {
                string = false;
            }
            i += 1;
            continue;
        }
        if b[i] == b'"' {
            string = true;
            out.push(b[i]);
            i += 1;
            continue;
        }
        if b[i] == b'/' && b.get(i + 1) == Some(&b'/') {
            i += 2;
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            let start = i;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 >= b.len() {
                return Err("unterminated JSONC comment".into());
            }
            // A comment is whitespace in JSONC; retain a separator even for
            // an empty block comment so `1/**/2` cannot become the token `12`.
            out.push(b' ');
            out.extend(
                b[start..i]
                    .iter()
                    .map(|c| if *c == b'\n' { b'\n' } else { b' ' }),
            );
            i += 2;
            continue;
        }
        if b[i] == b',' {
            let mut next = i + 1;
            loop {
                while next < b.len() && b[next].is_ascii_whitespace() {
                    next += 1;
                }
                if b.get(next..next + 2) == Some(b"//") {
                    next += 2;
                    while next < b.len() && b[next] != b'\n' {
                        next += 1;
                    }
                    continue;
                }
                if b.get(next..next + 2) == Some(b"/*") {
                    next += 2;
                    while next + 1 < b.len() && !(b[next] == b'*' && b[next + 1] == b'/') {
                        next += 1;
                    }
                    if next + 1 < b.len() {
                        next += 2;
                    }
                    continue;
                }
                break;
            }
            if matches!(b.get(next), Some(b'}' | b']')) {
                i += 1;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    if string {
        return Err("unterminated JSON string".into());
    }
    String::from_utf8(out).map_err(|_| "invalid UTF-8 in JSONC".into())
}

fn parse_cli_theme(text: &str) -> Result<Option<(Option<String>, Option<String>)>, String> {
    let text = strip_jsonc(text)?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|_| "invalid global CLI configuration".to_owned())?;
    let Some(object) = value.as_object() else {
        return Err("global CLI configuration root is not an object".into());
    };
    let Some(theme) = object.get("theme") else { return Ok(None) };
    let Some(theme) = theme.as_object() else {
        return Err("invalid CLI theme configuration: expected object".into());
    };
    let name = match theme.get("name") {
        None => None,
        Some(Value::String(name)) if !name.is_empty() && name.len() <= 64 && !name.chars().any(char::is_control) => Some(name.clone()),
        Some(_) => return Err("invalid CLI theme name".into()),
    };
    let mode = match theme.get("mode") {
        None => None,
        Some(Value::String(mode)) if matches!(mode.as_str(), "system" | "dark" | "light") => Some(mode.clone()),
        Some(_) => return Err("invalid CLI theme mode".into()),
    };
    Ok(Some((name, mode)))
}

fn merge_cli_theme(
    merged: &mut Option<(Option<String>, Option<String>)>,
    overlay: Option<(Option<String>, Option<String>)>,
) {
    let Some((name, mode)) = overlay else { return };
    let current = merged.get_or_insert((None, None));
    if name.is_some() { current.0 = name; }
    if mode.is_some() { current.1 = mode; }
}

fn parse_cli_tab_scope(text: &str) -> Result<Option<bool>, ()> {
    if text.len() > MAX_CONFIG_BYTES {
        return Err(());
    }
    let value: Value = serde_json::from_str(&strip_jsonc(text).map_err(|_| ())?).map_err(|_| ())?;
    let object = value.as_object().ok_or(())?;
    let Some(tabs) = object.get("tabs") else { return Ok(None) };
    let tabs = tabs.as_object().ok_or(())?;
    match tabs.get("scope") {
        None => Ok(None),
        Some(Value::String(scope)) if scope == "global" => Ok(Some(true)),
        Some(Value::String(scope)) if scope == "cwd" => Ok(Some(false)),
        Some(_) => Err(()),
    }
}

fn parse_cli_tab_indicators(text: &str) -> Option<crate::native_tabs::TabIndicators> {
    if text.len() > MAX_CONFIG_BYTES {
        return None;
    }
    let value: Value = serde_json::from_str(&strip_jsonc(text).ok()?).ok()?;
    if !crate::native_theme_runtime::config_tab_indicators_present(&value) {
        return None;
    }
    Some(crate::native_theme_runtime::config_tab_indicators(&value))
}

fn parse_cli_tab_orientation(text: &str) -> Result<Option<crate::native_navigation::TabOrientation>, ()> {
    if text.len() > MAX_CONFIG_BYTES {
        return Err(());
    }
    let value: Value = serde_json::from_str(&strip_jsonc(text).map_err(|_| ())?).map_err(|_| ())?;
    let object = value.as_object().ok_or(())?;
    let Some(tabs) = object.get("tabs") else { return Ok(None) };
    let tabs = tabs.as_object().ok_or(())?;
    match tabs.get("layout") {
        None => Ok(None),
        Some(Value::String(layout)) if layout == "horizontal" => Ok(Some(crate::native_navigation::TabOrientation::Horizontal)),
        Some(Value::String(layout)) if layout == "vertical" => Ok(Some(crate::native_navigation::TabOrientation::Vertical)),
        Some(_) => Err(()),
    }
}

#[cfg(test)]
mod cli_theme_tests {
    use super::*;

    #[test]
    fn v2_theme_reads_name_and_mode_without_rewriting_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        let original = r#"{"theme":{"name":"custom","mode":"light"},"keep":7}"#;
        fs::write(&path, original).unwrap();
        let (theme, diagnostics) = UiPreferences::cli_theme(Some(&path), None);
        assert_eq!(theme, Some(("custom".into(), "light".into())));
        assert!(diagnostics.is_empty());
        assert_eq!(fs::read_to_string(path).unwrap(), original);
    }

    #[test]
    fn inline_v2_theme_overlays_global_field_by_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        fs::write(&path, r#"{"theme":{"name":"global","mode":"dark"}}"#).unwrap();
        assert_eq!(UiPreferences::cli_theme(Some(&path), Some(r#"{"theme":{"mode":"light"}}"#)).0,
            Some(("global".into(), "light".into())));
    }

    #[test]
    fn malformed_theme_type_is_diagnostic_and_valid_inline_can_win() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        fs::write(&path, r#"{"theme":{"mode":7}}"#).unwrap();
        let (theme, diagnostics) = UiPreferences::cli_theme(Some(&path), Some(r#"{"theme":{"name":"inline","mode":"dark"}}"#));
        assert_eq!(theme, Some(("inline".into(), "dark".into())));
        assert_eq!(diagnostics.len(), 1);
        assert!(!diagnostics[0].contains("7"));
    }

    #[test]
    fn absent_name_or_mode_uses_v2_defaults() {
        assert_eq!(UiPreferences::cli_theme(None, Some(r#"{"theme":{}}"#)).0,
            Some(("opencode".into(), "system".into())));
    }


    #[test]
    fn inline_theme_at_limit_is_accepted_without_oversize_diagnostic() {
        let padding = " ".repeat(MAX_CONFIG_BYTES - r#"{"theme":{"mode":"light"}}"#.len());
        let inline = format!(r#"{{"theme":{{"mode":"light"}}}}{}"#, padding);
        assert_eq!(inline.len(), MAX_CONFIG_BYTES);
        let (theme, diagnostics) = UiPreferences::cli_theme(None, Some(&inline));
        assert_eq!(theme, Some(("opencode".into(), "light".into())));
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn oversized_inline_theme_preserves_valid_global_and_does_not_echo_value() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        fs::write(&path, r#"{"theme":{"name":"global","mode":"dark"}}"#).unwrap();
        let secret = "DO_NOT_ECHO_THEME_VALUE";
        let inline = format!(r#"{{"theme":{{"name":"{}"}}}}{}"#, secret, " ".repeat(MAX_CONFIG_BYTES));
        let (theme, diagnostics) = UiPreferences::cli_theme(Some(&path), Some(&inline));
        assert_eq!(theme, Some(("global".into(), "dark".into())));
        assert_eq!(diagnostics, vec!["inline CLI configuration exceeds size limit"]);
        assert!(diagnostics.iter().all(|line| !line.contains(secret)));
    }

    #[test]
    fn inline_theme_below_limit_overlays_global_recursively() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        fs::write(&path, r#"{"theme":{"name":"global","mode":"dark"}}"#).unwrap();
        let (theme, diagnostics) = UiPreferences::cli_theme(Some(&path), Some(r#"{"theme":{"mode":"light"}}"#));
        assert_eq!(theme, Some(("global".into(), "light".into())));
        assert!(diagnostics.is_empty());
    }
}

#[cfg(test)]
mod tab_config_tests {
    use super::*;
    use crate::native_navigation::TabOrientation;

    #[test]
    fn global_cli_layout_drives_navigation_orientation_without_rewriting_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        let original = "{ // keep this comment\n\"tabs\": {\"layout\": \"vertical\",}, \"unrelated\": 7 }\n";
        fs::write(&path, original).unwrap();
        let (orientation, diagnostics) = UiPreferences::tab_orientation(Some(&path), None);
        let mut navigation = crate::native_navigation::NavigationState::new();
        navigation.set_orientation(orientation);
        assert_eq!(navigation.orientation(), TabOrientation::Vertical);
        assert!(diagnostics.is_empty());
        assert_eq!(fs::read_to_string(path).unwrap(), original);
    }

    #[test]
    fn inline_layout_overrides_global_but_absent_inline_layout_preserves_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        fs::write(&path, r#"{"tabs":{"layout":"vertical"}}"#).unwrap();
        assert_eq!(UiPreferences::tab_orientation(Some(&path), Some(r#"{"tabs":{"layout":"horizontal"}}"#)).0, TabOrientation::Horizontal);
        assert_eq!(UiPreferences::tab_orientation(Some(&path), Some(r#"{"tabs":{"mode":"on"},"mouse":true}"#)).0, TabOrientation::Vertical);
    }

    #[test]
    fn invalid_inline_keeps_valid_global_and_does_not_echo_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        fs::write(&path, r#"{"tabs":{"layout":"vertical"}}"#).unwrap();
        for inline in [r#"{"tabs":{"layout":"FAKE_SECRET_LAYOUT"}}"#, r#"{"tabs":{"layout":42}}"#, r#"{"tabs":false}"#, "[1,2]", "not-json"] {
            let (orientation, diagnostics) = UiPreferences::tab_orientation(Some(&path), Some(inline));
            assert_eq!(orientation, TabOrientation::Vertical);
            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics.iter().all(|line| !line.contains("FAKE_SECRET_LAYOUT") && !line.contains("not-json")));
        }
    }

    #[test]
    fn absent_global_uses_horizontal_default_or_explicit_inline_without_home() {
        assert_eq!(UiPreferences::tab_orientation(None, None), (TabOrientation::Horizontal, Vec::new()));
        assert_eq!(UiPreferences::tab_orientation(None, Some("")), (TabOrientation::Horizontal, Vec::new()));
        assert_eq!(UiPreferences::tab_orientation(None, Some(r#"{"tabs":{"layout":"vertical"}}"#)), (TabOrientation::Vertical, Vec::new()));
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(UiPreferences::tab_orientation(Some(&dir.path().join("missing-cli.json")), None), (TabOrientation::Horizontal, Vec::new()));
    }

    #[test]
    fn invalid_global_does_not_prevent_valid_inline_or_leak_file_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        fs::write(&path, "FAKE_SECRET_INVALID_JSON").unwrap();
        let (orientation, diagnostics) = UiPreferences::tab_orientation(Some(&path), Some(r#"{"tabs":{"layout":"vertical"}}"#));
        assert_eq!(orientation, TabOrientation::Vertical);
        assert_eq!(diagnostics.len(), 1);
        assert!(!diagnostics[0].contains("FAKE_SECRET_INVALID_JSON"));
    }

    #[test]
    fn file_and_inline_config_are_bounded_independently() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        let oversized = " ".repeat(MAX_CONFIG_BYTES + 1);
        fs::write(&path, &oversized).unwrap();
        let (orientation, diagnostics) = UiPreferences::tab_orientation(Some(&path), Some(r#"{"tabs":{"layout":"vertical"}}"#));
        assert_eq!(orientation, TabOrientation::Vertical);
        assert_eq!(diagnostics.len(), 1);
        let (orientation, diagnostics) = UiPreferences::tab_orientation(None, Some(&oversized));
        assert_eq!(orientation, TabOrientation::Horizontal);
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn tab_indicators_defaults_to_status_and_inline_overrides_global() {
        use crate::native_tabs::TabIndicators;
        assert_eq!(UiPreferences::tab_indicators(None, None), TabIndicators::Status);
        assert_eq!(UiPreferences::tab_indicators(None, Some("")), TabIndicators::Status);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        fs::write(&path, r#"{"tabs":{"indicators":"numbers"}}"#).unwrap();
        assert_eq!(UiPreferences::tab_indicators(Some(&path), None), TabIndicators::Numbers);
        // An inline document that omits the key must not clobber the disk value.
        assert_eq!(
            UiPreferences::tab_indicators(Some(&path), Some(r#"{"tabs":{"mode":"on"}}"#)),
            TabIndicators::Numbers
        );
        assert_eq!(
            UiPreferences::tab_indicators(Some(&path), Some(r#"{"tabs":{"indicators":"status"}}"#)),
            TabIndicators::Status
        );
        // Invalid/oversized inline falls back to the disk value without leaking.
        assert_eq!(
            UiPreferences::tab_indicators(Some(&path), Some("not-json")),
            TabIndicators::Numbers
        );
    }
}

#[cfg(test)]
mod tab_persistence_tests {
    use super::*;
    use crate::native_navigation::{PersistedTabs, SessionRef};

    fn session(id: &str) -> SessionRef {
        SessionRef(id.to_owned())
    }

    #[test]
    fn tabs_roundtrip_per_scope_and_preserve_other_scopes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tabs.json");
        let first = PersistedTabs {
            order: vec![session("one"), session("two")],
            titles: vec![(session("two"), "Second".to_owned())],
        };
        UiPreferences::persist_session_tabs(&path, "/work/a", &first).unwrap();
        let second = PersistedTabs {
            order: vec![session("three")],
            titles: Vec::new(),
        };
        UiPreferences::persist_session_tabs(&path, "/work/b", &second).unwrap();
        assert_eq!(UiPreferences::load_session_tabs(&path, "/work/a").unwrap(), first);
        assert_eq!(UiPreferences::load_session_tabs(&path, "/work/b").unwrap(), second);
        assert_eq!(
            UiPreferences::load_session_tabs(&path, "/work/missing").unwrap(),
            PersistedTabs::default()
        );
        // On-disk shape follows upstream `{"cwd": {"<cwd>": {"tabs": [...]}}}`.
        let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["cwd"]["/work/a"]["tabs"][1]["sessionID"], "two");
        assert_eq!(saved["cwd"]["/work/a"]["tabs"][1]["title"], "Second");
        assert_eq!(saved["cwd"]["/work/b"]["tabs"][0]["sessionID"], "three");
    }

    #[test]
    fn global_scope_is_parsed_and_roundtrips_the_global_store() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tabs.json");
        let (scope, diagnostics) = UiPreferences::tab_scope(
            None,
            Some(r#"{"tabs":{"scope":"global"}}"#),
            "/work/a",
        );
        assert_eq!(scope, TabPersistenceScope::Global);
        assert!(diagnostics.is_empty());
        let tabs = PersistedTabs {
            order: vec![session("one")],
            titles: Vec::new(),
        };
        UiPreferences::persist_session_tabs(&path, scope, &tabs).unwrap();
        assert_eq!(UiPreferences::load_session_tabs(&path, scope).unwrap(), tabs);
        // `context/session-tabs.tsx:105-113`: global writes the `global` store,
        // not a `cwd` entry.
        let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["global"]["tabs"][0]["sessionID"], "one");
        assert!(saved.get("cwd").is_none());
        // Default is `cwd` (`config/index.tsx:334`).
        assert_eq!(
            UiPreferences::tab_scope(None, None, "/work/a").0,
            TabPersistenceScope::Cwd("/work/a")
        );
        assert_eq!(
            UiPreferences::tab_scope(None, Some(r#"{"tabs":{"scope":"cwd"}}"#), "/work/a").0,
            TabPersistenceScope::Cwd("/work/a")
        );
    }

    #[test]
    fn malformed_rows_are_skipped_without_failing_restore() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tabs.json");
        fs::write(
            &path,
            r#"{"cwd":{"/work/a":{"tabs":[
                {"sessionID":"keep","title":"ok"},
                {"sessionID":"keep","title":"duplicate"},
                {"title":"no id"},
                {"sessionID":""},
                {"sessionID":42},
                {"sessionID":"ctl\u0001"},
                {"sessionID":"long-title","title":""}
            ]}}}"#,
        )
        .unwrap();
        let loaded = UiPreferences::load_session_tabs(&path, "/work/a").unwrap();
        assert_eq!(loaded.order, vec![session("keep"), session("long-title")]);
        assert_eq!(loaded.titles, vec![(session("keep"), "ok".to_owned())]);
        // An invalid scope is refused before touching the file.
        assert!(UiPreferences::persist_session_tabs(&path, "", &loaded).is_err());
        assert!(UiPreferences::persist_session_tabs(&path, "bad\u{0}", &loaded).is_err());
    }

    #[test]
    fn missing_or_invalid_file_loads_default_layout() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            UiPreferences::load_session_tabs(&dir.path().join("missing.json"), "/work").unwrap(),
            PersistedTabs::default()
        );
        let path = dir.path().join("tabs.json");
        fs::write(&path, "not-json").unwrap();
        assert!(UiPreferences::load_session_tabs(&path, "/work").is_err());
        fs::write(&path, r#"{"cwd":{"/work":{"tabs":"nope"}}}"#).unwrap();
        assert_eq!(
            UiPreferences::load_session_tabs(&path, "/work").unwrap(),
            PersistedTabs::default()
        );
    }

    #[test]
    fn scope_count_is_bounded_and_new_scope_wins() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tabs.json");
        for index in 0..=MAX_TAB_SCOPES {
            let scope = format!("/work/{index}");
            UiPreferences::persist_session_tabs(
                &path,
                scope.as_str(),
                &PersistedTabs { order: vec![session("s")], titles: Vec::new() },
            )
            .unwrap();
        }
        let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["cwd"].as_object().unwrap().len(), MAX_TAB_SCOPES);
        assert!(saved["cwd"][format!("/work/{MAX_TAB_SCOPES}")].is_object());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_jsonc_values_and_nested_tui_shape() {
        let text = strip_jsonc(
            r#"{ // comment
          "tui": { "prompt": { "max_width": "auto", }, "theme": "dark" },
          "url": "https://example.test/*",
        }"#,
        )
        .unwrap();
        let parsed: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["tui"]["prompt"]["max_width"], "auto");
        assert_eq!(parsed["url"], "https://example.test/*");
        let joined = strip_jsonc("1/**/2").unwrap();
        assert!(serde_json::from_str::<Value>(&joined).is_err());
    }

    #[test]
    fn invalid_later_config_keeps_last_good_preferences() {
        let dir = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "opencode-ui-config-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        let global = dir.join("global.json");
        let project = dir.join("project.json");
        fs::write(&global, r#"{"prompt":{"max_width":88},"theme":"dark"}"#).unwrap();
        fs::write(&project, r#"{"prompt":{"max_width":0},"theme":7}"#).unwrap();

        let prefs = UiPreferences::load(&[global, project], &dir.join("missing-kv.json")).unwrap();
        assert_eq!(prefs.prompt_width, Some(PromptWidth::Fixed(88)));
        assert_eq!(prefs.theme.as_deref(), Some("dark"));
        assert_eq!(prefs.diagnostics.len(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    /// TAB-24: `tui.animations` is surfaced as the resolved reduced-motion
    /// flag source; the later config file wins and absent stays `None`
    /// (consumed as `?? true`).
    #[test]
    fn animations_flag_is_surfaced_from_the_last_config_file() {
        let dir = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "opencode-ui-anim-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        let global = dir.join("global.json");
        let project = dir.join("project.json");
        fs::write(&global, r#"{"animations":true}"#).unwrap();
        fs::write(&project, r#"{"animations":false}"#).unwrap();
        let prefs = UiPreferences::load(&[global, project], &dir.join("missing-kv.json")).unwrap();
        assert_eq!(prefs.animations, Some(false), "later config wins");
        let absent = UiPreferences::load(&[], &dir.join("missing-kv.json")).unwrap();
        assert_eq!(absent.animations, None, "absent => caller resolves `?? true`");
        fs::remove_dir_all(dir).unwrap();
    }

    /// THEME-14: `tui.cursor` is surfaced raw for the theme runtime to parse;
    /// the later config file wins, and an absent value stays `None`.
    #[test]
    fn cursor_object_is_surfaced_from_the_last_config_file() {
        let dir = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "opencode-ui-cursor-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        let global = dir.join("global.json");
        let project = dir.join("project.json");
        fs::write(&global, r#"{"cursor":{"style":"block","blinking":true}}"#).unwrap();
        fs::write(&project, r#"{"cursor":{"style":"underline","blinking":false}}"#).unwrap();
        let prefs = UiPreferences::load(&[global, project], &dir.join("missing-kv.json")).unwrap();
        assert_eq!(
            prefs.cursor,
            Some(serde_json::json!({"style":"underline","blinking":false}))
        );
        let empty = UiPreferences::load(&[], &dir.join("missing-kv.json")).unwrap();
        assert_eq!(empty.cursor, None);
        fs::remove_dir_all(dir).unwrap();
    }

    /// TAB-23: the raw `keybinds` object is surfaced from the last config file
    /// (`config/index.tsx:78` `Schema.optional(TuiKeybind.KeybindOverrides)`),
    /// kept as raw JSON like `cursor`; an absent value stays `None` so the live
    /// resolver falls back to the frozen default keymap.
    #[test]
    fn keybinds_object_is_surfaced_from_the_last_config_file() {
        let dir = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "opencode-ui-keybinds-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        let global = dir.join("global.json");
        let project = dir.join("project.json");
        fs::write(&global, r#"{"keybinds":{"session_tab_next":"ctrl+n"}}"#).unwrap();
        fs::write(&project, r#"{"keybinds":{"session_tab_close":false}}"#).unwrap();
        let prefs = UiPreferences::load(&[global, project], &dir.join("missing-kv.json")).unwrap();
        assert_eq!(
            prefs.keybinds,
            Some(serde_json::json!({"session_tab_close": false})),
            "later config wins"
        );
        let empty = UiPreferences::load(&[], &dir.join("missing-kv.json")).unwrap();
        assert_eq!(empty.keybinds, None);
        fs::remove_dir_all(dir).unwrap();
    }

    /// CP-36: `prompt.paste` is surfaced for the live fold decision
    /// (`config/index.tsx:144`, `prompt/index.tsx:1463`). Only the two literals
    /// are accepted; anything else is a bounded diagnostic exactly like
    /// `prompt.max_width`, and an absent value stays `None` (treated as fold).
    #[test]
    fn prompt_paste_is_surfaced_and_rejects_non_literal_values() {
        let dir = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "opencode-ui-paste-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        let global = dir.join("global.json");
        let project = dir.join("project.json");
        fs::write(&global, r#"{"prompt":{"paste":"compact"}}"#).unwrap();
        fs::write(&project, r#"{"prompt":{"paste":"full"}}"#).unwrap();
        let prefs = UiPreferences::load(&[global, project], &dir.join("missing-kv.json")).unwrap();
        assert_eq!(prefs.prompt_paste.as_deref(), Some("full"), "later config wins");
        assert!(prefs.diagnostics.is_empty());

        let invalid = dir.join("invalid.json");
        fs::write(&invalid, r#"{"prompt":{"paste":"yes"}}"#).unwrap();
        let bad = UiPreferences::load(std::slice::from_ref(&invalid), &dir.join("missing-kv.json")).unwrap();
        assert_eq!(bad.prompt_paste, None, "non-literal value is not surfaced");
        assert_eq!(bad.diagnostics.len(), 1, "malformed value is a diagnostic");

        let absent = UiPreferences::load(&[], &dir.join("missing-kv.json")).unwrap();
        assert_eq!(absent.prompt_paste, None, "absent => caller folds");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn preserves_other_kv_and_uses_actual_sidebar_values() {
        let dir = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "opencode-ui-prefs-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let path = dir.join("opencode/kv.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, br#"{"sidebar":"auto","theme":"opencode","keep":7}"#).unwrap();
        UiPreferences::persist_sidebar(&path, true).unwrap();
        let prefs = UiPreferences::load(&[], &path).unwrap();
        assert!(prefs.sidebar_hidden);
        assert_eq!(prefs.theme.as_deref(), Some("opencode"));
        assert_eq!(read_state(&path).unwrap()["keep"], 7);
        fs::remove_dir_all(dir).unwrap();
    }

    /// SIDE-12: the getting-started dismissal round-trips under the exact
    /// upstream key/value shape (`footer.tsx:11-16`) and defaults to `false`
    /// when absent, preserving unrelated KV data.
    #[test]
    fn onboarding_dismissal_round_trips_under_the_getting_started_key() {
        let dir = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "opencode-ui-onboarding-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let path = dir.join("opencode/kv.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, br#"{"sidebar":"auto","keep":7}"#).unwrap();
        // Absent key: upstream `initial: { dismissed: false }`.
        assert!(!UiPreferences::load(&[], &path).unwrap().onboarding_dismissed);
        UiPreferences::dismiss_onboarding(&path, true).unwrap();
        assert!(UiPreferences::load(&[], &path).unwrap().onboarding_dismissed);
        let saved = read_state(&path).unwrap();
        assert_eq!(saved["getting-started"]["dismissed"], true);
        assert_eq!(saved["keep"], 7);
        assert_eq!(saved["sidebar"], "auto");
        fs::remove_dir_all(dir).unwrap();
    }

    /// SIDE-09: the MCP disclosure round-trips under the exact upstream
    /// plugin-namespaced key (`plugin/api.tsx:201` + `sidebar/mcp.tsx:6,96`) and
    /// `{open: bool}` shape, defaults to `None` when absent, and never clobbers
    /// the unrelated bare `"view"` key that `todo.tsx:7` stores on its own
    /// namespace.
    #[test]
    fn mcp_sidebar_open_round_trips_under_the_plugin_namespaced_key() {
        let dir = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "opencode-ui-mcp-open-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let path = dir.join("opencode/kv.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        // A bare `view` key from todo.tsx's namespace must survive untouched.
        fs::write(&path, br#"{"sidebar":"auto","keep":7,"view":{"open":false}}"#).unwrap();
        // Absent key: `None`, so the view keeps the upstream `open: true` default.
        assert_eq!(UiPreferences::load(&[], &path).unwrap().mcp_sidebar_open, None);
        UiPreferences::persist_mcp_sidebar_open(&path, false).unwrap();
        assert_eq!(
            UiPreferences::load(&[], &path).unwrap().mcp_sidebar_open,
            Some(false)
        );
        let saved = read_state(&path).unwrap();
        assert_eq!(saved["plugin.opencode.sidebar.mcp.view"]["open"], false);
        // The unrelated bare `view` key and other entries are preserved.
        assert_eq!(saved["view"]["open"], false);
        assert_eq!(saved["keep"], 7);
        assert_eq!(saved["sidebar"], "auto");
        // Round-trip back to expanded.
        UiPreferences::persist_mcp_sidebar_open(&path, true).unwrap();
        assert_eq!(
            UiPreferences::load(&[], &path).unwrap().mcp_sidebar_open,
            Some(true)
        );
        fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod settings_persistence_tests {
    use super::*;
    use crate::native_navigation::TabOrientation;
    use std::fs;

    #[test]
    fn cli_jsonc_layout_write_preserves_unrelated_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        fs::write(&path, "{ // keep comment\n \"keep\":7, \"tabs\": { \"mode\": \"auto\", },\n}").unwrap();
        UiPreferences::persist_tab_orientation(&path, TabOrientation::Vertical).unwrap();
        let value = parse_jsonc_value(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["keep"], 7);
        assert_eq!(value["tabs"]["mode"], "auto");
        assert_eq!(value["tabs"]["layout"], "vertical");
    }

    #[test]
    fn invalid_tabs_refuses_write_and_keeps_original_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        let original = r#"{"keep":7,"tabs":"invalid"}"#;
        fs::write(&path, original).unwrap();
        assert!(UiPreferences::persist_tab_orientation(&path, TabOrientation::Vertical).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), original);
    }
}

#[cfg(test)]
mod markdown_mode_tests {
    use super::*;

    #[test]
    fn disk_source_is_selected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        fs::write(&path, r#"{"session":{"markdown":"source"}}"#).unwrap();
        assert_eq!(UiPreferences::markdown_mode(Some(&path), None).0.as_deref(), Some("source"));
    }

    #[test]
    fn inline_rendered_overrides_disk_source() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        fs::write(&path, r#"{"session":{"markdown":"source"}}"#).unwrap();
        assert_eq!(UiPreferences::markdown_mode(Some(&path), Some(r#"{"session":{"markdown":"rendered"}}"#)).0.as_deref(), Some("rendered"));
    }

    #[test]
    fn absent_markdown_returns_none_for_rendered_upstream_default() {
        assert_eq!(UiPreferences::markdown_mode(None, None), (None, Vec::<String>::new()));
        assert_eq!(UiPreferences::markdown_mode(None, Some(r#"{"session":{}}"#)), (None, Vec::<String>::new()));
    }

    #[test]
    fn invalid_value_is_generic_diagnostic_without_content() {
        let secret = "SECRET_MARKDOWN_VALUE";
        let (_, diagnostics) = UiPreferences::markdown_mode(None, Some(&format!(r#"{{"session":{{"markdown":"{secret}"}}}}"#)));
        assert_eq!(diagnostics, vec!["invalid session.markdown value"]);
        assert!(diagnostics.iter().all(|line| !line.contains(secret)));
    }

    #[test]
    fn inline_exact_max_bytes_valid_json_is_accepted() {
        let base = r#"{"session":{"markdown":"source"}}"#;
        let inline = format!("{}{}", base, " ".repeat(MAX_CONFIG_BYTES - base.len()));
        assert_eq!(inline.len(), MAX_CONFIG_BYTES);
        assert_eq!(UiPreferences::markdown_mode(None, Some(&inline)).0.as_deref(), Some("source"));
        assert!(UiPreferences::markdown_mode(None, Some(&inline)).1.is_empty());
    }

    #[test]
    fn oversized_inline_and_disk_reject_without_overriding_valid_prior() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        fs::write(&path, "x".repeat(MAX_CONFIG_BYTES + 1)).unwrap();
        let oversized_inline = "y".repeat(MAX_CONFIG_BYTES + 1);
        let (selected, diagnostics) = UiPreferences::markdown_mode(Some(&path), Some(&oversized_inline));
        assert_eq!(selected, None);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics, vec!["unable to read bounded global CLI configuration", "inline CLI configuration exceeds size limit"]);

        fs::write(&path, r#"{"session":{"markdown":"source"}}"#).unwrap();
        let (selected, diagnostics) = UiPreferences::markdown_mode(Some(&path), Some(&oversized_inline));
        assert_eq!(selected.as_deref(), Some("source"));
        assert_eq!(diagnostics, vec!["inline CLI configuration exceeds size limit"]);
    }

    #[test]
    fn malformed_jsonc_reports_generic_diagnostic_without_secret() {
        let secret = "SECRET_MALFORMED_MARKDOWN";
        let malformed = format!(r#"{{"session":{{"markdown":"source",}},"secret":"{secret}""#);
        let (selected, diagnostics) = UiPreferences::markdown_mode(None, Some(&malformed));
        assert_eq!(selected, None);
        assert_eq!(diagnostics, vec!["invalid inline CLI configuration"]);
        assert!(diagnostics.iter().all(|line| !line.contains(secret)));
    }
}

#[cfg(test)]
mod session_scrollbar_tests {
    use super::*;

    #[test]
    fn absent_scrollbar_keeps_the_upstream_off_default() {
        assert_eq!(UiPreferences::session_scrollbar(None, None), (false, Vec::<String>::new()));
        assert_eq!(UiPreferences::session_scrollbar(None, Some(r#"{"session":{}}"#)), (false, Vec::<String>::new()));
    }

    #[test]
    fn inline_true_overrides_disk_false() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        fs::write(&path, r#"{"session":{"scrollbar":false}}"#).unwrap();
        assert_eq!(UiPreferences::session_scrollbar(Some(&path), None), (false, Vec::<String>::new()));
        assert_eq!(
            UiPreferences::session_scrollbar(Some(&path), Some(r#"{"session":{"scrollbar":true}}"#)),
            (true, Vec::<String>::new())
        );
    }

    #[test]
    fn non_boolean_is_a_generic_diagnostic_without_content() {
        let secret = "SECRET_SCROLLBAR_VALUE";
        let (selected, diagnostics) =
            UiPreferences::session_scrollbar(None, Some(&format!(r#"{{"session":{{"scrollbar":"{secret}"}}}}"#)));
        assert!(!selected);
        assert_eq!(diagnostics, vec!["invalid session.scrollbar value"]);
        assert!(diagnostics.iter().all(|line| !line.contains(secret)));
    }
}

mod session_tps_tests {
    use super::*;

    #[test]
    fn absent_tps_keeps_the_upstream_on_default() {
        // `config/index.tsx:328`: `input.session?.tps ?? true`.
        assert_eq!(UiPreferences::session_tps(None, None), (true, Vec::<String>::new()));
        assert_eq!(UiPreferences::session_tps(None, Some(r#"{"session":{}}"#)), (true, Vec::<String>::new()));
    }

    #[test]
    fn inline_false_overrides_disk_true() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        fs::write(&path, r#"{"session":{"tps":true}}"#).unwrap();
        assert_eq!(UiPreferences::session_tps(Some(&path), None), (true, Vec::<String>::new()));
        assert_eq!(
            UiPreferences::session_tps(Some(&path), Some(r#"{"session":{"tps":false}}"#)),
            (false, Vec::<String>::new())
        );
    }

    #[test]
    fn non_boolean_is_a_generic_diagnostic_without_content() {
        let secret = "SECRET_TPS_VALUE";
        let (selected, diagnostics) =
            UiPreferences::session_tps(None, Some(&format!(r#"{{"session":{{"tps":"{secret}"}}}}"#)));
        assert!(selected, "invalid value keeps the upstream default");
        assert_eq!(diagnostics, vec!["invalid session.tps value"]);
        assert!(diagnostics.iter().all(|line| !line.contains(secret)));
    }
}

#[cfg(test)]
mod prompt_image_preview_tests {
    use super::*;

    #[test]
    fn cp38_absent_image_preview_keeps_the_upstream_on_default() {
        // `config/index.tsx:147` + `component/prompt/index.tsx:1699`.
        assert_eq!(
            UiPreferences::prompt_image_preview(None, None),
            (true, Vec::<String>::new())
        );
        assert_eq!(
            UiPreferences::prompt_image_preview(None, Some(r#"{"prompt":{}}"#)),
            (true, Vec::<String>::new())
        );
    }

    #[test]
    fn cp38_inline_false_overrides_disk_true() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        fs::write(&path, r#"{"prompt":{"image_preview":true}}"#).unwrap();
        assert_eq!(
            UiPreferences::prompt_image_preview(Some(&path), None),
            (true, Vec::<String>::new())
        );
        assert_eq!(
            UiPreferences::prompt_image_preview(
                Some(&path),
                Some(r#"{"prompt":{"image_preview":false}}"#)
            ),
            (false, Vec::<String>::new())
        );
    }

    #[test]
    fn cp38_non_boolean_is_a_generic_diagnostic_without_content() {
        let secret = "SECRET_PREVIEW_VALUE";
        let (selected, diagnostics) = UiPreferences::prompt_image_preview(
            None,
            Some(&format!(r#"{{"prompt":{{"image_preview":"{secret}"}}}}"#)),
        );
        assert!(selected, "invalid value keeps the upstream default");
        assert_eq!(diagnostics, vec!["invalid prompt.image_preview value"]);
        assert!(diagnostics.iter().all(|line| !line.contains(secret)));
    }
}

#[cfg(test)]
mod diff001_preferences_tests {
    use super::*;

    #[test]
    fn diff001_config_parse_absent_auto_split_unified() {
        assert_eq!(UiPreferences::diffs_view(None, None).0, crate::native_diff_view::DiffViewPreference::Auto);
        for (value, expected) in [
            ("auto", crate::native_diff_view::DiffViewPreference::Auto),
            ("split", crate::native_diff_view::DiffViewPreference::Split),
            ("unified", crate::native_diff_view::DiffViewPreference::Unified),
        ] {
            assert_eq!(UiPreferences::diffs_view(None, Some(&format!(r#"{{"diffs":{{"view":"{value}"}}}}"#))).0, expected);
        }
    }

    #[test]
    fn diff001_unknown_and_case_mismatch_are_invalid() {
        for value in ["unknown", "AUTO", "Split", "UNIFIED"] {
            let (selected, diagnostics) = UiPreferences::diffs_view(
                None,
                Some(&format!(r#"{{"diffs":{{"view":"{value}"}}}}"#)),
            );
            assert_eq!(selected, crate::native_diff_view::DiffViewPreference::Auto);
            assert_eq!(diagnostics, vec!["invalid diffs.view value"]);
        }
    }

    #[test]
    fn diff001_inline_overrides_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli.json");
        fs::write(&path, r#"{"diffs":{"view":"split"}}"#).unwrap();
        assert_eq!(UiPreferences::diffs_view(Some(&path), None).0, crate::native_diff_view::DiffViewPreference::Split);
        assert_eq!(UiPreferences::diffs_view(Some(&path), Some(r#"{"diffs":{"view":"unified"}}"#)).0, crate::native_diff_view::DiffViewPreference::Unified);
    }
}
