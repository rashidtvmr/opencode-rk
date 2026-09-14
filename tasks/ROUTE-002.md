# ROUTE-002 - Account selection strategy

Status: IMPLEMENTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-022.
Dependencies: ROUTE-001 account eligibility.
Test obligations: ROUTE-002-T01, ROUTE-002-T02, ROUTE-002-T03, ROUTE-002-T04, ROUTE-002-T05.

## User-observable outcome

After ROUTE-001 has produced the eligible provider accounts, routing chooses one account deterministically: an available preferred account wins, fill-first uses the already priority-ordered first account, and round-robin applies the pinned sticky/least-recently-used behavior without consulting wall clock or persistence inside the selector.

## Source evidence and decomposition

- Pinned 9router commit `17c4cc76877bd1755030a8414f8d0083f48dcccf`, `src/sse/services/auth.js:136-193`: provider/global strategy selection, preferred-connection pinning, sticky round-robin, least-recently-used switch, and fill-first fallback.
- Pinned `src/lib/db/repos/connectionsRepo.js:92-102`: active provider connections are returned in priority order before the strategy phase.
- Pinned `src/lib/db/repos/settingsRepo.js:7-19`: sticky round-robin default is 3 and provider strategy overrides are configuration inputs.
- `REQ-022` maps the mandatory built-in multi-account routing requirement to ROUTE-001/002/005/011. ROUTE-001 now owns eligibility and ROUTE-005 owns model-name resolution; this card assigns ROUTE-002 the immediately following account-selection phase from the same pinned `auth.js` call path. This is an explicit source-audit decomposition decision, not semantics inferred from the numeric task id.
- `sources/behavior-surface-rules.json` nominates ROUTE-002 only for `9router.routing`, matching this pure strategy boundary. ROUTE-011 remains available for later fallback/execution behavior once its source ownership is reconciled.

## Observable contract

- If `preferred_connection_id` names an eligible candidate, select it and skip strategy state mutation; if absent/unavailable, continue with the configured strategy.
- Fill-first selects the first caller-provided eligible candidate. ROUTE-001 is responsible for priority/id ordering before this phase.
- Round-robin stays on the most recently used candidate while its consecutive-use count remains below the sticky limit, returning a caller-owned persistence patch with `last_used_at = now` and incremented count.
- Once the sticky limit is reached, round-robin chooses the least recently used candidate (unused candidates first, preserving deterministic caller order for ties) and returns a patch resetting consecutive use to 1 at caller-supplied `now`.
- Empty candidates, invalid strategy parameters, and candidate overflow return typed errors.

## Ownership, persistence, and bounds

- Pure selection only. Caller owns eligible candidate state, strategy configuration, `now`, and application of any returned persistence patch.
- No DB/network/environment/wall-clock/background work in the selector.
- Candidate input is capped by a public frozen maximum; scan/sort work is bounded by that slice.
