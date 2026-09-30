# V2-WEB-MECHANICAL-BUILD — independent test-owner mechanical maintenance

Authority: current explicit user instruction + V2 AGENTS.md 64-72 (independent
mechanical maintenance: unused imports, no HITL). Branch: `v2/web-mechanical-build`,
base `fc2d201f450b6020b1dce12268a550053d65235d`. Allowed paths only:
`web/src/lib/canvas-model.test.ts` (mechanical imports) + this worklog.

## RED (before edit, frozen typecheck on exact base tree)
- Command: `arch -arm64 pnpm --dir web run typecheck` (deps preinstalled via
  `arch -arm64 pnpm --dir web install --frozen-lockfile --offline --ignore-scripts`, no lockfile change).
- Diagnostic (exit 2), compiler-identified only:
  - `src/lib/canvas-model.test.ts(1,10): error TS6133: 'describe' is declared but its value is never read.`
  - `src/lib/canvas-model.test.ts(4,8): error TS6133: 'CanvasEdge' is declared but its value is never read.`
  - `src/lib/canvas-model.test.ts(6,8): error TS6133: 'CanvasState' is declared but its value is never read.`
  - `src/lib/canvas-model.test.ts(7,8): error TS6133: 'Viewport' is declared but its value is never read.`
  - `src/lib/canvas-model.test.ts(13,3): error TS6133: 'evict' is declared but its value is never read.`
- `evict` verified zero code refs in test body (only T07/T08 comment/it-title text).
- Test-body hash before (bytes after `} from './canvas-model'`): `10392d8c2f17eec24a8f120e25b1aa67cc4872d1621d436e0c8f5b5c17185f44`.

## Change (import specifiers only, no bodies/assertions/flags)
- `web/src/lib/canvas-model.test.ts:1-16`: removed exactly the 5 compiler-flagged
  specifiers (`describe`, `CanvasEdge`, `CanvasState`, `Viewport`, `evict`).
- No assertion, expected-output, it-title, comment, selector, dependency, or
  compiler-setting change. No App/api/server/build-artifact/product change.
- Test-body hash after: `10392d8c2f17eec24a8f120e25b1aa67cc4872d1621d436e0c8f5b5c17185f44` (identical).
- `git diff --check` clean.

## GREEN (after edit, bounded node validation, one worker)
- `arch -arm64 pnpm --dir web run typecheck` -> exit 0, no output beyond `$ tsc -b --pretty false`.
- `arch -arm64 pnpm --dir web exec vitest run src/lib/canvas-model.test.ts --maxWorkers=1` -> 1 file, 16/16 pass (~0.4s).
- `arch -arm64 node --experimental-strip-types --test web/src/lib/api.auth.test.mjs web/src/lib/api.direct-auth.test.mjs web/src/lib/api.refresh-auth.test.mjs` -> 6/6 pass (~145ms).
- No Cargo run (G4 test-owner heavy slot respected). No `package.json`/`pnpm-lock.yaml` change.
- Not a browser acceptance claim: only frozen typecheck + targeted canvas vitest + existing node auth tests.
