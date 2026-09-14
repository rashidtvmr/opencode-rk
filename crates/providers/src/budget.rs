//! Provider budget module.
//!
//! Tracks token and cost consumption against configured limits, supporting
//! tiered billing models (Free, PayAsYouGo, Enterprise). Provides budget
//! status inspection, token consumption with over-limit protection, and
//! usage reset for periodic billing windows.

use opencode_rk_contracts::Timestamp;

/// Billing tier for a provider budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BudgetTier {
    /// Free tier: usage capped at a fixed, typically low, token limit with no
    /// paid overflow. Exceeding the limit is an error.
    Free,
    /// Pay-as-you-go: hard token limit with an additional monetary cost limit.
    PayAsYouGo,
    /// Enterprise: typically higher limits, possibly negotiated; same
    /// enforcement semantics as PayAsYouGo but with larger allowances.
    Enterprise,
}

/// Error returned when a token consumption request cannot be satisfied.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum BudgetError {
    /// The requested token count would exceed the token budget.
    #[error("token limit exceeded: requested {requested}, used {used}, limit {limit}")]
    TokenLimitExceeded {
        requested: u64,
        used: u64,
        limit: u64,
    },
    /// The requested token count would exceed the cost budget.
    #[error("cost limit exceeded: requested_cost {requested_cost}, cost {cost}, limit {limit}")]
    CostLimitExceeded {
        requested_cost: f64,
        cost: f64,
        limit: f64,
    },
}

/// Status snapshot of a provider budget at a point in time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BudgetStatus {
    /// Whether usage is within the configured limits.
    pub within_limit: bool,
    /// Percentage of the token limit consumed (0.0 to 100.0).
    pub percent_used: f64,
    /// Tokens remaining before hitting the token limit (0 if exhausted).
    pub remaining: u64,
    /// When the current usage window resets, if known.
    pub reset_at: Option<Timestamp>,
}

/// Provider budget tracking token and cost consumption.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderBudget {
    /// Tokens consumed so far in the current billing window.
    pub tokens_used: u64,
    /// Maximum tokens allowed in the current billing window.
    pub token_limit: u64,
    /// Maximum monetary cost allowed in the current billing window.
    pub cost_limit: f64,
    /// Billing tier.
    pub tier: BudgetTier,
    /// When the current billing window resets.
    reset_at: Option<Timestamp>,
    /// Accumulated cost in the current billing window.
    cost: f64,
    /// Default cost per 1000 tokens, used to derive cost from token consumption.
    price_per_1k_tokens: f64,
}

impl ProviderBudget {
    /// Creates a new budget with the given limits and tier.
    ///
    /// The billing window has no reset time initially. Call [`set_window`]
    /// to establish a reset boundary.
    #[must_use]
    pub fn new(
        token_limit: u64,
        cost_limit: f64,
        tier: BudgetTier,
        price_per_1k_tokens: f64,
    ) -> Self {
        Self {
            tokens_used: 0,
            token_limit,
            cost_limit,
            tier,
            reset_at: None,
            cost: 0.0,
            price_per_1k_tokens,
        }
    }

    /// Creates a budget for the Free tier with default small limits.
    #[must_use]
    pub fn free(token_limit: u64, price_per_1k_tokens: f64) -> Self {
        Self::new(token_limit, 0.0, BudgetTier::Free, price_per_1k_tokens)
    }

    /// Sets the billing-window reset timestamp.
    pub fn set_window(&mut self, reset_at: Timestamp) {
        self.reset_at = Some(reset_at);
    }

    /// Current accumulated cost.
    #[must_use]
    pub fn cost(&self) -> f64 {
        self.cost
    }

    /// Estimated cost for a given number of tokens, based on the configured
    /// price per 1000 tokens.
    #[must_use]
    pub fn cost_for_tokens(&self, tokens: u64) -> f64 {
        (tokens as f64) / 1000.0 * self.price_per_1k_tokens
    }

    /// Returns the current budget status without consuming tokens.
    #[must_use]
    pub fn check_budget(&self) -> BudgetStatus {
        let remaining = self.token_limit.saturating_sub(self.tokens_used);
        let percent_used = if self.token_limit > 0 {
            (self.tokens_used as f64 / self.token_limit as f64) * 100.0
        } else {
            0.0
        };
        let within_limit = self.tokens_used <= self.token_limit && self.cost <= self.cost_limit;

        BudgetStatus {
            within_limit,
            percent_used,
            remaining,
            reset_at: self.reset_at,
        }
    }

    /// Attempts to consume `count` tokens from the budget.
    ///
    /// Returns `Ok(())` if the tokens (and their derived cost) fit within
    /// the remaining limits. Returns [`BudgetError`] otherwise.
    ///
    /// # Errors
    ///
    /// - [`BudgetError::TokenLimitExceeded`] if `count` would push `tokens_used`
    ///   past `token_limit`.
    /// - [`BudgetError::CostLimitExceeded`] if the derived cost would push the
    ///   accumulated cost past `cost_limit`.
    pub fn consume_tokens(&mut self, count: u64) -> Result<(), BudgetError> {
        let new_used = self.tokens_used.saturating_add(count);
        if new_used > self.token_limit {
            return Err(BudgetError::TokenLimitExceeded {
                requested: count,
                used: self.tokens_used,
                limit: self.token_limit,
            });
        }

        let token_cost = self.cost_for_tokens(count);
        let new_cost = self.cost + token_cost;
        if new_cost > self.cost_limit {
            return Err(BudgetError::CostLimitExceeded {
                requested_cost: token_cost,
                cost: self.cost,
                limit: self.cost_limit,
            });
        }

        self.tokens_used = new_used;
        self.cost = new_cost;
        Ok(())
    }

    /// Resets token usage, cost, and window to initial values.
    pub fn reset_usage(&mut self) {
        self.tokens_used = 0;
        self.cost = 0.0;
        self.reset_at = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_budget() -> ProviderBudget {
        ProviderBudget::new(1000, 10.0, BudgetTier::PayAsYouGo, 0.01)
    }

    #[test]
    fn within_budget() {
        // PROV-013-T01: a budget with usage under the limit reports within_limit
        let mut budget = sample_budget();
        budget.consume_tokens(500).expect("should consume");

        let status = budget.check_budget();
        assert!(status.within_limit);
        assert_eq!(status.remaining, 500);
        assert!((status.percent_used - 50.0).abs() < 0.01);
    }

    #[test]
    fn over_budget() {
        // PROV-013-T02: exceeding the limit makes check_budget report outside limits
        let mut budget = sample_budget();
        budget.consume_tokens(800).expect("should consume");

        let status = budget.check_budget();
        assert!(status.within_limit);
        assert_eq!(status.remaining, 200);

        // Consume the remaining to exactly hit the limit
        budget.consume_tokens(200).expect("should consume");
        let status = budget.check_budget();
        assert!(status.within_limit);
        assert_eq!(status.remaining, 0);

        // Now try to consume any more - should fail
        let err = budget.consume_tokens(1).expect_err("should fail");
        assert!(matches!(
            err,
            BudgetError::TokenLimitExceeded { .. }
        ));

        // After failed consume, check_budget still reports within limit
        // since state didn't change. Test over-limit condition directly.
        budget.tokens_used = 1001;
        let status = budget.check_budget();
        assert!(!status.within_limit);
        assert_eq!(status.remaining, 0);
    }

    #[test]
    fn consume_tokens() {
        // PROV-013-T03: consume_tokens increments usage or errors on overflow
        let mut budget = sample_budget();

        // Valid consumption
        budget.consume_tokens(300).expect("should succeed");
        assert_eq!(budget.tokens_used, 300);

        // Partial consumption still valid
        budget.consume_tokens(400).expect("should succeed");
        assert_eq!(budget.tokens_used, 700);

        // Over-limit consumption fails
        let err = budget.consume_tokens(500).expect_err("should fail");
        assert!(matches!(
            err,
            BudgetError::TokenLimitExceeded {
                requested: 500,
                used: 700,
                limit: 1000,
            }
        ));

        // Usage unchanged after the failed attempt
        assert_eq!(budget.tokens_used, 700);
    }

    #[test]
    fn reset_usage() {
        // PROV-013-T04: reset_usage zeroes tokens, cost, and window
        let mut budget = sample_budget();
        budget.set_window(Timestamp::now());
        budget.consume_tokens(800).expect("should consume");

        assert_ne!(budget.tokens_used, 0);
        assert!(budget.reset_at.is_some());
        assert!(budget.cost() > 0.0);

        budget.reset_usage();

        assert_eq!(budget.tokens_used, 0);
        assert!(budget.reset_at.is_none());
        assert_eq!(budget.cost(), 0.0);

        let status = budget.check_budget();
        assert!(status.within_limit);
        assert_eq!(status.remaining, 1000);
        assert!((status.percent_used - 0.0).abs() < 0.01);
    }

    #[test]
    fn percent_calculated() {
        // PROV-013-T05: percent_used reflects exact fraction of consumed tokens
        let mut budget = sample_budget();

        budget.consume_tokens(250).expect("should succeed");
        let status = budget.check_budget();
        assert!((status.percent_used - 25.0).abs() < 0.001);

        budget.consume_tokens(250).expect("should succeed");
        let status = budget.check_budget();
        assert!((status.percent_used - 50.0).abs() < 0.001);

        budget.consume_tokens(500).expect("should succeed");
        let status = budget.check_budget();
        assert!((status.percent_used - 100.0).abs() < 0.001);

        // Edge: zero-limit budget reports 0 percent
        let mut zero_budget = ProviderBudget::new(0, 0.0, BudgetTier::Free, 0.01);
        let status = zero_budget.check_budget();
        assert!((status.percent_used - 0.0).abs() < 0.001);
    }
}