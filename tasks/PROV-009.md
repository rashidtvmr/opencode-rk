# PROV-009

Status: IN PROGRESS. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-039.
Dependencies: PROV-001 (Provider struct defined in registry.rs).
Test obligations: PROV-009-T01 through PROV-009-T05.

## User-observable outcome

Provider router with weighted round-robin (WRR) selection over a dynamic
provider list. Supports add/remove at runtime and smooth WRR distribution
proportional to per-provider weights.

## Source evidence

- crates/providers/src/router.rs (existing stub module, only a doc comment)
- crates/providers/src/registry.rs:6-41 (Provider struct definition)
- crates/providers/Cargo.toml (no new dependencies required)
- crates/contracts/src/lib.rs:65-76 (ProviderId type for reference)

## Observable contract

### ProviderRouter

- `providers: Vec<Provider>` - registered providers, parallel to weights
- `weights: Vec<f32>` - static weight per provider, parallel to providers
- `current_index: usize` - index of last selected provider (round-robin cursor)

### WeightedRoundRobin trait

- `route() -> Option<usize>` - returns next provider index, None if empty
- `next_provider() -> Option<&Provider>` - returns next provider, None if empty

### ProviderRouter methods

- `new() -> Self` - empty router, current_index = 0
- `count() -> usize` - number of registered providers
- `get(index) -> Option<&Provider>` - reference to provider at index
- `add_provider(provider, weight)` - append provider with weight
- `remove_provider(id) -> bool` - remove by id, adjust current_index

### WRR algorithm

- Nginx-style smooth weighted round-robin
- Current weight accumulates: current_weight[i] += weight[i]
- Select provider with highest current_weight
- Subtract total_weight from selected provider's current_weight
- Equal weights produce plain round-robin cycle
- Zero/negative total weight falls back to plain round-robin

## Failure states

- Empty router: route() and next_provider() return None
- Remove nonexistent id: returns false, state unchanged
- Remove last provider: current_index reset to 0

## Test obligations

- PROV-009-T01 next_returns_some - single provider returns Some
- PROV-009-T02 round_robin_cycles - equal weights cycle through all providers
- PROV-009-T03 weighted_distribution - 5:1 weight produces 10:2 over 12 iterations
- PROV-009-T04 remove_existing - removing by id removes correct provider
- PROV-009-T05 route_empty - empty router returns None

## Verification

```bash
cargo test -p opencode-rk-providers && cargo check --workspace
```
