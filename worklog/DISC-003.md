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

## Recorder / Effect runtime / repository operations reconciliation slice

The next source-only slice keeps DISC-003 **IN PROGRESS / NOT ACCEPTED** and promotes three OpenCode families from `queued` to reviewed `partial` without claiming target implementation:

- `opencode.provider-recording`: pinned `@opencode-ai/http-recorder` source, V2 SessionRunner integration caller, direct record/replay test suite, and package README contract. The record captures deterministic ordered HTTP/WebSocket replay, redaction/secret rejection, local recording, and the documented buffered-stream/WebSocket limits. It explicitly does **not** assign live network recording, filesystem cassette persistence, `CI`/environment policy, or environment-secret scanning to a target task by inference. Locally implemented `PROV-012` remains untouched; `OPS-009` also appears in repository-operations and still needs task-level decomposition.
- `opencode.effect-runtime`: pinned LayerNode graph compiler, managed runtime, generic Effect/Drizzle SQLite adapter, AppNode caller, direct LayerNode/SQLite tests, and storage adapter spec. The record keeps accepted BASE lifecycle and DB transaction/adapter semantics separate. It also records a newly found reconciliation defect: accepted `BASE-008` explicitly describes an effect-runtime retry/timeout/batch adapter but DISC-002 omitted BASE-008 from this surface's feature IDs, so this family cannot be used to invent OPS-007 ownership.
- `opencode.repository-operations`: pinned repository parser/cache source, reference-runtime caller, pure/live repository tests, and V2 Git-reference spec. The record captures branch validation, branch-isolated cache identity, serialized clone/refresh/checkout/reset, stale-origin replacement, and typed failures, while leaving container installation and observability portions of the candidate family unexhausted. Parser-only behavior must not be mislabeled full INT-002, and the record does not authorize filesystem/network/Git/environment side effects for OPS tasks merely from a path-derived nomination.

This moves the bounded ledger from 16 partial / 16 queued to 19 partial / 13 queued while preserving all 32 DISC-002 candidate families. It improves the evidence needed to reason about `OPS-007` and `OPS-009`, but deliberately stops short of a product contract where cross-surface or accepted-owner overlap remains unresolved.

Validation for this slice:

- `/usr/bin/python3 tools/reconcile_surfaces.py` => passed with exactly 32 families: 19 `partial`, 13 `queued`; implementation statuses 11 `partial`, 8 `reference-implemented`, 13 `unresolved`; 90 evidence references; 54 unresolved findings; zero validation errors.
- `/usr/bin/python3 -m unittest tests.bootstrap.test_disc003_reconciliation -v` => 8/8 passed, including manifest hash binding, feature-scope preservation, pinned-repository evidence, partial-state evidence, and queued-state negative checks.
- `/usr/bin/python3 tools/validate_plan.py` => `validate_plan: OK  stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- `jq empty` passed for the supplemental evidence catalog, reconciliation ledger, generated manifest, and workspace source map.
- `git diff --check` passed.
- Dirty-tree inspection showed exactly the seven intended DISC-003 evidence/reconciliation/manifest/task/worklog/workspace files; no runtime source, `ralph.json`, verifier/controller status, or accepted task card was changed.

## Contracts/schema reconciliation slice

The next source-only slice keeps DISC-003 **IN PROGRESS / NOT ACCEPTED** and promotes `opencode.contracts-schema` from `queued` to reviewed `partial`.

- Pinned source evidence now covers the browser-safe `Event` definition/inventory combinators, canonical `EventManifest`, and Core's exact `ServerDefinitions` facade.
- Pinned caller evidence shows Protocol deriving its SSE event union and `/api/event` schema directly from `EventManifest.ServerDefinitions`, preserving one canonical contract identity rather than a second server-local schema.
- Direct tests cover manifest/current-definition identity, durable-version selection, V1-only event exclusion, omitted-undefined encoding, stable unique identifiers, and avoidance of current-contract `Schema.Any`/mutable wrappers.
- The package guide explicitly defines the authority boundary: Schema owns serializable browser-safe wire/storage contracts; service/runtime behavior, side effects, and host-local implementation stay in the owning domain packages.
- Accepted/local `BASE-002`, accepted `DB-003`, and accepted/local `SESS-002` remain separate owners and are not reopened. Although those siblings leave `INT-008` as the only unfinished id within the contracts-schema feature set, `INT-008` is also nominated by the still-queued integrations, legacy-compatibility, and server-control-plane families. This promotion therefore does **not** invent an INT-008 product contract.

This moves the bounded ledger from 19 partial / 13 queued to 20 partial / 12 queued, with 97 pinned evidence references and 56 explicit unresolved findings.

Validation for this slice:

- `/usr/bin/python3 -m unittest tests.bootstrap.test_disc003_reconciliation -v` => 8/8 passed.
- `/usr/bin/python3 tools/reconcile_surfaces.py` => 32 families: 20 `partial`, 12 `queued`; implementation statuses 12 `partial`, 8 `reference-implemented`, 12 `unresolved`; 97 evidence references; 56 unresolved findings; zero errors.
- `/usr/bin/python3 tools/validate_plan.py` => `validate_plan: OK  stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- `jq empty` passed for the evidence catalog, reconciliation ledger, manifest, and workspace source map.
- `git diff --check` passed.
- Dirty-tree inspection showed exactly the seven intended DISC-003 evidence/reconciliation/manifest/task/worklog/workspace files; no runtime source, `ralph.json`, accepted task card, or verifier/controller state changed.

## INT-008 consumer-surface reconciliation slice

The next source-only slice keeps DISC-003 **IN PROGRESS / NOT ACCEPTED** and promotes the three remaining INT-008 consumer families from `queued` to reviewed `partial`: `opencode.integrations`, `opencode.legacy-compatibility`, and `opencode.server-control-plane`.

- `opencode.integrations`: pinned client/server contract projections both derive from the authoritative Protocol HttpApi, generation-equivalence tests reject transport drift, the Effect client decodes current SSE/durable metadata, and the typed Integration HTTP handler remains a thin caller over the core Integration service. The record leaves workspace/project/Git/PTY, generated SDK breadth, provider-specific auth, remote transports, and INT-001/003/005/006/007/009/010 unresolved.
- `opencode.legacy-compatibility`: pinned `EventV2Bridge` maps current event payloads to the legacy `{id,type,properties}` bus shape plus durable versioned `sync` envelopes, while V2 config lowering emits explicit invalid/unsupported/conflict diagnostics. Canonical legacy schemas and Core wrapper identity have direct tests. The record does not assign GlobalBus lifecycle, location lookup, config ownership, or runtime subscriber side effects to INT-008.
- `opencode.server-control-plane`: pinned Server/Protocol API composition, typed MoveSession control-plane service/handler, direct route test, and route-policy guide show that HttpApi owns wire/error translation while domain/storage services stay free of transport types. The record explicitly leaves Git/project/session mutations, event publication, daemon/bootstrap/auth/workspace routing, and WEB-004 remote exposure with their separate owners.

Together with the already reviewed `opencode.contracts-schema`, the four behavior-rule families that nominate INT-008 now have pinned partial evidence. Their exact feature-set intersection is INT-008, but this reconciliation still does **not** prove that every observed behavior belongs in one Rust implementation: accepted BASE/DB/SESS semantics stay closed, generated-client/network behavior remains broader, and the legacy bridge contains runtime bus/location effects beyond a pure wire contract. A task-level ownership audit is required before any INT-008 RED suite is authored.

This moves the bounded ledger from 20 partial / 12 queued to 23 partial / 9 queued, with 115 pinned evidence references and 62 explicit unresolved findings.

Validation for this slice:

- `git rev-parse --short HEAD` before commit => `d9b4b94`.
- Dirty-tree inspection showed exactly the seven intended DISC-003 evidence/reconciliation/manifest/task/worklog/workspace files: `sources/disc-003-evidence.json`, `sources/disc-003-reconciliation.json`, `sources/disc-003-reconciliation.manifest.json`, `tasks/DISC-003.md`, `worklog/DISC-003.md`, `workspaces/DISC-003/progress.md`, and `workspaces/DISC-003/source-map.json`.
- `git diff --name-only -- ralph.json` returned no path; no runtime source, accepted task card, verifier/controller state, or `ralph.json` acceptance field changed.
- `/usr/bin/python3 -m unittest tests.bootstrap.test_disc003_reconciliation -v` => 8/8 passed.
- `/usr/bin/python3 tools/reconcile_surfaces.py` => 32 families: 23 `partial`, 9 `queued`; implementation statuses 15 `partial`, 8 `reference-implemented`, 9 `unresolved`; 115 evidence references; 62 unresolved findings; zero errors.
- `/usr/bin/python3 tools/validate_plan.py` => `validate_plan: OK  stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- `jq empty` passed for `sources/disc-003-evidence.json`, `sources/disc-003-reconciliation.json`, `sources/disc-003-reconciliation.manifest.json`, and `workspaces/DISC-003/source-map.json`.
- `git diff --check` passed.
- DISC-003 remains **IN PROGRESS / NOT ACCEPTED** with reconciliation status `in-progress-not-release-evidence`; these results are local source-reconciliation evidence only and do not claim verifier/controller completion.

## Configuration-runtime reconciliation slice

The next source-only slice keeps DISC-003 **IN PROGRESS / NOT ACCEPTED** and promotes `opencode.configuration-runtime` from `queued` to reviewed `partial` without authorizing an OPS implementation.

- The candidate rule spans accepted/local BASE-004/005/006/007/008 plus residual OPS-001/004/005/008. Accepted BASE configuration, feature-gating, lifecycle, and already-owned effect-runtime semantics remain closed and are not re-owned by this slice.
- Pinned config evidence `OC-CONFIG-RUNTIME` (`packages/core/src/config.ts:122-217`, blob `c76486968b0784d57dab102e2d51a7e18f57b7f5`) plus `OC-CONFIG-RUNTIME-TEST` (`packages/core/test/config/config.test.ts:146-785`, blob `c3c42cab3022844a27a95a8381c16ed4c4b54ba0`) establishes location-scoped priority-ordered config discovery, read-once/reopen behavior, V1 migration, and filesystem/global/location dependencies. Those semantics substantially overlap accepted BASE configuration ownership rather than proving a residual OPS task.
- `OC-STATE-RUNTIME` (`packages/core/src/state.ts:29-127`, blob `ab3457fc18143c10d830979dbde3a9375e194ccf`) and `OC-STATE-RUNTIME-TEST` (`packages/core/test/state.test.ts:8-114`, blob `505cd724e7e324e665d195d950fc36b517fe53ad`) cover semaphore-serialized scoped transforms, replay on reload, idempotent disposal, interruption-safe commit, and batching. No pinned task decomposition assigns those behaviors to OPS-001/004/005/008.
- `OC-LOCATION-SERVICES` (`packages/core/src/location-services.ts:42-115`, blob `7da67673c31982abafc856f34fb52a2c82891525`) and `OC-LOCATION-LAYER-TEST` (`packages/core/test/location-layer.test.ts:39-62`, blob `e28e758c87b8d29d1cb5854e6541ab426aa2e472`) establish a `LayerMap` location-service cache with a 60-minute idle TTL and canonical `Location.Ref` cache reuse. Existing caller `OC-APP-NODE-BUILDER` injects the location map only when the graph requires it, while `OC-SESSION-SPEC` records runner/catalog/model/tool/permission/filesystem services cached per Location. This is concrete runtime/lifetime evidence but still not one residual OPS product contract.
- `OC-GLOBAL-RUNTIME` (`packages/core/src/global.ts:10-85`, blob `a192a4b4684fe2dc308665f3d3fa64c866bf8ccc`) includes process-environment reads and eager import-time directory creation. `OC-NPM-CONFIG` (`packages/core/src/npm-config.ts:12-40`, blob `896bb84872c1ffce95a81d6562214468769dcab3`) loads npm config from a copied process environment and normalizes registry URLs. Those side effects are recorded as upstream facts, not automatically adopted target requirements.
- `OC-INSTALLATION-VERSION` (`packages/core/src/installation/version.ts:1-8`, blob `25d9cd99aa6c0e4110a3efba8696c36bfcfaadb1`) is only compile-time version/channel/local metadata. OPS-004 and OPS-008 also appear in reviewed `opencode.repository-operations`; the broader legacy Installation/Git/cache/observability behavior lives outside this candidate path family, so ownership remains unresolved rather than inferred from the overlap.
- Location-mutation and control-plane paths mix domain/security/Git/project/session/event behavior already separated by other reviewed evidence. They are not assigned to residual OPS tasks by path membership alone.

The promoted record therefore keeps four explicit unresolved findings: preserve accepted BASE ownership; obtain a task/path decomposition for residual OPS-001/004/005/008; resolve the OPS-004/008 repository-operations overlap and broader installation ownership; and separately authorize any global/environment/npm/location-mutation/control-plane side effects. No product tests or runtime implementation are justified by this evidence alone.

This moves the ledger from 23 partial / 9 queued to 24 partial / 8 queued while preserving all 32 DISC-002 candidate families. The expected implementation-status ledger is 16 `partial`, 8 `reference-implemented`, and 8 unresolved queued families. The reconciliation currently binds 126 evidence references and retains 65 explicit unresolved findings.

Validation for this slice after assembling the final seven-file boundary:

- `/usr/bin/python3 -m unittest tests.bootstrap.test_disc003_reconciliation -v` => 8/8 passed, including candidate coverage, pinned-repository evidence, partial-state evidence, queued-state negative checks, and manifest hash binding.
- `/usr/bin/python3 tools/reconcile_surfaces.py` => passed with exactly 32 families: 24 `partial`, 8 `queued`; implementation statuses 16 `partial`, 8 `reference-implemented`, 8 `unresolved`; 126 evidence references; 65 unresolved findings; zero errors. The hash-bound manifest was regenerated.
- `/usr/bin/python3 tools/validate_plan.py` => `validate_plan: OK  stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- `jq empty` passed for `sources/disc-003-evidence.json`, `sources/disc-003-reconciliation.json`, `sources/disc-003-reconciliation.manifest.json`, and `workspaces/DISC-003/source-map.json`.
- `git diff --check` passed.
- Dirty-tree inspection showed exactly the seven intended DISC-003 files: `sources/disc-003-evidence.json`, `sources/disc-003-reconciliation.json`, `sources/disc-003-reconciliation.manifest.json`, `tasks/DISC-003.md`, `worklog/DISC-003.md`, `workspaces/DISC-003/progress.md`, and `workspaces/DISC-003/source-map.json`.
- `git diff --name-only -- ralph.json` returned no path; no runtime source, OPS task card, accepted-task state, verifier/controller state, or `ralph.json` acceptance field changed.
- DISC-003 remains **IN PROGRESS / NOT ACCEPTED** with reconciliation status `in-progress-not-release-evidence`; these are local source-reconciliation results only and do not claim verifier/controller completion.

## Accepted-owner runtime reconciliation slice

The next source-only slice keeps DISC-003 **IN PROGRESS / NOT ACCEPTED** and promotes three path-derived OpenCode candidates from `queued` to reviewed `partial`: `opencode.code-mode`, `opencode.process-terminal`, and `opencode.storage-events`. Live `ralph.json`, task/history evidence, and the behavior rules show that every target feature nominated by these three families is already accepted/local, so the slice records upstream behavior without creating or reopening a product task.

- `opencode.code-mode` maps only to TOOL-011, SEC-001, and SEC-010, all already accepted/local. `OC-CODEMODE-RUNTIME` and `OC-CODEMODE-TOOL-RUNTIME` bind the confined grouped-tool interpreter/runtime and schema boundary; caller `OC-CODEMODE-TOOL-CALLER` binds the permission-filtered MCP tool projection, plugin before/after calls, abort propagation, metadata, and attachments; direct CodeMode/OpenAPI tests and the package guide supply test/spec evidence. The record explicitly does not use upstream structured plugin-hook mutation/failure behavior to bypass the separately recorded EXT-008 blocker, and it does not authorize embedding a JS/TS runtime.
- `opencode.process-terminal` maps only to SEC-001, SEC-002, SEC-008, TOOL-005, and TOOL-013, all already accepted/local. `OC-PROCESS-RUNTIME` covers bounded stdout/stderr, timeout/abort, stdin, and streaming; `OC-CROSS-SPAWN-RUNTIME` covers scoped process acquisition/release, environment/cwd/fd setup, pipelines, group termination, and escalation; `OC-PTY-RUNTIME` records the 2 MiB retained-output bound and at-most-25 retained exited sessions plus replay/attach lifecycle; `OC-SHELL-RUNTIME` records platform shell selection and argument/kill policy. Four pinned direct test suites cover lifecycle, cleanup, replay isolation, resource bounds, and platform branches. No native PTY/process/environment/filesystem side effect is newly authorized by this evidence.
- `opencode.storage-events` maps exactly DB-001 through DB-016, all already accepted/local. New pinned evidence binds the SQLite bootstrap/migration service (`OC-DATABASE-RUNTIME`), durable EventV2 sequencing/transaction/replay-live runtime (`OC-EVENT-RUNTIME` plus direct tests), Git/filesystem-backed snapshots (`OC-SNAPSHOT-RUNTIME` plus tests), and bounded tool-output spill/retention (`OC-TOOL-OUTPUT-STORE` plus tests). The record reuses already pinned generic Effect/Drizzle adapter evidence and the storage adapter spec, and adds `OC-STORAGE-REMOVE-DB-SPEC` for migration/runtime ownership notes. Accepted DB behavior remains closed, INT-008 wire compatibility remains separately owned, and this source review is not permission to mutate a user database or repository.

The three records intentionally remain `implementationStatus: partial`: their bounded evidence does not exhaust every interpreter/stdlib/OpenAPI path, PTY protocol/platform adapter, generated migration/schema/event consumer, or storage side effect. Because the mapped feature owners are already accepted, those open traversal items are DISC evidence gaps rather than implied unfinished product tasks.

This moves the ledger from 24 partial / 8 queued to 27 partial / 5 queued while preserving all 32 DISC-002 candidate families. The implementation-status ledger is 19 `partial`, 8 `reference-implemented`, and 5 unresolved queued families. The reconciliation binds 151 evidence references and retains 68 explicit unresolved findings.

Validation for this slice after assembling the final seven-file boundary:

- `/usr/bin/python3 -m unittest tests.bootstrap.test_disc003_reconciliation -v` => 8/8 passed, including exact candidate coverage, pinned-repository evidence, partial-state evidence, queued-state negative checks, and manifest hash binding.
- `/usr/bin/python3 tools/reconcile_surfaces.py` => passed with exactly 32 families: 27 `partial`, 5 `queued`; implementation statuses 19 `partial`, 8 `reference-implemented`, 5 `unresolved`; 151 evidence references; 68 unresolved findings; zero errors. The hash-bound manifest was regenerated.
- `/usr/bin/python3 tools/validate_plan.py` => `validate_plan: OK  stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- `jq empty` passed for `sources/disc-003-evidence.json`, `sources/disc-003-reconciliation.json`, `sources/disc-003-reconciliation.manifest.json`, and `workspaces/DISC-003/source-map.json`.
- `git diff --check` passed.
- Dirty-tree inspection showed exactly the seven intended DISC-003 files: `sources/disc-003-evidence.json`, `sources/disc-003-reconciliation.json`, `sources/disc-003-reconciliation.manifest.json`, `tasks/DISC-003.md`, `worklog/DISC-003.md`, `workspaces/DISC-003/progress.md`, and `workspaces/DISC-003/source-map.json`.
- `git diff --name-only -- ralph.json` returned no path; no runtime source, accepted task card, verifier/controller state, or `ralph.json` acceptance field changed.
- The process-terminal follow-up audit independently confirmed the direct feature set is fully accepted in live `ralph.json`; TOOL-013 has no task card, so this slice records source behavior without inferring TOOL-013 semantics from its numeric id.
- DISC-003 remains **IN PROGRESS / NOT ACCEPTED** with reconciliation status `in-progress-not-release-evidence`; these are local source-reconciliation results only and do not claim verifier/controller completion.
