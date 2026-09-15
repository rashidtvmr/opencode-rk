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
    /// Provider is degraded but still usable with reduced capacity.
    Degraded,
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
    /// Unhealthy count threshold (backoff after this many errors).
    unhealthy_count: HashMap<String, u32>,
}

impl HealthMonitor {
    /// Creates a new monitor with the given staleness timeout.
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            providers: HashMap::new(),
            timeout_secs,
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
            Some(h) => h.status == HealthStatus::Healthy && !self.is_expired(h.last_check),
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
    /// Once a provider accumulates 3 consecutive failures it is marked
    /// [`HealthStatus::Unhealthy`].
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
        entry.latency_ms = 0;
        entry.last_check = SystemTime::now();
        if *count >= 3 {
            entry.status = HealthStatus::Unhealthy;
        } else if *count == 1 {
            entry.status = HealthStatus::Healthy;
        } else if *count == 2 {
            entry.status = HealthStatus::Degraded;
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

    /// Returns whether a provider should be retried (backoff after errors).
    ///
    /// A provider should be retried when it's not fully unhealthy (i.e.,
    /// still Healthy or Degraded). Backoff kicks in after 3 errors.
    pub fn should_retry(&self, provider_id: &str) -> bool {
        match self.providers.get(provider_id) {
            Some(h) => h.status != HealthStatus::Unhealthy,
            None => true,
        }
    }

    /// Returns the current consecutive failure count for a provider.
    pub fn failure_count(&self, provider_id: &str) -> u32 {
        self.unhealthy_count.get(provider_id).copied().unwrap_or(0)
    }

    /// Returns all unhealthy provider IDs.
    pub fn unhealthy_list(&self) -> Vec<String> {
        self.providers
            .iter()
            .filter_map(|(id, h)| {
                if h.status == HealthStatus::Unhealthy {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect()
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
        Self::new(60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_provider() {
        let mut monitor = HealthMonitor::new(60);
        monitor.record_success("provider-a", 150);

        let h = monitor.get("provider-a").unwrap();
        assert_eq!(h.provider_id, "provider-a");
        assert_eq!(h.status, HealthStatus::Healthy);
        assert_eq!(h.latency_ms, 150);
        assert!(monitor.check_health("provider-a"));
        assert!(monitor.is_healthy("provider-a"));
        assert!(monitor.should_retry("provider-a"));
    }

    #[test]
    fn error_threshold_triggers() {
        let mut monitor = HealthMonitor::new(60);

        // Record failures until threshold is reached
        monitor.record_failure("provider-b");
        assert_eq!(monitor.failure_count("provider-b"), 1);
        assert!(monitor.is_healthy("provider-b"));

        monitor.record_failure("provider-b");
        monitor.record_failure("provider-b");

        // Should be unhealthy after 3 errors
        let h = monitor.get("provider-b").unwrap();
        assert_eq!(h.status, HealthStatus::Unhealthy);
        assert!(!monitor.is_healthy("provider-b"));
        assert!(!monitor.check_health("provider-b"));
        assert!(!monitor.should_retry("provider-b"));
    }

    #[test]
    fn latency_recorded() {
        let mut monitor = HealthMonitor::new(60);

        monitor.record_success("provider-c", 42);
        let h = monitor.get("provider-c").unwrap();
        assert_eq!(h.latency_ms, 42);

        monitor.record_success("provider-c", 105);
        let h = monitor.get("provider-c").unwrap();
        assert_eq!(h.latency_ms, 105);

        // Failed checks record latency as 0
        monitor.record_failure("provider-c");
        let h = monitor.get("provider-c").unwrap();
        assert_eq!(h.latency_ms, 0);
    }

    #[test]
    fn unhealthy_list() {
        let mut monitor = HealthMonitor::new(60);

        // All healthy by default
        assert!(monitor.unhealthy_list().is_empty());

        // Add some providers
        monitor.record_success("provider-a", 50);
        monitor.record_success("provider-b", 100);

        // Two healthy providers - should be empty
        assert!(monitor.unhealthy_list().is_empty());

        // Make provider-a unhealthy
        monitor.record_failure("provider-a");
        monitor.record_failure("provider-a");
        monitor.record_failure("provider-a");

        let unhealthy = monitor.unhealthy_list();
        assert_eq!(unhealthy.len(), 1);
        assert!(unhealthy.contains(&"provider-a".to_string()));
        assert!(!unhealthy.contains(&"provider-b".to_string()));
    }

    #[test]
    fn backoff_after_errors() {
        let mut monitor = HealthMonitor::new(60);

        // No errors - should retry
        assert!(monitor.should_retry("provider-x"));

        // First error - still should retry (1 error < 3)
        monitor.record_failure("provider-x");
        assert!(monitor.should_retry("provider-x"));

        // Second error - still should retry (2 errors < 3)
        monitor.record_failure("provider-x");
        assert!(monitor.should_retry("provider-x"));

        // Third error - should NOT retry (backoff starts)
        monitor.record_failure("provider-x");
        assert!(!monitor.should_retry("provider-x"));

        // Success resets the state
        monitor.record_success("provider-x", 100);
        assert!(monitor.should_retry("provider-x"));
        assert_eq!(monitor.failure_count("provider-x"), 0);
    }
}
