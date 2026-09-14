//! Typed layered configuration for the native harness (BASE-006, REQ-019, REQ-032).
//!
//! Layer precedence, lowest to highest: built-in defaults -> user config file
//! (TOML or JSON) -> environment variables -> CLI key/value overrides. Layers
//! merge as objects; unknown keys are ignored so older clients can read newer
//! files. Only the final resolved value is validated semantically.
//!
//! REQ-032 contract: every optional feature is a flag that defaults to off
//! (`Features::default()` has all flags false). Disabled features must not
//! start their runtime subsystem (see PLAN.md ADR-005: native mode starts
//! neither plugin host); this module stores flags only and offers
//! `Features::require` so call sites gate construction explicitly.
//!
//! ADR-006 note: `permission` settings here configure ordinary operation
//! prompts only. Project config is never a source of human authority and can
//! never widen a human-only grant or bypass system protection.
//!
//! Environment variables use `OPENCODE_RK_` plus a double-underscore path
//! separator to keep underscore-bearing keys unambiguous, for example
//! `OPENCODE_RK_THEME__MODE=light` sets `theme.mode` and
//! `OPENCODE_RK_FEATURES__JS_PLUGIN_HOST=true` sets `features.js_plugin_host`.
//! CLI overrides use dotted paths, for example `theme.mode=dark`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Prefix for environment variables that feed the configuration layer.
pub const ENV_PREFIX: &str = "OPENCODE_RK_";

/// Separator inside an environment variable name between path segments.
pub const ENV_SEPARATOR: &str = "__";

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ConfigError {
    #[error("invalid TOML layer: {0}")]
    InvalidToml(String),
    #[error("invalid JSON layer: {0}")]
    InvalidJson(String),
    #[error("config layer must be an object, not a scalar or array")]
    LayerNotObject,
    #[error("invalid config value: {0}")]
    InvalidValue(String),
    #[error("feature {0} is disabled")]
    FeatureDisabled(&'static str),
}

// ---------------------------------------------------------------------------
// Theme (REQ-019)
// ---------------------------------------------------------------------------

/// Terminal theme mode. `System` follows terminal dark/light detection and
/// falls back to `Dark` when detection is unavailable, matching the pinned
/// upstream run-theme fallback (.upstream/opencode
/// packages/opencode/src/cli/cmd/run/theme.ts:1-7).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    System,
}

/// Theme selection plus optional named-theme install surface and custom color
/// overrides. Custom entries are hex colors keyed by theme slot name
/// (upstream surface: packages/plugin/src/tui.ts:359-368 `TuiTheme` install
/// and dark/light mode).
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub mode: ThemeMode,
    pub name: Option<String>,
    pub custom: BTreeMap<String, String>,
}

// ---------------------------------------------------------------------------
// Models (REQ-019)
// ---------------------------------------------------------------------------

/// Model selection and sampling bounds. All fields are optional because the
/// catalog owns model identity; no model id is invented here.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelSettings {
    pub default_model: Option<String>,
    pub temperature: Option<f32>,
    pub max_output_tokens: Option<u32>,
}

// ---------------------------------------------------------------------------
// Permissions (REQ-019)
// ---------------------------------------------------------------------------

/// Per-domain permission mode for ordinary operation. `Allow` never bypasses
/// human-only grants or mandatory system protection (ADR-006).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionMode {
    #[default]
    Ask,
    Allow,
    Deny,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PermissionSettings {
    pub edit: PermissionMode,
    pub bash: PermissionMode,
    pub webfetch: PermissionMode,
}

// ---------------------------------------------------------------------------
// Shell (REQ-019)
// ---------------------------------------------------------------------------

/// Shell tool settings. `max_output_bytes` bounds retained output per
/// invocation; the default mirrors the resource model's
/// `maxInMemoryPreviewBytesPerTool` target (config/resource-targets.json).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellSettings {
    pub program: String,
    pub timeout_ms: u64,
    pub max_output_bytes: usize,
}

impl Default for ShellSettings {
    fn default() -> Self {
        Self {
            program: "bash".to_string(),
            timeout_ms: 30_000,
            max_output_bytes: 65_536,
        }
    }
}

// ---------------------------------------------------------------------------
// Compaction (REQ-019)
// ---------------------------------------------------------------------------

/// Context compaction policy. Enabled by default because unbounded history
/// growth is forbidden by the resource model.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompactionSettings {
    pub enabled: bool,
    pub threshold_tokens: u64,
    pub keep_recent_turns: usize,
}

impl Default for CompactionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_tokens: 100_000,
            keep_recent_turns: 20,
        }
    }
}

// ---------------------------------------------------------------------------
// Feature flags (REQ-032)
// ---------------------------------------------------------------------------

/// Optional feature identifiers. Every flag is off in `Features::default()`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Feature {
    Plugins,
    JsPluginHost,
    WebClient,
    Desktop,
    Sharing,
    Sounds,
}

impl Feature {
    /// Every known flag, used for exhaustive default-off assertions.
    pub const ALL: &'static [Feature] = &[
        Feature::Plugins,
        Feature::JsPluginHost,
        Feature::WebClient,
        Feature::Desktop,
        Feature::Sharing,
        Feature::Sounds,
    ];

    /// Stable flag name used in config files and environment variables.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Feature::Plugins => "plugins",
            Feature::JsPluginHost => "js_plugin_host",
            Feature::WebClient => "web_client",
            Feature::Desktop => "desktop",
            Feature::Sharing => "sharing",
            Feature::Sounds => "sounds",
        }
    }

    /// Parse a flag name. Unknown names return `None` instead of panicking.
    #[must_use]
    pub fn parse(name: &str) -> Option<Feature> {
        Feature::ALL.iter().copied().find(|f| f.name() == name)
    }
}

/// Optional feature flags. Default has every flag off; disabled features
/// have no hidden cost because subsystem construction sites must check
/// `enabled` or `require` before initializing anything (ADR-005).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Features {
    pub plugins: bool,
    pub js_plugin_host: bool,
    pub web_client: bool,
    pub desktop: bool,
    pub sharing: bool,
    pub sounds: bool,
}

impl Features {
    #[must_use]
    pub fn enabled(&self, feature: Feature) -> bool {
        match feature {
            Feature::Plugins => self.plugins,
            Feature::JsPluginHost => self.js_plugin_host,
            Feature::WebClient => self.web_client,
            Feature::Desktop => self.desktop,
            Feature::Sharing => self.sharing,
            Feature::Sounds => self.sounds,
        }
    }

    pub fn set(&mut self, feature: Feature, on: bool) {
        match feature {
            Feature::Plugins => self.plugins = on,
            Feature::JsPluginHost => self.js_plugin_host = on,
            Feature::WebClient => self.web_client = on,
            Feature::Desktop => self.desktop = on,
            Feature::Sharing => self.sharing = on,
            Feature::Sounds => self.sounds = on,
        }
    }

    /// Runtime gate: error unless the feature is enabled. Call sites use this
    /// before constructing or starting the feature's subsystem.
    pub fn require(&self, feature: Feature) -> Result<(), ConfigError> {
        if self.enabled(feature) {
            Ok(())
        } else {
            Err(ConfigError::FeatureDisabled(feature.name()))
        }
    }
}

// ---------------------------------------------------------------------------
// Root config
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: ThemeConfig,
    pub model: ModelSettings,
    pub permission: PermissionSettings,
    pub shell: ShellSettings,
    pub compaction: CompactionSettings,
    pub features: Features,
}

impl Config {
    /// Resolve the configuration by merging layers over the defaults in the
    /// given order (later layers win), then deserializing into typed config
    /// and validating. An empty layer list yields the validated defaults.
    pub fn resolve(layers: &[ConfigLayer]) -> Result<Self, ConfigError> {
        let mut merged = serde_json::to_value(Config::default())
            .map_err(|error| ConfigError::InvalidJson(error.to_string()))?;
        for layer in layers {
            merge_value(&mut merged, layer.value().clone());
        }
        let config: Config = serde_json::from_value(merged)
            .map_err(|error| ConfigError::InvalidValue(error.to_string()))?;
        config.validate()?;
        Ok(config)
    }

    /// Semantic validation. Parse-level errors are already rejected by serde;
    /// this catches values that parse but violate the resource or theme
    /// contract.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if let Some(name) = &self.theme.name {
            if name.trim().is_empty() {
                return Err(ConfigError::InvalidValue(
                    "theme.name must not be empty".to_string(),
                ));
            }
        }
        for (slot, color) in &self.theme.custom {
            if !is_hex_color(color) {
                return Err(ConfigError::InvalidValue(format!(
                    "theme.custom.{slot} must be a #RRGGBB or #RRGGBBAA color, got {color}"
                )));
            }
        }
        if let Some(temperature) = self.model.temperature {
            if !(0.0..=2.0).contains(&temperature) {
                return Err(ConfigError::InvalidValue(format!(
                    "model.temperature must be within 0.0..=2.0, got {temperature}"
                )));
            }
        }
        if let Some(tokens) = self.model.max_output_tokens {
            if tokens == 0 {
                return Err(ConfigError::InvalidValue(
                    "model.max_output_tokens must be non-zero".to_string(),
                ));
            }
        }
        if self.shell.program.trim().is_empty() {
            return Err(ConfigError::InvalidValue(
                "shell.program must not be empty".to_string(),
            ));
        }
        if self.shell.timeout_ms == 0 {
            return Err(ConfigError::InvalidValue(
                "shell.timeout_ms must be non-zero".to_string(),
            ));
        }
        if self.shell.max_output_bytes == 0 {
            return Err(ConfigError::InvalidValue(
                "shell.max_output_bytes must be non-zero".to_string(),
            ));
        }
        if self.compaction.threshold_tokens == 0 {
            return Err(ConfigError::InvalidValue(
                "compaction.threshold_tokens must be non-zero".to_string(),
            ));
        }
        if self.compaction.keep_recent_turns == 0 {
            return Err(ConfigError::InvalidValue(
                "compaction.keep_recent_turns must be non-zero".to_string(),
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Layers
// ---------------------------------------------------------------------------

/// One configuration layer: a JSON object produced from a TOML or JSON file,
/// environment variables, or CLI key/value pairs.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigLayer(Value);

impl ConfigLayer {
    /// Parse a user config file written in TOML.
    pub fn from_toml_str(text: &str) -> Result<Self, ConfigError> {
        let parsed: toml::Value = toml::from_str(text)
            .map_err(|error| ConfigError::InvalidToml(error.to_string()))?;
        Self::from_value(serde_json::to_value(parsed).map_err(|error| ConfigError::InvalidToml(error.to_string()))?)
    }

    /// Parse a user config file written in JSON.
    pub fn from_json_str(text: &str) -> Result<Self, ConfigError> {
        let parsed: Value =
            serde_json::from_str(text).map_err(|error| ConfigError::InvalidJson(error.to_string()))?;
        Self::from_value(parsed)
    }

    /// Build a layer from dotted key/value pairs, the CLI override mechanism.
    /// Values are stringly and coerced: "true"/"false" become booleans and
    /// numeric strings become numbers.
    pub fn from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Result<Self, ConfigError> {
        let mut root = serde_json::Map::new();
        for (key, raw) in pairs {
            let segments: Vec<&str> = key.split('.').collect();
            if segments.is_empty() || segments.iter().any(|s| s.is_empty()) {
                return Err(ConfigError::InvalidValue(format!("empty path segment in override key {key}")));
            }
            let (leaf, parents) = segments.split_last().expect("non-empty by check above");
            let mut cursor = &mut root;
            for segment in parents {
                cursor = match cursor
                    .entry(segment.to_string())
                    .or_insert_with(|| Value::Object(serde_json::Map::new()))
                {
                    Value::Object(map) => map,
                    other => {
                        *other = Value::Object(serde_json::Map::new());
                        match other {
                            Value::Object(map) => map,
                            _ => unreachable!("just replaced with an object"),
                        }
                    }
                };
            }
            if cursor.insert(leaf.to_string(), coerce(raw)).is_some() {
                return Err(ConfigError::InvalidValue(format!("duplicate override key {key}")));
            }
        }
        Self::from_value(Value::Object(root))
    }

    /// Build a layer from environment variables. Only variables starting
    /// with `OPENCODE_RK_` are considered; the rest is split on `__` into a
    /// dotted path, lowercased. Callers pass an iterator so tests stay pure.
    pub fn from_env_vars(
        vars: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, ConfigError> {
        let mut pairs: Vec<(String, String)> = Vec::new();
        for (name, value) in vars {
            let Some(rest) = name.strip_prefix(ENV_PREFIX) else {
                continue;
            };
            let path = rest.to_ascii_lowercase().replace(ENV_SEPARATOR, ".");
            pairs.push((path, value));
        }
        let borrowed: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        Self::from_pairs(borrowed)
    }

    fn from_value(value: Value) -> Result<Self, ConfigError> {
        if value.is_object() {
            Ok(ConfigLayer(value))
        } else {
            Err(ConfigError::LayerNotObject)
        }
    }

    #[must_use]
    pub fn value(&self) -> &Value {
        &self.0
    }
}

fn merge_value(base: &mut Value, over: Value) {
    match (base, over) {
        (Value::Object(base_map), Value::Object(over_map)) => {
            for (key, over_value) in over_map {
                merge_value(base_map.entry(key).or_insert(Value::Null), over_value);
            }
        }
        (base, over) => *base = over,
    }
}

fn coerce(raw: &str) -> Value {
    if raw == "true" {
        return Value::Bool(true);
    }
    if raw == "false" {
        return Value::Bool(false);
    }
    if let Ok(number) = raw.parse::<u64>() {
        return Value::Number(number.into());
    }
    if let Ok(float) = raw.parse::<f64>() {
        if let Some(number) = serde_json::Number::from_f64(float) {
            return Value::Number(number);
        }
    }
    Value::String(raw.to_string())
}

fn is_hex_color(value: &str) -> bool {
    let Some(digits) = value.strip_prefix('#') else {
        return false;
    };
    (digits.len() == 6 || digits.len() == 8) && digits.bytes().all(|b| b.is_ascii_hexdigit())
}

// ---------------------------------------------------------------------------
// Tests (BASE-006-T01..T05)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const USER_TOML: &str = r#"
theme.mode = "light"
theme.name = "ocean"
shell.timeout_ms = 60000
"#;

    // T01: defaults resolve, every optional feature is off by default.
    #[test]
    fn t01_defaults_and_zero_features() {
        let config = Config::resolve(&[]).unwrap();
        assert_eq!(config, Config::default());
        assert_eq!(config.theme.mode, ThemeMode::Dark);
        assert!(config.theme.name.is_none());
        assert!(config.theme.custom.is_empty());
        assert!(config.model.default_model.is_none());
        assert_eq!(config.permission.edit, PermissionMode::Ask);
        assert_eq!(config.permission.bash, PermissionMode::Ask);
        assert_eq!(config.permission.webfetch, PermissionMode::Ask);
        assert_eq!(config.shell.program, "bash");
        assert_eq!(config.shell.timeout_ms, 30_000);
        assert_eq!(config.shell.max_output_bytes, 65_536);
        assert!(config.compaction.enabled);
        assert_eq!(config.compaction.threshold_tokens, 100_000);
        assert_eq!(config.compaction.keep_recent_turns, 20);
        for feature in Feature::ALL {
            assert!(!config.features.enabled(*feature), "default must disable {}", feature.name());
        }
        assert_eq!(Features::default(), Features {
            plugins: false,
            js_plugin_host: false,
            web_client: false,
            desktop: false,
            sharing: false,
            sounds: false,
        });
    }

    // T02: TOML and JSON user layers override defaults; untouched fields keep
    // defaults (layered merge, REQ-019).
    #[test]
    fn t02_user_layers_override_defaults() {
        let toml_layer = ConfigLayer::from_toml_str(USER_TOML).unwrap();
        let config = Config::resolve(&[toml_layer]).unwrap();
        assert_eq!(config.theme.mode, ThemeMode::Light);
        assert_eq!(config.theme.name.as_deref(), Some("ocean"));
        assert_eq!(config.shell.timeout_ms, 60_000);
        assert_eq!(config.permission.bash, PermissionMode::Ask);
        assert_eq!(config.shell.max_output_bytes, 65_536);

        let json_layer = ConfigLayer::from_json_str(r##"{"theme":{"mode":"system","custom":{"border":"#00ff88"}}}"##).unwrap();
        let config = Config::resolve(&[json_layer]).unwrap();
        assert_eq!(config.theme.mode, ThemeMode::System);
        assert_eq!(config.theme.custom.get("border").map(String::as_str), Some("#00ff88"));
    }

    // T03: precedence defaults -> user file -> env -> CLI, with string
    // coercion for booleans and numbers.
    #[test]
    fn t03_env_and_cli_layers_win_in_order() {
        let user = ConfigLayer::from_json_str(r#"{"theme":{"mode":"light"},"model":{"temperature":0.2}}"#).unwrap();
        let env = ConfigLayer::from_env_vars([
            ("OPENCODE_RK_THEME__MODE".to_string(), "system".to_string()),
            ("OPENCODE_RK_COMPACTION__ENABLED".to_string(), "false".to_string()),
            ("OPENCODE_RK_MODEL__MAX_OUTPUT_TOKENS".to_string(), "4096".to_string()),
            ("UNRELATED_ENV".to_string(), "ignored".to_string()),
        ])
        .unwrap();
        let cli = ConfigLayer::from_pairs([("theme.mode", "dark"), ("model.temperature", "0.5")]).unwrap();

        let config = Config::resolve(&[user.clone(), env.clone(), cli]).unwrap();
        assert_eq!(config.theme.mode, ThemeMode::Dark, "CLI must win over env");
        assert!(!config.compaction.enabled, "env must win over user file and default");
        assert_eq!(config.model.max_output_tokens, Some(4096));
        assert_eq!(config.model.temperature, Some(0.5));

        let config = Config::resolve(&[user, env]).unwrap();
        assert_eq!(config.theme.mode, ThemeMode::System, "env must win over user file");
    }

    // T04: semantic validation rejects parseable but invalid values.
    #[test]
    fn t04_validation_rejects_invalid_values() {
        let bad_color = ConfigLayer::from_json_str(r##"{"theme":{"custom":{"border":"red"}}}"##).unwrap();
        assert!(Config::resolve(&[bad_color]).is_err());

        let bad_color_len = ConfigLayer::from_json_str(r##"{"theme":{"custom":{"border":"#00ff"}}}"##).unwrap();
        assert!(Config::resolve(&[bad_color_len]).is_err());

        let bad_temperature = ConfigLayer::from_json_str(r#"{"model":{"temperature":5.0}}"#).unwrap();
        assert!(Config::resolve(&[bad_temperature]).is_err());

        let zero_tokens = ConfigLayer::from_json_str(r#"{"model":{"max_output_tokens":0}}"#).unwrap();
        assert!(Config::resolve(&[zero_tokens]).is_err());

        let zero_timeout = ConfigLayer::from_json_str(r#"{"shell":{"timeout_ms":0}}"#).unwrap();
        assert!(Config::resolve(&[zero_timeout]).is_err());

        let zero_output = ConfigLayer::from_json_str(r#"{"shell":{"max_output_bytes":0}}"#).unwrap();
        assert!(Config::resolve(&[zero_output]).is_err());

        let empty_program = ConfigLayer::from_json_str(r#"{"shell":{"program":"  "}}"#).unwrap();
        assert!(Config::resolve(&[empty_program]).is_err());

        let zero_threshold = ConfigLayer::from_json_str(r#"{"compaction":{"threshold_tokens":0}}"#).unwrap();
        assert!(Config::resolve(&[zero_threshold]).is_err());

        let zero_turns = ConfigLayer::from_json_str(r#"{"compaction":{"keep_recent_turns":0}}"#).unwrap();
        assert!(Config::resolve(&[zero_turns]).is_err());

        let empty_name = ConfigLayer::from_json_str(r#"{"theme":{"name":"  "}}"#).unwrap();
        assert!(Config::resolve(&[empty_name]).is_err());

        let alpha_custom = ConfigLayer::from_json_str(r##"{"theme":{"custom":{"border":"#00ff88AA"}}}"##).unwrap();
        assert!(Config::resolve(&[alpha_custom]).is_ok());
    }

    // T05: feature flags gate runtime surfaces and stay parseable by name.
    #[test]
    fn t05_feature_flags_gate_and_toggle() {
        let mut features = Features::default();
        assert_eq!(features.require(Feature::Sharing), Err(ConfigError::FeatureDisabled("sharing")));
        features.set(Feature::Sharing, true);
        assert!(features.enabled(Feature::Sharing));
        assert_eq!(features.require(Feature::Sharing), Ok(()));
        assert!(!features.enabled(Feature::Plugins), "other flags stay off");

        assert_eq!(Feature::parse("web_client"), Some(Feature::WebClient));
        assert_eq!(Feature::parse("js_plugin_host"), Some(Feature::JsPluginHost));
        assert_eq!(Feature::parse("nonexistent"), None);

        let layer = ConfigLayer::from_env_vars([("OPENCODE_RK_FEATURES__PLUGINS".to_string(), "true".to_string())]).unwrap();
        let config = Config::resolve(&[layer]).unwrap();
        assert!(config.features.plugins);
        assert_eq!(config.features.require(Feature::Plugins), Ok(()));
        assert_eq!(config.features.require(Feature::Desktop), Err(ConfigError::FeatureDisabled("desktop")));
    }

    // Layer construction rejects malformed or non-object input; unknown keys
    // stay lenient; serialization round-trips.
    #[test]
    fn layers_reject_bad_input_and_roundtrip() {
        assert!(ConfigLayer::from_toml_str("= broken").is_err());
        assert!(ConfigLayer::from_json_str("[1,2]").is_err());
        assert!(ConfigLayer::from_json_str("not json").is_err());
        assert!(ConfigLayer::from_pairs([("", "x")]).is_err());
        assert!(ConfigLayer::from_pairs([("theme..mode", "x")]).is_err());

        let unknown_keys = ConfigLayer::from_json_str(r#"{"future_key":123,"theme":{"mode":"light"}}"#).unwrap();
        let config = Config::resolve(&[unknown_keys]).unwrap();
        assert_eq!(config.theme.mode, ThemeMode::Light);

        let empty_env = ConfigLayer::from_env_vars(Vec::new()).unwrap();
        let config = Config::resolve(&[empty_env]).unwrap();
        assert_eq!(config, Config::default());

        let config = Config::resolve(&[]).unwrap();
        let text = serde_json::to_string(&config).unwrap();
        let round: Config = serde_json::from_str(&text).unwrap();
        assert_eq!(config, round);
    }
}
