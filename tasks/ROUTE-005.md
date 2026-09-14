# ROUTE-005

Status: IMPLEMENTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-022.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: ROUTE-005-T01, ROUTE-005-T02, ROUTE-005-T03, ROUTE-005-T04, ROUTE-005-T05.

## User-observable outcome

Model routing recognizes explicit provider/model targets, named model aliases, and named model combos; combo names take precedence over aliases so a multi-model route cannot be accidentally collapsed to a single provider target.

## Source evidence

- Pinned 9router commit `17c4cc76877bd1755030a8414f8d0083f48dcccf`, `src/sse/services/model.js:27-94`: resolve aliases, detect combo names before alias resolution, and expose combo model lists; explicit provider/model strings bypass combo lookup.
- `sources/disc-003-reconciliation.json` reviewed `9router.model-combos`: model service resolves aliases and combo names and is consumed by chat routing.
- The review explicitly records missing direct alias/combo tests; this target slice adds native behavioral coverage rather than claiming broader combo execution parity.

## Observable contract

- `provider/model` resolves directly to that provider and model.
- A known combo name resolves to a combo target and its bounded member list.
- Combo lookup wins over an alias with the same name.
- A known alias resolves to one explicit provider/model target.
- Unknown or malformed model names return a typed unresolved/invalid result without guessing a provider.
