# AUTHWEB-BROWSER-CLIENT-VERIFY

## Claim / ownership
- Task ID: AUTHWEB-BROWSER-CLIENT-VERIFY
- Session: ses_f214372a2ffem88gxOOC3xHP39
- Verifier worktree: /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/verify-authweb-browser-client
- Verifier commit: ecec045471641eb50b8e59bed00003f1af943d4d ("AUTHWEB: attach in-memory fragment bearer to browser API calls")
- Owned file: worklog/AUTHWEB-BROWSER-CLIENT-VERIFY.md (this file only)
- No source edits; no frozen-test edits.

## Intake checklist (paraphrased from .agents/WORKER.md section 8)
1. Task identity: subagent spawned with @vyce-deepseek-v41 route. Verifier worktree at exact commit ecec045471641eb50b8e59bed00003f1af943d4d.
2. Owned scope: ONE scratchpad file in canonical repo; no source/test/controller/policy edits.
3. Source evidence:
   - web/src/lib/api.ts:171-184 bearerToken + consumeBrowserCredential (fragment token consumed, cleared via history.replaceState)
   - web/src/lib/api.ts:186-193 apiHeaders (adds Bearer only for /api/ paths)
   - web/src/lib/api.ts:195-211 request() -- fetch site 1
   - web/src/lib/api.ts:597-621 uploadDraftAttachment -- fetch site 2 (apiHeaders)
   - web/src/lib/api.ts:623-637 deleteDraftAttachment -- fetch site 3 (apiHeaders)
   - web/src/lib/api.ts:1020-1039 runTurnStream -- fetch site 4 (apiHeaders)
   - crates/cli/src/main.rs:668-747 serve() / web() bare origin: println!(descriptor.http_origin); credential minted at 742, published at 743; router_with_auth at 753
4. Observable contract: browser API client attaches in-memory bearer (from #oc2-token= fragment) ONLY to /api/ requests, strips fragment from URL, never injects token into query string, live endpoint returns 401 without credential.
5. Resource bounds: local HTTP on 127.0.0.1 ephemeral port; tests cleanup with finally block.
6. Security: no secret file access; token passed via mocked window.history.replaceState; no env inheritance beyond test fixtures.
7. Test plan: frozen tests run via node --experimental-strip-types (TS strip mode).
8. Convergence gate: ran before; baseline total=83, exit 0.

## Verification evidence
- Hashes (expected vs actual):
  - api.auth.test.mjs:     f007aa291204204d366506c2c8b5149099317bb103da6fe915f8e4409e5a6a62 == f007aa291204204d366506c2c8b5149099317bb103da6fe915f8e4409e5a6a62 (MATCH)
  - api.direct-auth.test.mjs: 4c7201da9246717ccea089ddcf4f7c0befe749403be985e9d3a630926e3a879c == 4c7201da9246717ccea089ddcf4f7c0befe749403be985e9d3a630926e3a879c (MATCH)
- Command: `rtk node --experimental-strip-types --test web/src/lib/api.auth.test.mjs web/src/lib/api.direct-auth.test.mjs`
- Result: 2/2 PASS (0 fail, 0 skip), duration 107.96ms

## Source analysis (no changes needed; verification only)
- api.ts:171-184: bearerToken module-private, consumed lazily from location.hash #oc2-token=[0-9a-f]{64}. Consumed once, stored in-memory only (not URL/query), fragment cleared via history.replaceState.
- api.ts:186-193: apiHeaders injects `authorization: Bearer <token>` ONLY for paths starting with '/api/'.
- 4 authenticated fetch sites confirmed via grep: request() L198, uploadDraftAttachment L604, deleteDraftAttachment L629, runTurnStream L1029 -- all via apiHeaders.
- main.rs:668-747: CLI web()/serve() prints bare http_origin (no token in URL) at 670/706; daemon credential minted (742) and published via publish_backend_descriptor_with_auth (743); router_with_auth wraps app state with Some(credential) (753). Origin is printed bare; credential attached server-side, consistent with "lost token on refresh" concern noted in task but outside browser-client verify scope.
- Canonical guard note: main.rs:668-747 is bare origin path; task notes "lost token on refresh, direct-test author's claim mismatch, canonical guard 51 errors" -- these are pre-existing findings recorded, not introduced by this verify lane.

## Decisions
- Verifier-only lane: confirm frozen tests pass against exact commit; record findings in scratchpad; commit+push scratchpad+claim only to SSH verify branch.
- No source edits, no test edits.

## Remaining unknowns / gaps (out of scope)
- main.rs bare origin behavior, token-on-refresh lifecycle, direct-test author claim mismatch, canonical guard 51 errors -- recorded as findings, flagged to orchestrator, not fixed by this lane.
- No cargo/typecheck run per task instructions.

## Status
- Verification PASS: 2/2 frozen tests green; hashes match expected exactly.
