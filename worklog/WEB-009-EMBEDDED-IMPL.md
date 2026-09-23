# WEB-009 embedded production bundle implementation

## Candidate boundary

- Frozen embedded RED integrated at `2f06398`; implementation claim at `f2b4d9d`.
- Frozen test: `crates/server/tests/web009_embedded_activity_red.rs`.
- Frozen SHA-256: `dd2029891ec289cbc3b75abb07f646eb4e2024ff9b5ed48944c4291640eb0a95`.
- Product artifact: `crates/server/web_dist/**`, generated from the already verified `web/src` tree. No test or web source was edited in this step.

## Implementation

`pnpm exec vite build` rebuilt the embedded production artifact through `web/vite.config.ts`, replacing the stale hashed JS/CSS assets and updating `index.html` to the new same-origin module path. The new server-embedded module contains the bounded structured-activity parser and the Tool activity, Reasoning summary, References, HTTPS, and noreferrer presentation semantics verified in source at `598e2f3` and independently at `220de90`/integrated `d20ebae`.

The normal `pnpm build` wrapper remains blocked before Vite by pre-existing TS6133 errors in frozen `web/src/lib/canvas-model.test.ts`; that unrelated test was not edited. Vite itself completed successfully from the source revision whose full Vitest suite is GREEN.

## Verification

- Exact authenticated embedded-router target: 1 passed, 0 failed.
- Existing `web_assets` router target: 1 passed, 0 failed.
- Full web Vitest suite: 17 files, 49 passed, 0 failed.
- Two consecutive Vite builds produced byte-identical output hashes:
  - `web_dist/index.html`: `517933d24e283e2a1792fe25eac29cc6688bb7449654418be28ba7e926ce61fe`
  - CSS: `16b43a730b810f08f3bfb521454484f9a71b3beea18cff9c15b801769261fc12`
  - JS: `20260b4692e59a440ddfa2834dcee71c0f68608869f875e0f58a90b41a9fc316`
- Frozen embedded test hash re-read unchanged.
- `git diff --check`: passed.

The Vite size report warns that the single 509.25 kB JS chunk exceeds its advisory 500 kB threshold; the router/test 2 MiB hard body cap remains satisfied. Code splitting is a performance follow-up, not hidden acceptance.

## Remaining blocker

This is a GREEN candidate, not acceptance. WEB-009 still lacks branch-session structured activity unification and independent verification of this exact embedded artifact. External real-provider/browser evidence remains unavailable without credentials; the authenticated router-served artifact itself is now covered locally.
