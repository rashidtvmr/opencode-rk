# WEB-009 browser structured activity implementation

## Candidate boundary

- Integrated RED commit: `7cce88f`; implementation claim commit: `b99f672`.
- Frozen test: `web/src/App.browser-red.test.tsx`.
- Frozen SHA-256: `8dbbb07c5d1ff6d1ff10ddcf2fa020aa3fff318a53a6d994d4f6e786fc11901d`.
- Product scope: `web/src/lib/api.ts` and `web/src/App.tsx`. No test was edited.

## Observable contract

- Browser activity uses one bounded model for reasoning, completed/failed tool lifecycle, and HTTPS references.
- Reload responses accept legacy reasoning-only activity as empty tool/reference arrays, while malformed records, duplicate IDs/URLs, over-cap arrays, control characters, credential-bearing URLs, non-HTTPS URLs, and mismatched state/ok values fail closed.
- Stream parsing supports the production `tool_call`, `tool_output`, and `reference` events and the aggregate `assistant_activity` event exercised by the frozen browser journey. Aggregate/final activity must agree before projection.
- Limits match the Rust contracts: 128 tool records, 64 references, 128-byte tool fields, 1024-byte reference fields, and 8 KiB reasoning summary.
- Reasoning and Tool activity render as separate collapsed native `details` controls. Final answer remains expanded. References render as labeled HTTPS links with `rel="noreferrer"`; streamed answer deltas are not inserted into a live region.

## Verification

Bounded serial commands on the product spine:

- Frozen browser target: 3 passed, 0 failed.
- Existing activity/turn plus frozen target: 5 passed, 0 failed.
- Entire web Vitest suite: 17 files, 49 passed, 0 failed.
- `pnpm exec tsc -b --pretty false`: blocked before WEB-009 validation by pre-existing TS6133 unused imports in `web/src/lib/canvas-model.test.ts`; that frozen/unrelated test was not edited.
- `git diff --check`: passed.
- Frozen test hash re-read unchanged.

## Remaining blockers

This is a GREEN candidate, not acceptance. WEB-009 remains blocked pending independent verification, the third exact disconnect/cancellation pass, production browser evidence, and branch-session structured activity unification. The repository/convergence gates remain independently blocked by their recorded authority conflicts.
