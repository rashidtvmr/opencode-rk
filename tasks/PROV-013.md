# PROV-013

Status: DONE. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-038.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: PROV-013-T01, PROV-013-T02, PROV-013-T03, PROV-013-T04, PROV-013-T05.

## User-observable outcome

Provider budget tracking: token and cost consumption limits with tiered
billing, status inspection, and usage reset.

## Implementation

- BudgetTier enum: Free, PayAsYouGo, Enterprise
- ProviderBudget: tokens_used: u64, token_limit: u64, cost_limit: f64, tier: BudgetTier
- check_budget() -> BudgetStatus
- consume_tokens(count) -> Result<(), BudgetError>
- reset_usage()
- BudgetStatus: within_limit: bool, percent_used: f64, remaining: u64, reset_at: Option<Timestamp>
- BudgetError: TokenLimitExceeded, CostLimitExceeded

## Tests

- within_budget (PROV-013-T01): budget under limit reports within_limit=true
- over_budget (PROV-013-T02): exceeding limit triggers TokenLimitExceeded error, check_budget reports outside limit
- consume_tokens (PROV-013-T03): valid consumption increments tokens; over-limit fails without state change
- reset_usage (PROV-013-T04): zeroes tokens_used, cost, and reset_at window
- percent_calculated (PROV-013-T05): percent_used reflects exact fraction of consumed tokens

## Verify

cargo test -p opencode-rk-providers budget::tests (5 passed)
cargo check --workspace (clean, no errors)

## Notes

- Timestamp type sourced from opencode_rk_contracts::Timestamp
- Price per 1k tokens drives cost derivation; cost_limit=0.0 for Free tier
- tap.rs tests fail due to pre-existing uncommitted changes outside this task scope
