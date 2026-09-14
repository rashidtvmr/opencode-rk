# DISC-003 worklog

## Current bounded reconciliation slice

DISC-003 remains **IN PROGRESS / NOT ACCEPTED**. This change does not claim full public-surface extraction or release evidence. It advances exactly one previously queued candidate family, `9router.translation-proxy`, to a reviewed `partial` reference record while retaining explicit unresolved work.

## Pinned evidence

- 9router commit: `17c4cc76877bd1755030a8414f8d0083f48dcccf`.
- New source evidence `NR-RESPONSES-TRANSLATOR`: `open-sse/translator/formats/responsesApi.js:1-194`, blob `5454f9f3f44f1d7fa428d2feae8e90f73f2a3e66`. It covers Responses input normalization, call-id/output coercion, and Responses→chat request translation but is not the complete format/provider matrix.
- Existing caller evidence `NR-CHAT`: `src/sse/handlers/chat.js`, blob `7aa530d381c1cc1da4f6ca667a7abe271a8fbc8e`; the compatibility chat handler enters the shared SSE/provider translation path.
- Existing executor/caller evidence `NR-CODEX-EXECUTOR`: `open-sse/executors/codex.js:1-55`, blob `de2af8229fcf97d04f12f1e1f73a0d92ad396296`; the executor imports Responses normalization and performs provider-specific request transformation.
- Existing spec evidence `NR-ARCHITECTURE`: `docs/ARCHITECTURE.md:1-120`, blob `548c31908d73003617d630d7a15e45597e7b8183`; it explicitly describes request/response translation and the shared SSE + Translation Core.
- New direct test evidence `NR-RESPONSES-TRANSLATION-TEST`: `tests/translator/bugs-codexCli-responses.test.js:1-75`, blob `573b5eb0c5ba99a8417c4c230a33dbb13458cd43`. It exercises Responses↔OpenAI conversion and also records two known upstream failing edge cases.
- New direct test evidence `NR-CODEX-TOOL-NORMALIZATION-TEST`: `tests/unit/codex-tool-normalization.test.js:1-203`, blob `c71938983b578d0589bbb1e315908121569e540b`. It exercises Codex Responses tool normalization and schema sanitization.

## Reconciliation decision

- `9router.translation-proxy` is moved from `queued` to `partial`, with `implementationStatus: reference-implemented`. This status describes observed pinned 9router behavior only; it does not say OpenCode RK has implemented or must reproduce 9router's architecture.
- The record keeps two unresolved findings: enumerate every request/response format adapter, provider executor, and streaming transform; and reconcile the broader provider/format test matrix plus the known-failing translation cases.
- Ledger scope remains exactly 32 DISC-002 candidate families. The expected count moves from 11 partial / 21 queued to 12 partial / 20 queued. DISC-003 task status remains `IN PROGRESS` and controller acceptance remains untouched.

## Validation

- `/usr/bin/python3 tools/reconcile_surfaces.py` passed and regenerated the hash-bound manifest with exactly 32 families: 12 `partial`, 20 `queued`; implementation statuses 7 `partial`, 5 `reference-implemented`, 20 `unresolved`; 49 evidence references; 45 unresolved findings; zero validation errors.
- `/usr/bin/python3 tools/validate_plan.py` => `validate_plan: OK  stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- `jq empty` passed for the supplemental evidence catalog, reconciliation ledger, generated manifest, and workspace source map.
- `git diff --check` passed.
- No product/runtime source, task acceptance state, or `ralph.json` verifier/controller status is changed by this reconciliation slice.

## Follow-on bounded reconciliation slice

The next source-only slice keeps DISC-003 **IN PROGRESS / NOT ACCEPTED** and promotes three additional candidate families from `queued` to reviewed `partial` reference records without claiming target implementation:

- `9router.dashboard-api`: pinned model-availability and provider-list API sources, dashboard callers, provider-status classification tests, and architecture evidence. The record explicitly leaves the canonical status projection unresolved because the reference separately exposes cooldown/unavailable model rows, provider connected/error totals, noAuth classification, expired-cooldown behavior, latest error metadata, and secret-free API projection.
- `9router.dashboard-settings`: pinned settings GET/PATCH source, profile-page caller, settings persistence test, conditional E2E API read, and architecture evidence. The record keeps public-key enumeration, validation, secret handling, authentication settings, proxy/combo side effects, auto-ping loading, and other optional/runtime behaviors unresolved.
- `9router.network-proxy`: pinned outbound proxy validation and per-connection proxy resolution sources, settings API caller, proxy security tests, and architecture evidence. The record keeps proxy pools/relay rewriting, proxy-test APIs, MITM handlers/certificates, platform branches, and the target native capability/security boundary unresolved; it does **not** infer that OpenCode RK should mutate inherited proxy environment or implement MITM.

This moves the bounded ledger from 12 partial / 20 queued to 15 partial / 17 queued while preserving all 32 DISC-002 candidate families. No runtime source or controller acceptance state is changed.

### Follow-on validation

- `/usr/bin/python3 -m unittest tests.bootstrap.test_disc003_reconciliation -v` => 8/8 passed, including the negative checks for scope shrinkage, missing evidence, queued implementation claims, and manifest hash binding.
- `/usr/bin/python3 tools/reconcile_surfaces.py` => passed with exactly 32 families: 15 `partial`, 17 `queued`; implementation statuses 7 `partial`, 8 `reference-implemented`, 17 `unresolved`; 65 evidence references; 48 unresolved findings; zero validation errors.
- `/usr/bin/python3 tools/validate_plan.py` => `validate_plan: OK  stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- JSON parsing passed for the supplemental evidence catalog, reconciliation ledger, generated manifest, and workspace source map.
- `git diff --check` passed.

## Prompt-context reconciliation slice

The next bounded source-only slice promotes `opencode.prompt-context` from `queued` to reviewed `partial` without claiming target implementation or DISC-003 completion.

- Source evidence: pinned `packages/core/src/reference.ts` and `packages/core/src/reference/guidance.ts` show a scope-owned transformable reference registry plus model-visible guidance generation. Reference finalization distinguishes inert local metadata from Git sources that delegate clone/refresh to `RepositoryCache`.
- Caller evidence: pinned `packages/core/src/session/runner/llm.ts` loads `SystemContextRegistry` and `ReferenceGuidance` during provider request assembly.
- Direct tests: `packages/core/test/reference.test.ts` verifies scoped registration/removal, local source metadata, Git path derivation with cache I/O mocked, and descriptions; `packages/core/test/reference-guidance.test.ts` verifies only described registered references become model-visible guidance.
- Spec evidence: pinned `specs/v2/session.md` records durable typed prompt attachments complete while native template/@ mention expansion, configured references, and several instruction/materialization paths remain partial or missing.
- Explicit boundary: Git repository clone/refresh, filesystem materialization, remote instructions, and attachment resolution remain unresolved owners. This slice does not authorize target network/storage behavior.

Validation for this slice:

- `/usr/bin/python3 -m unittest tests.bootstrap.test_disc003_reconciliation -v` => 8/8 passed.
- `/usr/bin/python3 tools/reconcile_surfaces.py` => 32 families: 16 `partial`, 16 `queued`; implementation statuses 8 `partial`, 8 `reference-implemented`, 16 `unresolved`; 71 evidence references; 49 unresolved findings; zero errors.
- `git diff --check` passed.
