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
