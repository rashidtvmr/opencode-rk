# AUTHWEB-DIRECT-FETCH-RED

- Claim: `AUTHWEB-DIRECT-FETCH`, session `ses_f215e9103ffeQRrS4n5lI5Ceyr`; claimed from `not-started`.
- Branch: `red/AUTHWEB-DIRECT-FETCH` (worktree at `/private/var/.../red-authweb-direct-fetch`), base commit `6de682f7`.
- Source evidence: `web/src/lib/api.ts:599-612`, `deleteDraftAttachment` calls `fetch()` directly with NO Authorization header; `web/src/lib/api.ts:171-188`, `request()` also lacks bearer injection; `crates/server/src/daemon_auth.rs:150-175`, `require_bearer` middleware returns 401 for `/api/*` without bearer token. This is current client behavior lacking authorization + current server requiring bearer = new required browser-client behavior for the direct-fetch path.
- Classification: `deleteDraftAttachment` uses direct `fetch` (not `request()`), bypassing any future auth injection in `request`. Gap confirmed at api.ts:604-606: `fetch(path, { method: 'DELETE', signal })` with no headers.authorization.
- Observable contract: real Node HTTP server binds only `127.0.0.1` on ephemeral port. `deleteDraftAttachment('s1', 'a1')` with fake 64-hex token in `#oc2-token=...` succeeds only when its real request carries `Authorization: Bearer ...`; token must not appear in URL/query; fragment cleared via `history.replaceState`. Global fetch wrapper resolves relative path ONLY, no synthesized response, no credential injection. Separate live no-token call returns 401.
- Resource/security bounds: one local HTTP server, one request at a time, ephemeral port, no external network, no real credentials/DB, server closed in `finally`. Fake token is test-only.
- Test: `web/src/lib/api.direct-auth.test.mjs`; Node `v24.21.0`, command `node --experimental-strip-types --test web/src/lib/api.direct-auth.test.mjs`.
- Status: RED-only candidate; not acceptance.

## Run result (RED)

Command: `node --experimental-strip-types --test web/src/lib/api.direct-auth.test.mjs`

Result: 1 fail, 0 pass (exit 1).

Failure trace: `ApiError: Request failed with 401` at `deleteDraftAttachment (web/src/lib/api.ts:610:11)`.

The test compiles and executes against a real local HTTP fixture on 127.0.0.1:0. It fails because the server endpoint rejects the request with HTTP 401 -- the client `deleteDraftAttachment` at api.ts:599-612 calls `fetch()` directly with no Authorization header. The global fetch wrapper only resolves the relative path to the local server; it does not inject credentials. This is a genuine 401 from the real fixture, not a setup/loader error.

Frozen test SHA-256: `4c7201da9246717ccea089ddcf4f7c0befe749403be985e9d3a630926e3a879c` (`web/src/lib/api.direct-auth.test.mjs`). No test edits after this freeze.
