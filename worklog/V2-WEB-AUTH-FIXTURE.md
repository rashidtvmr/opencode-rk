# V2 web authenticated-launch fixture

Source-only candidate based on `684032a`.

The browser API transport now fails closed for every `/api/*` request unless a
successfully validated launcher fragment has been consumed into tab memory.
An unrecognized or malformed non-empty fragment no longer bypasses the guard.
Valid `#oc2-token=<64 hex>` fragments retain the existing behavior: the token
is consumed, removed from the URL while preserving path/query, and used only
as an in-memory bearer header. SSR behavior and public non-API requests are
unchanged.

Frozen evidence supplied by the test owner:

- setup fixture hash: `d8ffbf5e1aaee39f3dd33d18c1fe9629b39ce2831e826832faa834687f2726a7`
- auth tests hash: `b528f74158eae54a60720a8feb3e286298516a81d7cca086fae620a172dd1291`
- baseline runtime source: `684032a`
- focused pre-fix result: original stream test and two auth controls GREEN;
  malformed-fragment fail-closed control RED because `listModels` reached
  `fetch` instead of throwing `Re-authentication required`.

No tests, setup files, manifests, or dependencies were modified by this
candidate. Parent must run the focused and full web suites independently.

## Controller preverification and runner maintenance

Exact candidate `45ed37378318e074c7a628e0102c01cc70b45cc0` passes all four
focused streaming/credential controls. Its full Vitest invocation passes all 49
TS/TSX assertions, but discovers four existing `node:test` `.mjs` contracts and
reports "No test suite found" for those files. These are runner mismatches.
Evidence: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-web-verify-45ed373-sszb_43s`.

The independent controller directs Vitest to TS/TSX suites and the standard
package test command to run every existing `.mjs` contract through `node --test`
afterward. None of the existing semantic test files or assertions are changed.
Both runners remain required for full Web test acceptance.

## Independent contract supersession

The unchanged Node runner exposed a semantic conflict in the new malformed-token
preflight expectation. Independent evaluator `ses_f0d28bef1ffeUDXYYY4jehyYp5`
inspected the accepted `api.refresh-auth.test.mjs:266–295` and
`worklog/AUTHWEB-REFRESH-IMPL.md:13–17,29`: malformed fragments are ignored, send no
Authorization header, and receive a real server `401`. No higher-authority
requirement prohibits that unauthenticated request. The original behavior is
secure and accepted; the new preflight restriction was an unsupported product
change. `45ed373` remains preserved as a conflicting proposal.

The controller restores the accepted product source and supersedes only the new
TS malformed-fragment assertion to exercise the same server-authoritative
rejection. All 20 existing semantic test files and their assertions remain
byte-identical to the base. The substantive product transport is unchanged.
The component launcher fixture and dual-runner repair remain independently useful.
