# V2 web authenticated-launch fixture

Mechanical fixture package based on `710a410`, with fixture freeze `684032a`, carried through
the restored product source at `de6b81d8a8bd3d2e141e939fbe8f4beb76bd6ce9`.

The substantive browser API transport is byte-identical to the accepted base:
valid `#oc2-token=<64 hex>` fragments are consumed into tab memory, malformed
fragments send no bearer and remain subject to the server's real `401`, and
public non-API requests remain unchanged. The earlier `45ed373` malformed
preflight proposal is superseded as a conflicting product change, not used as
the contract.

Frozen evidence supplied by the test owner:

- setup fixture hash: `d8ffbf5e1aaee39f3dd33d18c1fe9629b39ce2831e826832faa834687f2726a7`
- auth tests hash: updated after the mechanical response-fixture correction;
  the stub now uses the product reader's `message` field.
- baseline runtime source: `684032a`
- historical proposal result: original stream test and two auth controls GREEN;
  malformed-fragment fail-closed control RED because `listModels` reached
  `fetch` instead of throwing `Re-authentication required`.

The rejected product-only proposal changed the API guard. The final mechanical
package changes launcher fixture setup, adds credential controls and separates
the two existing test runners. Existing product source and semantic test files
are byte-identical to the accepted base.

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

## Independent preverification

Independent test-owner candidate `0076bfb32103c778786ba4e567807912c1e20bf9`
passed **49/49 Vitest assertions** in 17 suites and **11/11 Node assertions**.
Evidence:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-web-auth-mechanical-0076bfb`.
Log SHA-256: Vitest `02f11594259118561974630610ef1c17b53eb57e0d4ba3b273dd24934f1253ee`,
Node `a2c050b0d3382442630cafe5f1db913d52d3b96d724e7a78fa65431112f9c604`.
Current new credential-control SHA-256:
`b6340008577d599282692d65c0c573c3975f9055a589b7259f00d1a8ad00745e`.

The controller independently confirms the API source and all 20 pre-existing
test files are byte-identical to `710a410`. Candidate history `45ed373`, `de6b81d`
and `0076bfb` is integrated as `ae67e8c`, `8e07d74`, `68e59ae`; their final net
product-source change is zero. Exact integrated dual-runner verification remains
pending. Passing fixtures does not establish the real-browser G6 journey.
