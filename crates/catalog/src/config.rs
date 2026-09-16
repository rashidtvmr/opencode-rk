//! Catalog configuration management.
//!
//! Provides typed configuration for the catalog subsystem including
//! directory paths, auto-reload behavior, refresh intervals, and
//! remote source permissions.

use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

/// Catalog configuration for plugin/skills listing and discovery.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogConfig {
    /// Directory containing catalog entries (skills/plugins).
    pub catalog_dir: PathBuf,
    /// Whether to auto-reload catalog on filesystem changes.
    pub auto_reload: bool,
    /// Refresh interval in seconds for polling-based reload.
    pub refresh_interval_secs: u64,
    /// Maximum number of catalog entries to load.
    pub max_entries: usize,
    /// Whether to allow loading entries from remote sources.
    pub allow_remote: bool,
}

impl CatalogConfig {
    /// Load configuration from a TOML file at the given path.
    /// Returns default configuration if the file does not exist.
    pub fn load(path: &std::path::Path) -> Self {
        if path.exists() {
            match fs::read_to_string(path) {
                Ok(content) => toml::from_str(&content).unwrap_or_default(),
                Err(_) => Self::default(),
            }
        } else {
            Self::default()
        }
    }

    /// Save configuration to a TOML file at the given path.
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        fs::write(path, content)
    }

    /// Validate configuration values.
    pub fn validate(&self) -> Result<(), String> {
        if self.max_entries == 0 {
            return Err("max_entries must be greater than 0".to_string());
        }
        Ok(())
    }
}

// Manual Default implementation with sane defaults
impl Default for CatalogConfig {
    fn default() -> Self {
        Self {
            catalog_dir: PathBuf::from("catalogs"),
            auto_reload: true,
            refresh_interval_secs: 30,
            max_entries: 1000,
            allow_remote: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn default_values() {
        let config = CatalogConfig::default();
        assert_eq!(config.auto_reload, true);
        assert_eq!(config.refresh_interval_secs, 30);
        assert_eq!(config.max_entries, 1000);
        assert_eq!(config.allow_remote, false);
        assert_eq!(config.catalog_dir, PathBuf::from("catalogs"));
    }

    #[test]
    fn load_missing_uses_default() {
        let temp_dir = TempDir::new().unwrap();
        let missing_path = temp_dir.path().join("nonexistent.toml");
        let config = CatalogConfig::load(&missing_path);
        assert_eq!(config, CatalogConfig::default());
    }

    #[test]
    fn save_and_reload() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let original = CatalogConfig {
            catalog_dir: PathBuf::from("/custom/catalog"),
            auto_reload: false,
            refresh_interval_secs: 60,
            max_entries: 500,
            allow_remote: true,
        };

        original.save(&config_path).unwrap();
        let reloaded = CatalogConfig::load(&config_path);

        assert_eq!(original, reloaded);
    }

    #[test]
    fn validate_positive_max() {
        let config = CatalogConfig {
            max_entries: 100,
            ..Default::default()
        };
        assert!(config.validate().is_ok());

        let config = CatalogConfig {
            max_entries: 1,
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_rejects_zero() {
        let config = CatalogConfig {
            max_entries: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
        let err = config.validate().unwrap_err();
        assert!(err.contains("max_entries"));
    }
}
