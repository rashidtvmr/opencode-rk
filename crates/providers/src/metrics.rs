//! Provider metrics module for tracking calls, errors, and latency.

use std::collections::HashMap;

use serde::Serialize;

use opencode_rk_contracts::ProviderId;

/// Simple histogram for latency distribution tracking.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Histogram {
    buckets: HashMap<u64, u64>,
    sum: f64,
    count: u64,
}

impl Histogram {
    /// Creates a new empty histogram.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a latency value in milliseconds.
    pub fn record(&mut self, value_ms: f64) {
        let bucket = value_ms as u64;
        *self.buckets.entry(bucket).or_insert(0) += 1;
        self.sum += value_ms;
        self.count += 1;
    }

    /// Returns the count of recorded values.
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Returns the sum of all recorded values.
    #[must_use]
    pub fn sum(&self) -> f64 {
        self.sum
    }

    /// Returns the average of recorded values.
    #[must_use]
    pub fn avg(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }

    /// Returns a copy of the bucket distribution.
    #[must_use]
    pub fn buckets(&self) -> HashMap<u64, u64> {
        self.buckets.clone()
    }

    /// Clears all recorded data.
    pub fn clear(&mut self) {
        self.buckets.clear();
        self.sum = 0.0;
        self.count = 0;
    }
}

impl std::fmt::Display for Histogram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Histogram(count={}, avg={:.2}ms, sum={:.2}ms)",
            self.count,
            self.avg(),
            self.sum
        )
    }
}

/// Metrics collected for a single provider.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ProviderMetrics {
    /// Total number of calls made to this provider.
    pub calls_total: u64,
    /// Total number of failed calls.
    pub errors_total: u64,
    /// Latency distribution in milliseconds.
    pub latency_ms: Histogram,
    /// Current number of active (in-flight) requests.
    pub active_requests: u64,
}

impl ProviderMetrics {
    /// Creates a new empty metrics instance.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears all metrics to zero.
    pub fn reset(&mut self) {
        self.calls_total = 0;
        self.errors_total = 0;
        self.latency_ms.clear();
        self.active_requests = 0;
    }
}

/// Recorder for tracking per-provider metrics.
#[derive(Clone, Debug, Default)]
pub struct ProviderMetricsRecorder {
    per_provider: HashMap<ProviderId, ProviderMetrics>,
}

impl ProviderMetricsRecorder {
    /// Creates a new empty recorder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            per_provider: HashMap::new(),
        }
    }

    /// Records a provider call with the given latency and success status.
    pub fn record_call(&mut self, provider_id: ProviderId, latency_ms: f64, success: bool) {
        let metrics = self.per_provider.entry(provider_id).or_default();
        metrics.calls_total += 1;
        metrics.latency_ms.record(latency_ms);

        if success {
            // Decrement active requests on success
            if metrics.active_requests > 0 {
                metrics.active_requests -= 1;
            }
        } else {
            metrics.errors_total += 1;
            // Decrement active requests on error
            if metrics.active_requests > 0 {
                metrics.active_requests -= 1;
            }
        }
    }

    /// Increments the active request count for a provider.
    pub fn increment_active(&mut self, provider_id: &ProviderId) {
        let metrics = self.per_provider.entry(provider_id.clone()).or_default();
        metrics.active_requests += 1;
    }

    /// Returns a snapshot of all provider metrics as a vector of (name, metrics) tuples.
    #[must_use]
    pub fn snapshot(&self) -> Vec<(String, ProviderMetrics)> {
        self.per_provider
            .iter()
            .map(|(id, metrics)| (id.to_string(), metrics.clone()))
            .collect()
    }

    /// Clears all recorded metrics.
    pub fn reset(&mut self) {
        self.per_provider.clear();
    }

    /// Gets metrics for a specific provider, if they exist.
    #[must_use]
    pub fn get(&self, provider_id: &ProviderId) -> Option<&ProviderMetrics> {
        self.per_provider.get(provider_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_provider_id() -> ProviderId {
        ProviderId::new("openai").expect("valid provider id")
    }

    #[test]
    fn records_call() {
        let mut recorder = ProviderMetricsRecorder::new();
        let provider_id = sample_provider_id();
        let latency = 150.5;

        recorder.record_call(provider_id.clone(), latency, true);

        let metrics = recorder.get(&provider_id).expect("metrics should exist");
        assert_eq!(metrics.calls_total, 1);
        assert_eq!(metrics.errors_total, 0);
        assert_eq!(metrics.latency_ms.count(), 1);
        assert!((metrics.latency_ms.avg() - latency).abs() < 0.01);
    }

    #[test]
    fn error_incremented() {
        let mut recorder = ProviderMetricsRecorder::new();
        let provider_id = sample_provider_id();

        recorder.record_call(provider_id.clone(), 100.0, false);

        let metrics = recorder.get(&provider_id).expect("metrics should exist");
        assert_eq!(metrics.calls_total, 1);
        assert_eq!(metrics.errors_total, 1);
    }

    #[test]
    fn latency_recorded() {
        let mut recorder = ProviderMetricsRecorder::new();
        let provider_id = sample_provider_id();

        recorder.record_call(provider_id.clone(), 50.0, true);
        recorder.record_call(provider_id.clone(), 100.0, true);
        recorder.record_call(provider_id.clone(), 150.0, false);

        let metrics = recorder.get(&provider_id).expect("metrics should exist");
        assert_eq!(metrics.latency_ms.count(), 3);
        assert!((metrics.latency_ms.avg() - 100.0).abs() < 0.01);

        let buckets = metrics.latency_ms.buckets();
        assert_eq!(buckets.get(&50), Some(&1));
        assert_eq!(buckets.get(&100), Some(&1));
        assert_eq!(buckets.get(&150), Some(&1));
    }

    #[test]
    fn snapshot_returns_all() {
        let mut recorder = ProviderMetricsRecorder::new();
        let openai_id = sample_provider_id();
        let anthropic_id = ProviderId::new("anthropic").expect("valid provider id");

        recorder.record_call(openai_id.clone(), 50.0, true);
        recorder.record_call(anthropic_id.clone(), 75.0, true);

        let snapshot = recorder.snapshot();
        assert_eq!(snapshot.len(), 2);

        let provider_names: Vec<&str> = snapshot.iter().map(|(name, _)| name.as_str()).collect();
        assert!(provider_names.contains(&"openai"));
        assert!(provider_names.contains(&"anthropic"));
    }

    #[test]
    fn reset_clears() {
        let mut recorder = ProviderMetricsRecorder::new();
        let provider_id = sample_provider_id();

        recorder.record_call(provider_id.clone(), 50.0, true);
        recorder.record_call(provider_id.clone(), 100.0, false);
        recorder.reset();

        assert!(recorder.snapshot().is_empty());
        assert!(recorder.get(&provider_id).is_none());
    }
}
