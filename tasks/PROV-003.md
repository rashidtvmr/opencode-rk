# PROV-003

Status: IN PROGRESS. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-007.
Dependencies: none.
Test obligations: PROV-003-T01, PROV-003-T02, PROV-003-T03, PROV-003-T04, PROV-003-T05.

## User-observable outcome

Provider health monitoring system. Tracks per-provider health verdicts with success/failure accounting, a consecutive-failure threshold (backoff after 3 errors), and a staleness timeout.

## Source evidence

- crates/providers/src/health.rs: current implementation.

## Observable contract

- ProviderHealth struct: provider_id, status (enum Healthy/Degraded/Unhealthy), latency_ms, last_check (SystemTime)
- HealthMonitor struct: providers HashMap, timeout_secs, unhealthy_count HashMap
- Methods: check_health(provider_id) -> bool, record_success(provider_id, latency_ms), record_failure(provider_id)
- Methods: is_healthy(provider_id) -> bool, should_retry(provider_id) -> bool
- Backoff after 3 errors: should_retry returns false when status is Unhealthy
- Tests: healthy_provider, error_threshold_triggers, latency_recorded, unhealthy_list, backoff_after_errors

- No unbounded queue, no unbounded retained output; owner and cancel path defined.