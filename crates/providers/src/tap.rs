//! Provider tap module.
//!
//! Per-provider access control for request-level gating. Providers can be
//! allowed, denied, or rate-limited on a per-request basis. The [`ProviderTap`]
//! struct holds rules keyed by provider id and tracks how many requests have
//! been recorded for rate-limited providers.

use std::collections::HashMap;

/// A rule governing how requests to a provider are handled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TapRule {
    /// Requests are always allowed.
    Allow,
    /// Requests are always denied.
    Deny,
    /// Requests are allowed up to `limit` per the request counter managed
    /// by [`ProviderTap::record_request`]. Once the counter reaches the
    /// limit, further requests are denied until the counter is reset.
    RateLimit(u64),
}

/// Decision produced by evaluating a tap rule for a single request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TapDecision {
    /// Whether the request is permitted.
    pub allowed: bool,
    /// Remaining requests before a rate limit is exhausted.
    /// `None` when the rule is not a rate limit or when no limit applies.
    pub limit_remaining: Option<u64>,
    /// Human-readable reason for the decision, if any.
    pub reason: Option<String>,
}

/// Per-provider tap controller.
///
/// Maintains a set of rules keyed by provider id and a request counter used
/// to enforce `RateLimit` rules.
#[derive(Debug, Default)]
pub struct ProviderTap {
    /// Tap rules keyed by provider id.
    rules: HashMap<String, TapRule>,
    /// Per-provider request counts used for rate limiting.
    request_count: HashMap<String, u64>,
}

impl ProviderTap {
    /// Creates a new, empty tap controller.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces a tap rule for a provider.
    pub fn add_rule(&mut self, provider_id: &str, rule: TapRule) {
        self.rules.insert(provider_id.to_string(), rule);
        // Reset the request counter when a new rule is added so that a
        // RateLimit rule starts from a clean slate.
        self.request_count.remove(provider_id);
    }

    /// Removes a tap rule for a provider.
    ///
    /// Returns `true` if a rule was present and removed, `false` otherwise.
    pub fn remove_rule(&mut self, provider_id: &str) -> bool {
        let removed = self.rules.remove(provider_id).is_some();
        if removed {
            // Clear the counter so a future rule starts fresh.
            self.request_count.remove(provider_id);
        }
        removed
    }

    /// Records a request for a provider, incrementing its rate-limit counter.
    ///
    /// Returns the new request count after incrementing.
    pub fn record_request(&mut self, provider_id: &str) -> u64 {
        let count = self.request_count.entry(provider_id.to_string()).or_insert(0);
        *count += 1;
        *count
    }

    /// Evaluates the tap rule for a provider and returns a decision.
    ///
    /// When no rule is present the request is allowed. When a `RateLimit` rule
    /// is present, the decision considers the current request count recorded
    /// via [`record_request`](Self::record_request).
    pub fn tap(&mut self, provider_id: &str) -> TapDecision {
        match self.rules.get(provider_id) {
            Some(TapRule::Allow) => TapDecision {
                allowed: true,
                limit_remaining: None,
                reason: None,
            },
            Some(TapRule::Deny) => TapDecision {
                allowed: false,
                limit_remaining: None,
                reason: Some("denied by tap rule".to_string()),
            },
            Some(TapRule::RateLimit(limit)) => {
                let current = self.request_count.get(provider_id).copied().unwrap_or(0);
                if current < *limit {
                    TapDecision {
                        allowed: true,
                        limit_remaining: Some(*limit - current),
                        reason: None,
                    }
                } else {
                    TapDecision {
                        allowed: false,
                        limit_remaining: Some(0),
                        reason: Some("rate limit exceeded".to_string()),
                    }
                }
            }
            None => TapDecision {
                allowed: true,
                limit_remaining: None,
                reason: Some("no tap rule configured".to_string()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_rule() {
        let mut tap = ProviderTap::new();
        tap.add_rule("provider-a", TapRule::Allow);

        let decision = tap.tap("provider-a");
        assert!(decision.allowed);
        assert_eq!(decision.limit_remaining, None);
        assert!(decision.reason.is_none());
    }

    #[test]
    fn deny_rule() {
        let mut tap = ProviderTap::new();
        tap.add_rule("provider-b", TapRule::Deny);

        let decision = tap.tap("provider-b");
        assert!(!decision.allowed);
        assert_eq!(decision.limit_remaining, None);
        assert_eq!(
            decision.reason.as_deref(),
            Some("denied by tap rule")
        );
    }

    #[test]
    fn rate_limit() {
        let mut tap = ProviderTap::new();
        tap.add_rule("provider-c", TapRule::RateLimit(5));

        // Before any requests are recorded, all remaining.
        let d = tap.tap("provider-c");
        assert!(d.allowed);
        assert_eq!(d.limit_remaining, Some(5));

        // Record 3 requests.
        tap.record_request("provider-c");
        tap.record_request("provider-c");
        tap.record_request("provider-c");

        let d = tap.tap("provider-c");
        assert!(d.allowed);
        assert_eq!(d.limit_remaining, Some(2));

        // Record 2 more requests to exhaust the limit (count == limit).
        tap.record_request("provider-c");
        tap.record_request("provider-c");

        let d = tap.tap("provider-c");
        assert!(!d.allowed);
        assert_eq!(d.limit_remaining, Some(0));
        assert_eq!(
            d.reason.as_deref(),
            Some("rate limit exceeded")
        );
    }

    #[test]
    fn remove_rule() {
        let mut tap = ProviderTap::new();
        tap.add_rule("provider-d", TapRule::Deny);

        // Denied while rule exists.
        let d = tap.tap("provider-d");
        assert!(!d.allowed);

        // Remove the rule.
        assert!(tap.remove_rule("provider-d"));

        // After removal, requests are allowed (no rule present).
        let d = tap.tap("provider-d");
        assert!(d.allowed);
        assert_eq!(d.limit_remaining, None);
        assert_eq!(
            d.reason.as_deref(),
            Some("no tap rule configured")
        );

        // Removing a non-existent rule returns false.
        assert!(!tap.remove_rule("provider-d"));
    }

    #[test]
    fn record_and_check() {
        let mut tap = ProviderTap::new();
        tap.add_rule("provider-e", TapRule::RateLimit(3));

        // No requests recorded yet.
        let d = tap.tap("provider-e");
        assert!(d.allowed);
        assert_eq!(d.limit_remaining, Some(3));

        // Record request 1, count becomes 1.
        let count = tap.record_request("provider-e");
        assert_eq!(count, 1);
        let d = tap.tap("provider-e");
        assert!(d.allowed);
        assert_eq!(d.limit_remaining, Some(2));

        // Record request 2, count becomes 2.
        let count = tap.record_request("provider-e");
        assert_eq!(count, 2);
        let count = tap.record_request("provider-e");
        assert_eq!(count, 3);
        // At count=3 with limit=3, next request should be denied.
        let d = tap.tap("provider-e");
        assert!(!d.allowed, "expected denied at count=3, limit=3");
        assert_eq!(d.limit_remaining, Some(0));
    }
}