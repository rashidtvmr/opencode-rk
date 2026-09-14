//! Catalog statistics: plugin totals, loaded count, and error count.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

/// Aggregated catalog statistics.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CatalogStats {
    /// Total number of plugins known to the catalog.
    pub total_plugins: u64,
    /// Number of plugins successfully loaded.
    pub loaded_count: u64,
    /// Number of plugins that failed to load.
    pub error_count: u64,
}

impl CatalogStats {
    /// Record one plugin as successfully loaded.
    pub fn record_loaded(&mut self) {
        self.loaded_count += 1;
    }

    /// Record one plugin as failed to load.
    pub fn record_error(&mut self) {
        self.error_count += 1;
    }
}

/// Persist stats as JSON to `path`.
///
/// # Errors
/// Returns an `io::Error` if the file cannot be created or written.
pub fn save_stats(path: &Path, stats: &CatalogStats) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(stats).map_err(io::Error::other)?;
    std::fs::write(path, bytes)
}

/// Load stats from a JSON file written by [`save_stats`].
///
/// # Errors
/// Returns an `io::Error` if the file cannot be read or parsed.
pub fn load_stats(path: &Path) -> io::Result<CatalogStats> {
    let bytes = std::fs::read(path)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

/// Render a human-readable summary of the stats.
#[must_use]
pub fn stats_summary(stats: &CatalogStats) -> String {
    format!(
        "plugins total={} loaded={} errors={}",
        stats.total_plugins, stats.loaded_count, stats.error_count
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample() -> CatalogStats {
        CatalogStats {
            total_plugins: 10,
            loaded_count: 2,
            error_count: 3,
        }
    }

    #[test]
    fn default_values() {
        let empty = CatalogStats::default();
        assert_eq!(empty.total_plugins, 0);
        assert_eq!(empty.loaded_count, 0);
        assert_eq!(empty.error_count, 0);
    }

    #[test]
    fn save_and_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("stats.json");
        save_stats(&path, &sample()).unwrap();
        let loaded = load_stats(&path).unwrap();
        assert_eq!(loaded, sample());
    }

    #[test]
    fn summary_contains_counts() {
        let summary = stats_summary(&sample());
        assert!(summary.contains("total=10"));
        assert!(summary.contains("loaded=2"));
        assert!(summary.contains("errors=3"));
    }

    #[test]
    fn loaded_tracking() {
        let mut stats = CatalogStats::default();
        stats.total_plugins = 2;
        stats.record_loaded();
        stats.record_loaded();
        assert_eq!(stats.loaded_count, 2);
        assert_eq!(stats.error_count, 0);
    }

    #[test]
    fn error_count() {
        let mut stats = CatalogStats::default();
        stats.record_error();
        stats.record_error();
        assert_eq!(stats.error_count, 2);
        assert_eq!(stats.loaded_count, 0);
    }
}
