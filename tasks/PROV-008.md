# PROV-008

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-038.
Dependencies: none.
Test obligations: PROV-008-T01 through PROV-008-T05.

## User-observable outcome

Provider metrics system for tracking calls, errors, and latency across providers.

## Source evidence

- crates/providers/src/metrics.rs (existing stub module)
- crates/providers/Cargo.toml (dependencies)
- crates/contracts/src/lib.rs (ProviderId type)

## Observable contract

### ProviderMetrics

- `calls_total: u64` - total number of provider calls
- `errors_total: u64` - total number of failed calls
- `latency_ms: Histogram` - latency distribution in milliseconds
- `active_requests: u64` - current concurrent requests

### ProviderMetricsRecorder

- `per_provider: HashMap<ProviderId, ProviderMetrics>`
- `record_call(provider_id, latency_ms, success)` - record a call
- `snapshot() -> Vec<(String, ProviderMetrics)>` - get all metrics
- `reset()` - clear all metrics

## Test obligations

- PROV-008-T01 records_call - records successful calls
- PROV-008-T02 error_incremented - errors increment on failure
- PROV-008-T03 latency_recorded - latency histogram records latency
- PROV-008-T04 snapshot_returns_all - snapshot returns all providers
- PROV-008-T05 reset_clears - reset clears all data

## Verification

```bash
cargo test -p opencode-rk-providers && cargo check --workspace
```