# V2 Web authenticated-launch fixture maintenance

Base: `710a410e0ce32c716c40217696b515caa6c52310`.

The accepted browser transport requires the launcher credential before `/api/*`
requests. Legacy component tests launched a fresh jsdom page without it. On the
unchanged base, the focused streaming suite fails before reaching its streaming
assertions: `Unable to find role="heading" and name "Streaming turn"`.
Baseline command: `pnpm exec vitest run src/App.stream.test.tsx --maxWorkers=1
--minWorkers=1`, exit 1 in 2.95s. Retained command/log evidence:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-web-auth-baseline-710a410-ru8e8iny`.

This independent test-owner maintenance supplies a synthetic launcher fragment
in `web/src/test/setup.ts`. Existing test-method source and all product files
remain byte-identical to the base. New credential-boundary controls explicitly
clear that fragment and verify missing/malformed credentials fail before fetch,
valid credentials leave the URL before fetch, and a new document needs a fresh
credential. They guard against accidentally making authentication optional for
the older component fixtures.

No product acceptance is claimed by this fixture repair. Commands, hashes and
remaining failures are recorded after execution below.

## Focused executable evidence and new transport RED

With the synthetic launcher fragment, the original streaming test passes with
all its assertions unchanged. The new missing-credential and valid-fragment
controls also pass. The malformed-fragment control fails: the production API
client falls through to fetch when a nonempty unrecognized fragment is present.
The expected fail-closed request boundary is already enforced for an empty
fragment. This is a reproduced transport failure ready for a minimal product
repair, with the three new assertions frozen before that repair.

Evidence:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-web-auth-fixture-qf9n1tbx`.
Command: `node /Users/mymac/Projects/opencode-rk-main-v2/web/node_modules/vitest/vitest.mjs
run src/App.stream.test.tsx src/lib/api.auth.test.ts --maxWorkers=1 --minWorkers=1`.
Result: 3/4 passed, malformed-credential test RED. The direct pinned Vitest
entrypoint avoids a pnpm auto-install check rejecting a shared dependency symlink;
that earlier infrastructure failure is retained separately.
