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
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

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
    /// `tui.json` theme takes precedence over the persisted `kv.json` choice.
    pub theme: Option<String>,
    pub sidebar_hidden: bool,
    pub theme_mode: Option<String>,
    pub theme_mode_lock: Option<String>,
    /// Bounded diagnostics for config/state files skipped during startup.
    pub diagnostics: Vec<String>,
}

impl UiPreferences {
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
        let mut configured_theme = None;
        let mut diagnostics = Vec::new();
        for path in config_paths {
            let loaded = (|| -> Result<Option<(Option<PromptWidth>, Option<String>)>, String> {
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
                Ok(Some((width, theme)))
            })();

            match loaded {
                Ok(Some((width, theme))) => {
                    if let Some(width) = width {
                        prompt_width = Some(width);
                    }
                    if let Some(theme) = theme {
                        configured_theme = Some(theme);
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
        Ok(Self {
            prompt_width,
            theme: configured_theme.or(stored_theme),
            sidebar_hidden,
            theme_mode: state
                .get("theme_mode")
                .and_then(Value::as_str)
                .map(str::to_owned),
            theme_mode_lock: state
                .get("theme_mode_lock")
                .and_then(Value::as_str)
                .map(str::to_owned),
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
        if self.original.is_none() {
            return Ok(Map::new());
        }
        let file = File::from(
            rfs::openat(
                &self.directory,
                &self.name,
                rfs::OFlags::RDONLY
                    | rfs::OFlags::NOFOLLOW
                    | rfs::OFlags::NONBLOCK
                    | rfs::OFlags::CLOEXEC,
                rfs::Mode::empty(),
            )
            .map_err(|_| "unable to open anchored TUI state")?,
        );
        let metadata = file
            .metadata()
            .map_err(|_| "unable to inspect opened TUI state")?;
        if !metadata.is_file()
            || metadata.nlink() != 1
            || metadata.uid() != getuid().as_raw()
            || Some((metadata.dev(), metadata.ino())) != self.original
            || metadata.len() > MAX_STATE_BYTES as u64
        {
            return Err("TUI state changed or exceeds bounds".into());
        }
        let mut bytes = Vec::new();
        file.take(MAX_STATE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "unable to read anchored TUI state")?;
        if bytes.len() > MAX_STATE_BYTES {
            return Err("TUI state exceeds bounds".into());
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
        let bytes = serde_json::to_vec(&state).map_err(|_| "unable to encode TUI state")?;
        if bytes.len() > MAX_STATE_BYTES {
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
}
