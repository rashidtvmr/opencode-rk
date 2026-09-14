# PROV-001

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-007.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: PROV-001-T01, PROV-001-T02, PROV-001-T03, PROV-001-T04, PROV-001-T05.

## User-observable outcome

LLM provider registry for routing requests. Provides Provider struct with id, name, base_url, api_key_env, priority, timeout_secs. ProviderRegistry manages providers by id and by priority order. ProviderConfig supplies default provider configurations.

## Source evidence

- crane/src/main.rs:87-94 provider selection logic; 152-167 request dispatch.
- crates/providers/src/router.rs:42-68 fallback chain resolution.

## Observable contract

- Happy path, failure states, and resource bounds per test obligations PROV-001-T01, PROV-001-T02.
- No unbounded queue, no unbounded retained output; owner and cancel path defined.