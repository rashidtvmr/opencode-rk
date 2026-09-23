# Phase 1 product-spine integration branch

## Boundary

- Branch: `lane/PHASE1-product-spine-20260923`.
- Base: `06ed486`, the pushed Phase 1 candidate before audit-only waves.
- Purpose: serialize verified product and frozen-RED commits without importing
  audit-only ledger history or the dirty original integration worktree.

## Admission rules

1. Source lane branch is pushed and retained.
2. Exact changed files and claim row are verified from Git, not chat output.
3. New tests have a compiling RED, frozen SHA-256, and bounded command manifest.
4. Product commits do not edit frozen tests.
5. Focused GREEN and regressions rerun on this exact integrated branch.
6. `validate_repository.py` and convergence failures remain explicit blockers;
   no accepted flag, verifier, policy, or safeguard is weakened.

## Pending candidates

- UI-014 renderer-independent composer prewire integrated at `cf80be7` after
  verifying the pushed three-file commit from disk. It replaces the native
  loop's second draft with one bounded `native_composer::Composer` and exposes
  a renderer-independent event seam. Lane non-native check passed; the exact
  integrated check was deferred under memory pressure. Async turn ownership,
  queue-while-busy, cancellation/join, PTY decoding, Unicode/paste/focus input,
  behavioral RED, and the arm64 OpenTUI artifact remain blockers.
  The preserved external fixture at
  `/Users/mymac/Projects/opencode-rk-web006-integrate/crates/opentui-bridge/native/lib/aarch64-apple-darwin/libopentui.dylib`
  was verified as a broken symlink to the absent
  `opentui-pinned/packages/native/lib/aarch64-macos/libopentui.dylib`; it is not
  usable native-build evidence and remains unmodified/untracked.
  Independent behavioral-RED review integrated at `0464988` confirms no honest
  public caller seam exists before a production-owned async worker with bounded
  result channels and cancellation/join is wired. No test was fabricated from
  pure composer state.
- WEB-009 durable tool/reference persistence RED integrated through `00d0c48`.
  Frozen SHA-256 is
  `c0acaf32cbf235452499acbe3eb5f617b7e319f8e6043a7b3cd54720a5b81bb8`.
  The authenticated disposable-SQLite fixture compiles and fails at the real
  provider adapter with unsupported event
  `response.output_text.annotation.added`; native citation production and the
  durable tool/reference read API remain missing.
  Implementation integrated at `08a40be`: the authenticated turn path now
  bounds and persists reasoning, tool lifecycle, and HTTPS references in one
  assistant-message transaction. Exact integrated frozen rerun: 1 passed,
  0 failed; frozen SHA-256 remained unchanged. WEB-009 stays blocked pending
  browser accessibility, the third exact disconnect pass, branch activity
  unification, and independent integrated verification.
  Browser RED integrated at `7cce88f` and implementation at `598e2f3`: the
  client now validates bounded tool/reference activity, consumes production
  stream events, renders collapsed Reasoning/Tool controls and navigable HTTPS
  References, and preserves reload fidelity. Frozen browser target is 3/3 and
  the full web Vitest suite is 49/49. Typecheck remains independently blocked by
  pre-existing TS6133 errors in `web/src/lib/canvas-model.test.ts`.
- PROV-023 runtime catalog RED integrated at `896103a`: frozen test SHA-256
  `f74dba4c600c89f59d011fff0361ef6d5be1925d113ac750b7897889b1d57e95`.
  Its public request-path case compiles and fails because Google is documented
  but `profile_for` returns `UnknownProvider`; malformed/version/cap loader seam
  remains absent, so the task stays blocked after this RED eventually turns green.
  Exact integrated rerun (bounded through Python because macOS has no `timeout`
  binary): 4 tests, T02–T04 pass, sole T01 failure is `UnknownProvider`; exit 101.

No candidate is integrated merely because it compiles or has isolated GREEN
tests. The hard installed `oc2` journey in `docs/CONVERGENCE.md` remains the
parent acceptance boundary.

## Exact integrated structural gates

At `0464988`, after WEB-009 browser GREEN and UI-014 no-seam integration:

- `python3 tools/convergence_gate.py`: blocked with the unchanged 80 findings.
- `python3 tools/validate_repository.py`: protection fixtures pass, then the
  unchanged backlog-exhaustion contract conflict reports 51 errors.

No accepted flag, ownership source, frozen validator, evidence file, or policy
was weakened to alter either result.
