# PROV-010: Implement Fallback Routing for Provider System

## Status
In Progress

## Objective
Implement provider fallback routing in `crates/providers/src/fallback.rs` with configurable policies.

## Implementation

### Files Modified
- `crates/providers/src/fallback.rs` - Full implementation

### Components

#### FallbackPolicy Enum
```rust
pub enum FallbackPolicy {
    Sequential,       // Strict order, no wrap
    RoundRobin,       // Cyclic rotation through providers
    ExponentialBackoff // Retry with delay per provider
}
```

#### FallbackHandler Struct
```rust
pub struct FallbackHandler {
    providers: Vec<Provider>,
    policy: FallbackPolicy,
    current_index: usize,
    attempt: u32,
    failures: HashMap<String, FailureRecord>,
}
```

#### Public API
- `next() -> Option<&Provider>` - Get next provider per policy
- `should_fallback(provider_id) -> bool` - Whether to skip/rotate
- `record_failure(id) -> u32` - Record failure, compute backoff
- `reset()` - Clear all state
- Secondary accessors: `providers()`, `policy()`, `current_index()`, `attempt()`, `len()`, `is_empty()`

#### Private Support
- `FailureRecord` struct tracks attempts, last_failure, backoff duration

### Test Plan
1. `sequential_fallback` - Verifies Sequential policy
2. `round_robin_cycle` - Verifies RoundRobin cycles correctly
3. `exponential_backoff` - Verifies backoff skips failed providers
4. `reset_clears` - Verifies reset() clears all state
5. `should_not_retry_same` - Verifies single-provider fallback edge case

## Verification Commands
```bash
cargo test -p opencode-rk-providers
cargo check --workspace
```

## Dependencies Used
- `opencode_rk_contracts::ProviderId`
- `crate::registry::Provider` re-exported
- `std::time::{Duration, Instant}`
- `std::collections::HashMap`

## Notes
- All tests use `sample_providers()` helper for consistency
- Exponential backoff: 50ms * 2^(attempts-1), capped at 5s
- Policy defaults to `Sequential`