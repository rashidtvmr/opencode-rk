# ROUTE-007 - Compatible provider prefix precedence

Status: IMPLEMENTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: none (DISC-002 discovered routing surface).
Dependencies: ROUTE-005 model-route naming boundary.
Test obligations: ROUTE-007-T01, ROUTE-007-T02, ROUTE-007-T03, ROUTE-007-T04, ROUTE-007-T05.

## User-observable outcome

Explicit provider/model routing honors built-in provider ids and aliases before user-defined compatible-provider prefixes, while still allowing a non-reserved compatible prefix to route to its configured provider-node id.

## Source evidence and ownership

- Pinned 9router commit `17c4cc76877bd1755030a8414f8d0083f48dcccf`, `src/sse/services/model.js:12-17,41-66`: registry/local provider ids and aliases form the reserved prefix set; only non-reserved provider prefixes may match compatible provider nodes.
- Pinned `tests/unit/model-routing.test.js:41-59`: built-in `cf`/Cloudflare routing wins over a colliding compatible-node prefix.
- Pinned `tests/unit/model-routing.test.js:61-79`: non-reserved `oct` resolves to the compatible node id while preserving the model remainder.
- `sources/behavior-surface-rules.json` `9router.model-combos` nominates only `CAT-005`, `ROUTE-005`, and `ROUTE-007` for `src/sse/services/model.js`. CAT-005 owns catalog search; ROUTE-005 already owns explicit/alias/combo-name resolution and explicitly leaves provider-node prefix compatibility for a later slice. This makes ROUTE-007 the strongest source-grounded remaining owner for prefix precedence.
- `open-sse/services/combo.js` is jointly nominated to ROUTE-006/ROUTE-007 through `9router.translation-proxy`; combo rotation/fallback/stream-commit ownership is therefore not claimed by this card until DISC-003 resolves that shared surface.

## Observable contract

- A built-in/reserved provider id or alias is never shadowed by a compatible node with the same prefix.
- A non-reserved compatible prefix resolves to the matching compatible node id and preserves the model remainder.
- A canonical explicit provider/model target remains canonical when no compatible override is allowed.
- Malformed or unresolved provider/model input returns a typed result/error rather than consulting storage/network or guessing.
- Caller-supplied compatible-node candidates are explicitly bounded; overflow is typed and deterministic.

## Ownership, lifetime, and resource bounds

- Pure provider-model routing decision only; caller supplies the reserved-alias map and compatible-node slice.
- No DB, network, environment, clock, persistence, queue, or background task.
- Compatible-node scan is bounded by a public small maximum frozen by the RED suite.
- Combo rotation/fallback execution remains unassigned between ROUTE-006/ROUTE-007 and is not part of this candidate claim.
