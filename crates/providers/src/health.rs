//! Provider health module.
//!
//! Tracks per-provider health verdicts: success/failure accounting, a
//! configurable consecutive-failure threshold that flips a provider to
//! unhealthy, and a staleness timeout that expires recorded health states.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Health status of a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Provider is healthy and usable.
    Healthy,
    /// Provider is unhealthy and should not be used.
    Unhealthy,
}

/// Health record for a single provider.
#[derive(Debug, Clone)]
pub struct ProviderHealth {
    /// Provider identifier.
    pub provider_id: String,
    /// Current health status.
    pub status: HealthStatus,
    /// Last measured latency in milliseconds.
    pub latency_ms: u64,
    /// Timestamp of the last recorded health transition.
    pub last_check: SystemTime,
}

impl ProviderHealth {
    /// Creates a new health record for a provider.
    pub fn new(provider_id: String, latency_ms: u64) -> Self {
        Self {
            provider_id,
            status: HealthStatus::Healthy,
            latency_ms,
            last_check: SystemTime::now(),
        }
    }
}

/// Monitor that tracks provider health and applies a consecutive-failure
/// threshold plus a staleness timeout.
#[derive(Debug)]
pub struct HealthMonitor {
    /// Health records keyed by provider id.
    providers: HashMap<String, ProviderHealth>,
    /// A recorded health state older than this many seconds is expired.
    timeout_secs: u64,
    /// Consecutive failures required to mark a provider unhealthy.
    unhealthy_threshold: u32,
    /// Consecutive failure count per provider.
    unhealthy_count: HashMap<String, u32>,
}

impl HealthMonitor {
    /// Creates a new monitor with the given staleness timeout and
    /// consecutive-failure threshold.
    pub fn new(timeout_secs: u64, unhealthy_threshold: u32) -> Self {
        Self {
            providers: HashMap::new(),
            timeout_secs,
            unhealthy_threshold,
            unhealthy_count: HashMap::new(),
        }
    }

    /// Runs a health check for a provider.
    ///
    /// Returns `true` when a provider has a healthy record that is not older
    /// than the configured timeout. A provider with no recorded state is
    /// treated as unknown and returns `false`.
    pub fn check_health(&self, provider_id: &str) -> bool {
        match self.providers.get(provider_id) {
            Some(h) => {
                h.status == HealthStatus::Healthy && !self.is_expired(h.last_check)
            }
            None => false,
        }
    }

    /// Records a successful health check, resetting failure state.
    pub fn record_success(&mut self, provider_id: &str, latency_ms: u64) {
        let entry = self
            .providers
            .entry(provider_id.to_string())
            .or_insert_with(|| ProviderHealth::new(provider_id.to_string(), latency_ms));
        entry.status = HealthStatus::Healthy;
        entry.latency_ms = latency_ms;
        entry.last_check = SystemTime::now();
        self.unhealthy_count.remove(provider_id);
    }

    /// Records a failed health check.
    ///
    /// Once a provider accumulates `unhealthy_threshold` consecutive failures
    /// it is marked [`HealthStatus::Unhealthy`].
    pub fn record_failure(&mut self, provider_id: &str) {
        let count = self
            .unhealthy_count
            .entry(provider_id.to_string())
            .or_insert(0);
        *count += 1;

        let entry = self
            .providers
            .entry(provider_id.to_string())
            .or_insert_with(|| ProviderHealth::new(provider_id.to_string(), 0));
        entry.last_check = SystemTime::now();
        if *count >= self.unhealthy_threshold {
            entry.status = HealthStatus::Unhealthy;
        } else {
            entry.status = HealthStatus::Healthy;
        }
    }

    /// Returns whether a provider is currently healthy, ignoring staleness.
    ///
    /// Returns `true` when the provider has no recorded state yet (unchecked
    /// providers are assumed usable) or its recorded status is healthy.
    pub fn is_healthy(&self, provider_id: &str) -> bool {
        match self.providers.get(provider_id) {
            Some(h) => h.status == HealthStatus::Healthy,
            None => true,
        }
    }

    /// Current consecutive failure count for a provider.
    pub fn failure_count(&self, provider_id: &str) -> u32 {
        self.unhealthy_count.get(provider_id).copied().unwrap_or(0)
    }

    /// Returns the current health record for a provider, if any.
    pub fn get(&self, provider_id: &str) -> Option<&ProviderHealth> {
        self.providers.get(provider_id)
    }

    /// Whether a timestamp is older than the configured timeout.
    fn is_expired(&self, last_check: SystemTime) -> bool {
        last_check
            .elapsed()
            .map(|elapsed| elapsed > Duration::from_secs(self.timeout_secs))
            .unwrap_or(true)
    }
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new(60, 3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    #[test]
    fn record_and_check() {
        let mut monitor = HealthMonitor::new(60, 3);
        monitor.record_success("provider-a", 120);

        assert!(monitor.check_health("provider-a"));
        let h = monitor.get("provider-a").unwrap();
        assert_eq!(h.provider_id, "provider-a");
        assert_eq!(h.status, HealthStatus::Healthy);
        assert_eq!(h.latency_ms, 120);
        assert_eq!(monitor.failure_count("provider-a"), 0);
    }

    #[test]
    fn multiple_providers() {
        let mut monitor = HealthMonitor::new(60, 3);
        monitor.record_success("provider-a", 10);
        monitor.record_success("provider-b", 20);

        assert!(monitor.check_health("provider-a"));
        assert!(monitor.check_health("provider-b"));

        // Failures for one provider must not affect the other.
        monitor.record_failure("provider-a");
        monitor.record_failure("provider-a");
        monitor.record_failure("provider-a");
        assert!(!monitor.check_health("provider-a"));
        assert!(monitor.check_health("provider-b"));
        assert_eq!(monitor.failure_count("provider-b"), 0);
    }

    #[test]
    fn timeout_enforced() {
        let mut monitor = HealthMonitor::new(60, 3);
        monitor.record_success("provider-a", 50);
        assert!(monitor.check_health("provider-a"));

        // Stale check timestamp older than the timeout must fail the check.
        let stale = UNIX_EPOCH + Duration::from_secs(1_000);
        if let Some(h) = monitor.providers.get_mut("provider-a") {
            h.last_check = stale;
        }
        assert!(!monitor.check_health("provider-a"));
        // Staleness does not flip the recorded status itself.
        assert!(monitor.is_healthy("provider-a"));
    }

    #[test]
    fn failure_triggers_unhealthy() {
        let mut monitor = HealthMonitor::new(60, 3);
        monitor.record_failure("provider-a");
        monitor.record_failure("provider-a");
        assert!(monitor.is_healthy("provider-a"));

        // Third consecutive failure crosses the threshold.
        monitor.record_failure("provider-a");
        assert!(!monitor.is_healthy("provider-a"));
        assert!(!monitor.check_health("provider-a"));
        assert_eq!(monitor.failure_count("provider-a"), 3);
    }

    #[test]
    fn health_returns_false_after_failures() {
        let mut monitor = HealthMonitor::new(60, 2);
        monitor.record_success("provider-a", 30);
        monitor.record_failure("provider-a");
        assert!(monitor.is_healthy("provider-a"));

        monitor.record_failure("provider-a");
        assert!(!monitor.check_health("provider-a"));
        assert!(!monitor.is_healthy("provider-a"));

        // A success resets the failure counter and the verdict.
        monitor.record_success("provider-a", 40);
        assert!(monitor.check_health("provider-a"));
        assert!(monitor.is_healthy("provider-a"));
        assert_eq!(monitor.failure_count("provider-a"), 0);
    }
}