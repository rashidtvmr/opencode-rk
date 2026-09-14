# ROUTE-001

Status: IMPLEMENTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-022.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: ROUTE-001-T01, ROUTE-001-T02, ROUTE-001-T03, ROUTE-001-T04, ROUTE-001-T05.

## User-observable outcome

Multi-account routing filters inactive, explicitly excluded, and model-locked accounts before strategy selection; eligible accounts are deterministic by priority, and exhaustion distinguishes ordinary unavailability from rate-limited retry timing.

## Source evidence

- Pinned 9router commit `17c4cc76877bd1755030a8414f8d0083f48dcccf`, `src/sse/services/auth.js:72-140`: load active provider connections; filter caller exclusions and model locks; return null when unavailable or typed all-rate-limited state with earliest retry; strategy selection consumes the resulting eligible list.
- `sources/disc-003-reconciliation.json` reviewed `9router.account-storage` and `9router.routing`: provider connections/model health are persisted reference state; routing confirms multi-account selection and fallback state.
- `sources/disc-003-evidence.json` pins `NR-GITHUB-LOCK-TEST` for account/model lock persistence; this slice consumes lock state but does not duplicate storage ownership.

## Observable contract

- Active, unlocked, non-excluded accounts are returned in stable priority/id order.
- Inactive, excluded, and currently model-locked accounts are not eligible.
- Expired model locks no longer block the account.
- When every otherwise-active account is currently locked, return the earliest retry timestamp as typed rate-limited exhaustion.
- When no active candidates exist for non-lock reasons, return typed unavailable exhaustion. Selection is pure and bounded by the caller-provided candidate slice.
