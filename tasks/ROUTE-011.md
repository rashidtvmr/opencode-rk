# ROUTE-011 - Bounded account fallback execution coordinator

Status: IMPLEMENTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-022.
Dependencies: ROUTE-001 account eligibility, ROUTE-002 account selection, ROUTE-003 failure lock planning, ROUTE-004 success cleanup.
Test obligations: ROUTE-011-T01, ROUTE-011-T02, ROUTE-011-T03, ROUTE-011-T04, ROUTE-011-T05.

## User-observable outcome

Multi-account routing can cycle to the next eligible account after a fallback-eligible provider failure, remember only bounded failure metadata for final exhaustion, and stop immediately for success, caller cancellation, or a non-fallback failure. The coordinator is pure routing state: the caller still owns provider execution, cancellation, lock persistence, token refresh, and endpoint-specific HTTP responses.

## Source evidence and decomposition

- Pinned 9router commit `17c4cc76877bd1755030a8414f8d0083f48dcccf`, `src/sse/handlers/chat.js:228-338`: the account loop tracks an exclusion set plus last error/status, retries only when `shouldFallback` is true, returns success/non-fallback directly, distinguishes initial no-credentials from post-attempt exhaustion, and carries rate-limit retry state from account eligibility.
- The same loop shape is repeated at pinned `src/sse/handlers/embeddings.js:92-164`, `stt.js:55-87`, and `fetch.js:157-231`, which differ in endpoint response mapping but share exclusion/retry/exhaustion semantics.
- Pinned `src/app/api/v1beta/models/[...path]/route.js:246-364` plus `tests/unit/gemini-native-endpoint.test.js:151-236`: a fallback-eligible timeout excludes the first credential and succeeds on the next; a non-fallback network failure returns directly; caller cancellation does not mark/exclude the account.
- Pinned `tests/unit/embeddings.cloud.test.js:432-468`: a non-fallback error propagates without cycling; a fallback-eligible 429 cycles until account exhaustion.
- `REQ-022` assigns built-in multi-account routing to ROUTE-001/002/005/011. ROUTE-001 owns eligibility/exhaustion inputs, ROUTE-002 owns one-account selection, ROUTE-005 owns model-name resolution, and their committed task/worklogs explicitly reserve fallback/execution semantics for ROUTE-011.
- `sources/behavior-surface-rules.json` nominates ROUTE-011 only for reviewed `9router.routing`, matching this pure coordinator boundary. No task semantics are inferred from the numeric id.

## Observable contract

- Caller supplies each attempted account id and the attempt outcome. A fallback-eligible failure records that account as excluded, retains bounded last-failure status/text, and returns a retry decision; exclusions preserve deterministic insertion order.
- A non-fallback failure returns that failure immediately and does not mutate the exclusion/last-fallback state.
- Success and caller cancellation are terminal decisions and do not add an exclusion. Cancellation must never be converted into account fallback by this coordinator.
- When eligibility reports ordinary unavailability before any fallback attempt, finalization yields typed `NoCredentials`; after one or more fallback attempts it yields typed `Exhausted` carrying the last bounded failure.
- When eligibility reports rate-limited exhaustion, finalization preserves the caller-provided retry timestamp and the optional last fallback failure rather than converting it to ordinary exhaustion.
- Duplicate attempted-account fallback, invalid/oversized account ids, and attempt-count overflow are typed and non-mutating. The attempt count reuses the existing bounded account-selection maximum.
- Retained failure text is explicitly bounded. Truncation affects only coordinator exhaustion metadata; direct non-fallback failures remain caller-owned and are not rewritten by the routing state machine.

## Ownership, lifetime, persistence, and safety

- Product boundary: additive pure API in `crates/providers/src/router.rs` only; no new dependency or runtime service.
- Caller owns coordinator lifetime, provider calls, token refresh, `should_fallback` classification, ROUTE-003 persistence patch application, ROUTE-004 success cleanup, client cancellation signal, and endpoint-specific response construction.
- No DB/storage, secrets, network, environment access, wall clock, process/task spawning, or background work.
- The coordinator retains at most `MAX_ACCOUNT_SELECTION_CANDIDATES` account ids plus one bounded failure record; no unbounded queue/output history is introduced.
