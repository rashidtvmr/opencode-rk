# AUTHWEB-REFRESH-IMPL

## Claim
- Task: AUTHWEB-REFRESH-IMPL
- Session: ses_f1ea35c6fffemxzSnOGZHiYSd8
- Owned product path: `web/src/lib/api.ts`
- Frozen refresh tests: `web/src/lib/api.refresh-auth.test.mjs`, SHA `c7e9e7fdcd05371b261e01778060b44ff9a13e2a0ae481d85ce3778fcb295f5c`
- RED baseline commit: `255958c`

## Source evidence and contract
- Current `api.ts:161-211` defines `ApiError`, consumes a valid `#oc2-token=<64 hex>` fragment into module memory, clears it with `history.replaceState`, and attaches it only to `/api/` requests.
- Current `api.ts:195-211` otherwise sends browser `/api/*` calls without a bearer when a fresh module has no fragment; this causes an unintended unauthenticated network request after reload.
- Frozen refresh tests require same-tab in-memory reuse, no persistence/query/console leakage, local clear re-auth failure on fresh browser module with no fragment before `/api/*` fetch, malformed fragments to remain ignored and reach real 401, and non-API/non-browser compatibility.

## Observable boundary
- Browser `/api/*` requests with no valid consumed credential and no fragment fail locally with `ApiError` status 401 and a re-authentication message, before `fetch`.
- A malformed token fragment remains ignored and therefore retains the existing unauthenticated request behavior.
- Valid credentials remain module-memory only; no storage, URL, query, console, or remote relaunch endpoint is introduced.
- Non-browser callers and non-`/api/` paths preserve existing behavior.

## Resource/security
- No retained secret beyond existing module-scoped token lifetime; no persistence or new network calls.
- No test files modified.

## Tests
- Pending focused frozen refresh/auth/direct runs.

## GREEN evidence (2026-09-26)
- Implemented browser-only local `ApiError(401, ...)` guard in `apiHeaders` for `/api/*` when no valid fragment credential exists; malformed fragments remain unauthenticated and are not locally intercepted.
- `node --test --experimental-strip-types web/src/lib/api.refresh-auth.test.mjs`: 4 passed.
- `node --test --experimental-strip-types web/src/lib/api.auth.test.mjs web/src/lib/api.direct-auth.test.mjs`: 2 passed.
- `pnpm --dir web typecheck`: product typecheck reached pre-existing unused imports/locals in `web/src/lib/canvas-model.test.ts`; no errors reported for `api.ts`.
- Convergence gate was run and is blocked by pre-existing off-plan ledger entries; not modified.
