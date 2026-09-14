//! Catalog health checks for plugins.

use std::collections::VecDeque;
use std::time::{Duration, SystemTime};

/// Health status of a plugin.
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    /// Plugin is fully healthy.
    Healthy,
    /// Plugin is degraded with a reason string.
    Degraded(String),
    /// Plugin is unhealthy with a reason string.
    Unhealthy(String),
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Degraded(msg) => write!(f, "degraded: {}", msg),
            HealthStatus::Unhealthy(msg) => write!(f, "unhealthy: {}", msg),
        }
    }
}

/// Health record for a single plugin.
#[derive(Debug, Clone)]
pub struct PluginHealth {
    /// Plugin identifier.
    pub plugin_id: String,
    /// Current health status.
    pub status: HealthStatus,
    /// Last check timestamp.
    pub last_check: SystemTime,
    /// Response time in milliseconds.
    pub response_ms: u64,
}

/// Health checker for tracking plugin health records.
pub struct HealthChecker {
    /// Records of plugin health checks, order preserved for LRU.
    checks: VecDeque<PluginHealth>,
    /// Maximum history size.
    max_history: usize,
}

impl HealthChecker {
    /// Creates a new HealthChecker with specified max history size.
    pub fn new(max_history: usize) -> Self {
        Self {
            checks: VecDeque::with_capacity(max_history),
            max_history,
        }
    }

    /// Records a health check result for a plugin.
    /// If history is full, oldest entry is removed.
    pub fn record(&mut self, plugin_id: String, status: HealthStatus, response_ms: u64) {
        let check = PluginHealth {
            plugin_id,
            status,
            last_check: SystemTime::now(),
            response_ms,
        };

        if self.checks.len() >= self.max_history && self.max_history > 0 {
            self.checks.pop_front();
        }

        self.checks.push_back(check);
    }

    /// Gets the current health record for a plugin by ID.
    pub fn check(&self, plugin_id: &str) -> Option<&PluginHealth> {
        self.checks.back().and_then(|latest| {
            if latest.plugin_id == plugin_id {
                Some(latest)
            } else {
                None
            }
        })
    }

    /// Returns true if all recorded plugins are healthy.
    pub fn is_healthy(&self) -> bool {
        self.checks.front().map_or(true, |_| {
            self.checks
                .iter()
                .all(|c| matches!(c.status, HealthStatus::Healthy))
        })
    }

    /// Returns references to all health records.
    pub fn get_all(&self) -> Vec<&PluginHealth> {
        self.checks.iter().collect()
    }

    /// Removes entries older than the specified age.
    /// Returns the number of entries removed.
    pub fn prune_old(&mut self, now: SystemTime, max_age_secs: u64) -> usize {
        let limit = Duration::from_secs(max_age_secs);
        let before = now
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            - limit;

        let len_before = self.checks.len();
        self.checks.retain(|c| {
            c.last_check
                .duration_since(SystemTime::UNIX_EPOCH)
                .is_ok_and(|d| d > before)
        });
        len_before - self.checks.len()
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_check() {
        let mut checker = HealthChecker::new(10);
        checker.record("plugin-a".to_string(), HealthStatus::Healthy, 100);

        let health = checker.check("plugin-a");
        assert!(health.is_some());
        let h = health.unwrap();
        assert_eq!(h.plugin_id, "plugin-a");
        assert_eq!(h.status, HealthStatus::Healthy);
        assert_eq!(h.response_ms, 100);
    }

    #[test]
    fn all_healthy() {
        let mut checker = HealthChecker::new(10);
        checker.record("plugin-a".to_string(), HealthStatus::Healthy, 50);
        checker.record("plugin-b".to_string(), HealthStatus::Healthy, 75);

        assert!(checker.is_healthy());
    }

    #[test]
    fn one_degraded_makes_unhealthy() {
        let mut checker = HealthChecker::new(10);
        checker.record("plugin-a".to_string(), HealthStatus::Healthy, 50);
        checker.record(
            "plugin-b".to_string(),
            HealthStatus::Degraded("slow".to_string()),
            200,
        );

        assert!(!checker.is_healthy());
    }

    #[test]
    fn prune_old_removes() {
        let mut checker = HealthChecker::new(10);
        let old_time = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
        let new_time = SystemTime::UNIX_EPOCH + Duration::from_secs(2000);

        let old_check = PluginHealth {
            plugin_id: "old-plugin".to_string(),
            status: HealthStatus::Healthy,
            last_check: old_time,
            response_ms: 10,
        };
        let new_check = PluginHealth {
            plugin_id: "new-plugin".to_string(),
            status: HealthStatus::Healthy,
            last_check: new_time,
            response_ms: 10,
        };

        checker.checks.push_back(old_check);
        checker.checks.push_back(new_check);

        let removed = checker.prune_old(SystemTime::UNIX_EPOCH + Duration::from_secs(1500), 300);
        assert_eq!(removed, 1);
        assert_eq!(checker.checks.len(), 1);
        assert_eq!(checker.checks.front().unwrap().plugin_id, "new-plugin");
    }

    #[test]
    fn get_all_returns_all() {
        let mut checker = HealthChecker::new(10);
        checker.record("plugin-a".to_string(), HealthStatus::Healthy, 50);
        checker.record("plugin-b".to_string(), HealthStatus::Healthy, 75);

        let all = checker.get_all();
        assert_eq!(all.len(), 2);
        assert!(all[0].plugin_id == "plugin-a" || all[0].plugin_id == "plugin-b");
        assert!(all[1].plugin_id == "plugin-a" || all[1].plugin_id == "plugin-b");
    }
}
