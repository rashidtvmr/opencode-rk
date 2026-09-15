# PROV-021

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-041.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: PROV-021-T01, PROV-021-T02, PROV-021-T03, PROV-021-T04, PROV-021-T05.
Ownership locks: crates/providers/src/usage_status.rs only; worker never edits shared lib.rs, Cargo.toml, schemas, migrations.
Suggested module: crates/providers/src/usage_status.rs (new module in crate opencode-rk-providers; lib.rs wiring left to integrator).

## User-observable outcome

Provider usage and limits telemetry: per model, provider, and auth source shows request count, input/output tokens, cost when available, and rate-limit reset metadata as bounded snapshots. Secrets never shown.

## Source evidence

- crates/providers/src/config.rs:7-22 ProviderConfig (provider_id, base_url, timeout_secs, max_tokens).
- crates/providers/src/registry.rs for provider/model routing (Provider, ProviderRegistry).
- crates/providers/src/model_route.rs for provider/model routing (ModelRoute, MAX_COMBO_MODELS).
- crates/providers/src/auth.rs:6-18 AuthMethod; :40-101 AuthHandler and in-memory refresh.
- crates/providers/src/lib.rs:4-48 provider modules (no usage_status yet).
- crates/providers/src/integration.rs:1-5 explicitly stops before credential storage/external OAuth; :11-18 bounded constants.
- docs/SECURITY.md:14-32 brokered permissions, no direct secret access/unrestricted env.
- docs/research/REQ-041-provider-compatibility-research.md:1 REQ-041 official provider compatibility and CLI authentication scope.

## Observable contract

- `UsageKey { provider_id, model, auth_source }`: explicit identity triple; empty segments rejected.
- `UsageCounters { requests: u64, input_tokens: u64, output_tokens: u64, cost_micros: Option<u64>, rate_limit_reset_ms: Option<u64> }`: saturating arithmetic; cost present only when provider reports it.
- `record(store, key, delta) -> Result<(), UsageError>`: adds bounded delta; rejects unknown provider and over-cap deltas.
- `snapshot(store) -> UsageSnapshot { entries: Vec<UsageEntry>, truncated: bool }`: deterministic provider/model order; capped entries.
- `reset(store, key)`: clears one triple; unknown key is harmless no-op.
- Caps: MAX_USAGE_ENTRIES 256, MAX_DELTA_TOKENS 10_000_000 per record call; counters saturate at u64::MAX instead of wrapping.
- Deterministic: same record sequence yields byte-identical snapshot; no I/O, no network, no clock reads (caller supplies timestamps).
- Suggested module boundary: crates/providers/src/usage_status.rs owning UsageKey, UsageCounters, UsageSnapshot, UsageError, record, snapshot, reset; shared lib.rs wiring left to integrator.

## Failure states

- Empty provider, model, or auth source: `Err(UsageError::EmptyField)`; store unchanged.
- Store full at 256 entries plus new key: `Err(UsageError::Overflow)`; store unchanged.
- Over-cap delta: `Err(UsageError::DeltaTooLarge)`; store unchanged.
- Counter saturation: saturates at u64::MAX, never wraps, never panics.
- Secret safety: snapshots and errors contain counts and metadata only, zero credential bytes. No DB writes; tests use disposable in-memory stores only.

## Resource bounds

- Pure library: no Command, no thread, no I/O, no network; caller owns store lifetime.
- Bounded entries and deltas; snapshot allocation capped; no unbounded retained output.
- No unbounded queue; owner and cancel path defined per crates/providers/src/integration.rs:11-18 bounded-constants precedent.

## Acceptance criteria

- PROV-021-T01: record happy path: record 3 requests with input/output tokens plus cost yields snapshot entry with exact counts and cost; second record accumulates.
- PROV-021-T02: snapshot order and reset: two keys snapshot in provider/model order; reset clears one key and leaves the other intact.
- PROV-021-T03: overflow and delta caps: 257th distinct key yields Overflow unchanged; over-cap delta yields DeltaTooLarge unchanged; u64::MAX saturates without wrap.
- PROV-021-T04: validation: empty provider/model/auth-source each yield EmptyField; unknown-key reset is harmless Ok.
- PROV-021-T05: determinism and isolation: same sequence byte-identical snapshot; Debug plus serialize contain no secret substrings; no files or DB writes outside disposable test scope.

## Test-first execution

- RED: author tests PROV-021-T01..T05 against crates/providers/src/usage_status.rs; establish compiling RED (fail: no usage_status module).
- GREEN: implement minimum native Rust counters with saturating math and bounded snapshot; GREEN, refactor, rerun; negative tests (empty, overflow, delta-cap, saturation).
- Evidence: `cargo test -p opencode-rk-providers usage_status`; `cargo check --workspace`; frozen test hash plus command manifest; verifier decides acceptance.
