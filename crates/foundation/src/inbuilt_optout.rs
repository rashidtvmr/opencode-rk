//! Unified opt-out settings for four inbuilt subsystems (REL-007).
//!
//! Additive fragment for the foundation config: `InbuiltFeatures` defaults
//! all-on, each flag independently disableable via layered resolve
//! (defaults < file < env < CLI per BASE-006). `require` gates construction
//! so a disabled subsystem costs zero I/O, zero threads, zero retained bytes.
//!
//! Integrator seam: add `pub mod inbuilt_optout;` to the foundation `lib.rs`
//! and `pub inbuilt: InbuiltFeatures` with `#[serde(default)]` to `Config` in
//! `config.rs`; old files then resolve unchanged and `Config::resolve` merges
//! the `inbuilt` subtree with the existing defaults < file < env < CLI order.

use serde::{Deserialize, Serialize};

use crate::config::{ConfigError, ConfigLayer};

fn default_on() -> bool {
    true
}

/// Four inbuilt subsystems, each independently disableable. Default is all
/// on (mirrors `Features` default-off precedent from BASE-006, inverted).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InbuiltFeatures {
    #[serde(default = "default_on")]
    pub codebase_index: bool,
    #[serde(default = "default_on")]
    pub rtk_filter: bool,
    #[serde(default = "default_on")]
    pub terse_mode: bool,
    #[serde(default = "default_on")]
    pub telemetry: bool,
}

impl Default for InbuiltFeatures {
    fn default() -> Self {
        Self {
            codebase_index: true,
            rtk_filter: true,
            terse_mode: true,
            telemetry: true,
        }
    }
}

/// Inbuilt subsystem identifiers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptOutFeature {
    CodebaseIndex,
    RtkFilter,
    TerseMode,
    Telemetry,
}

impl OptOutFeature {
    /// Every known flag, used for exhaustive default-on assertions.
    pub const ALL: &'static [OptOutFeature] = &[
        OptOutFeature::CodebaseIndex,
        OptOutFeature::RtkFilter,
        OptOutFeature::TerseMode,
        OptOutFeature::Telemetry,
    ];

    /// Stable flag name used in config files and environment variables.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            OptOutFeature::CodebaseIndex => "codebase_index",
            OptOutFeature::RtkFilter => "rtk_filter",
            OptOutFeature::TerseMode => "terse_mode",
            OptOutFeature::Telemetry => "telemetry",
        }
    }

    /// Parse a flag name. Unknown names return `None` instead of panicking.
    #[must_use]
    pub fn parse(name: &str) -> Option<OptOutFeature> {
        Self::ALL.iter().copied().find(|f| f.name() == name)
    }
}

impl InbuiltFeatures {
    #[must_use]
    pub fn enabled(&self, feature: OptOutFeature) -> bool {
        match feature {
            OptOutFeature::CodebaseIndex => self.codebase_index,
            OptOutFeature::RtkFilter => self.rtk_filter,
            OptOutFeature::TerseMode => self.terse_mode,
            OptOutFeature::Telemetry => self.telemetry,
        }
    }

    pub fn set(&mut self, feature: OptOutFeature, on: bool) {
        match feature {
            OptOutFeature::CodebaseIndex => self.codebase_index = on,
            OptOutFeature::RtkFilter => self.rtk_filter = on,
            OptOutFeature::TerseMode => self.terse_mode = on,
            OptOutFeature::Telemetry => self.telemetry = on,
        }
    }

    /// Runtime gate: error unless the subsystem is enabled. Call sites use
    /// this before constructing anything, so a disabled subsystem costs zero
    /// I/O, zero threads, zero retained bytes.
    pub fn require(&self, feature: OptOutFeature) -> Result<(), ConfigError> {
        if self.enabled(feature) {
            Ok(())
        } else {
            Err(ConfigError::FeatureDisabled(feature.name()))
        }
    }

    /// Resolve the `inbuilt` subtree from existing layers (later layers win
    /// per-key, untouched keys keep defaults, unknown keys ignored). Same
    /// merge semantics as `Config::resolve`, scoped to this fragment until
    /// the integrator wires the `Config.inbuilt` field.
    pub fn resolve(layers: &[ConfigLayer]) -> Result<Self, ConfigError> {
        let mut merged = serde_json::to_value(Self::default())
            .map_err(|error| ConfigError::InvalidJson(error.to_string()))?;
        for layer in layers {
            if let Some(section) = layer.value().get("inbuilt") {
                merge_layer(&mut merged, section.clone());
            }
        }
        serde_json::from_value(merged)
            .map_err(|error| ConfigError::InvalidValue(error.to_string()))
    }
}

fn merge_layer(base: &mut serde_json::Value, over: serde_json::Value) {
    match (base, over) {
        (serde_json::Value::Object(base_map), serde_json::Value::Object(over_map)) => {
            for (key, value) in over_map {
                merge_layer(
                    base_map.entry(key).or_insert(serde_json::Value::Null),
                    value,
                );
            }
        }
        (base, over) => *base = over,
    }
}
