# PROV-011

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes. Requirements: REQ-XXX.
Dependencies: none. Test obligations: PROV-011-T01, PROV-011-T02, PROV-011-T03, PROV-011-T04, PROV-011-T05.

## User-observable outcome

Provider tap system for request-level access control. Provides TapRule enum with Allow, Deny, and RateLimit variants; ProviderTap struct that tracks per-provider tap rules and request counts; and TapDecision result with allowed flag, remaining limit, and reason.

## Source evidence

- crates/providers/src/health.rs:1-180 HealthMonitor pattern for tracking provider state with HashMap
- crates/providers/src/lib.rs includes tap module

## Observable contract

- TapRule enum with Allow, Deny(u64), RateLimit(u64) variants
- ProviderTap with rules HashMap<String, TapRule> and request_count HashMap<String, u64>
- tap(provider_id) -> TapDecision; add_rule(id, rule); remove_rule(id); record_request(id)
- TapDecision: allowed: bool, limit_remaining: Option<u64>, reason: Option<String>

## Test obligations

- PROV-011-T01: allow_rule - tap returns allowed when rule is Allow
- PROV-011-T02: deny_rule - tap returns !allowed when rule is Deny
- PROV-011-T03: rate_limit - tap enforces rate limit and returns remaining count
- PROV-011-T04: remove_rule - tap returns allowed after rule is removed
- PROV-011-T05: record_and_check - record_request increments counter correctly