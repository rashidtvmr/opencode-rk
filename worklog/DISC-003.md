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

## Sharing and enterprise-remote reconciliation slice

The next source-only slice keeps DISC-003 **IN PROGRESS / NOT ACCEPTED** and promotes `opencode.sharing` and `opencode.enterprise-remote` from `queued` to reviewed `partial`. The source audit found concrete behavior in both families but no one-to-one unfinished native task safe for strict TDD.

- `opencode.sharing` maps SHARE-001 through SHARE-005, all still unfinished/generic in live plan data. `OC-SHARE-SQL` records the session-share persistence row. `OC-ENTERPRISE-SHARE` records typed share data, deterministic key-based last-write replacement, secret-gated create/sync/remove, and legacy-event-to-snapshot migration; `OC-ENTERPRISE-SHARE-ROUTE` exposes create/sync/data/delete over HTTP and a separate support-admin removal path. Direct `OC-ENTERPRISE-SHARE-TEST` covers create/remove, sync/read, latest-write replacement, legacy migration, invalid secret, and missing-share behavior.
- Current runtime caller `OC-SHARE-NEXT-CALLER` makes the task boundary broader: it chooses legacy `/api/share` versus org-authenticated `/api/shares`, persists share metadata in SQLite, subscribes to EventV2 session/message/part/diff/delete events, coalesces changes, performs HTTP sync, and closes its owned scope on location teardown. `OC-SHARE-NEXT-TEST` covers endpoint/auth choice, persisted create/remove, non-OK failure, and delayed coalescing with a mocked transport.
- The caller's queue is key-coalescing but has no explicit global item or byte cap. That is insufficient for a native target contract under the repository's bounded-resource policy. No SHARE task card/path decomposition establishes whether pure merge, persistence, subscriber lifetime, transport/auth, retry policy, deletion, or remote hosting belongs to SHARE-001/002/003/004/005. SHARE-003 also appears in enterprise-remote; cross-family intersection alone is not ownership proof.
- `opencode.enterprise-remote` mixes multiple hosted contracts. `OC-FUNCTION-REMOTE` contains Cloudflare Durable Object websocket share state and R2 persistence, share HTTP routes, Feishu-to-Discord support relay, GitHub Actions OIDC exchange, PAT-based GitHub exchange, and app-installation lookup. `OC-FUNCTION-GITHUB-TEST` only tests repository-claim parsing used by one exchange branch; there is no direct pinned test matrix for SyncServer/websocket lifecycle, share remote routes, support relay, token exchange, or installation lookup.
- The enterprise-remote feature set is WEB-004, PROV-006, INT-010, and SHARE-003. PROV-006 is already accepted; WEB-004/INT-010/SHARE-003 remain generic source-audit rows. The heterogeneous source does not establish how the residual three divide hosted web exposure, integration/auth exchange, and share hosting. Cloudflare/R2, Discord/GitHub network calls, secrets/credentials, websocket fanout, and deployment infrastructure are recorded upstream facts, not authorized target side effects.

The promoted records therefore stay `implementationStatus: partial` and explicitly require task/path decomposition, a dedicated sharing/remote specification, bounded queue/retention/backpressure policy, separation of local/persistent/network/auth responsibilities, and direct remote lifecycle tests before product implementation.

This moves the ledger from 27 partial / 5 queued to 29 partial / 3 queued while preserving all 32 DISC-002 candidate families. The implementation-status ledger is 21 `partial`, 8 `reference-implemented`, and 3 unresolved queued families. The reconciliation binds 163 evidence references and retains 73 explicit unresolved findings.

Validation for this slice after assembling the final seven-file boundary:

- `/usr/bin/python3 -m unittest tests.bootstrap.test_disc003_reconciliation -v` => 8/8 passed, including exact candidate coverage, pinned-repository evidence, partial-state evidence, queued-state negative checks, and manifest hash binding.
- `/usr/bin/python3 tools/reconcile_surfaces.py` => passed with exactly 32 families: 29 `partial`, 3 `queued`; implementation statuses 21 `partial`, 8 `reference-implemented`, 3 `unresolved`; 163 evidence references; 73 unresolved findings; zero errors. The hash-bound manifest was regenerated.
- `/usr/bin/python3 tools/validate_plan.py` => `validate_plan: OK  stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- `jq empty` passed for `sources/disc-003-evidence.json`, `sources/disc-003-reconciliation.json`, `sources/disc-003-reconciliation.manifest.json`, and `workspaces/DISC-003/source-map.json`.
- `git diff --check` passed.
- Dirty-tree inspection showed exactly the seven intended DISC-003 files: `sources/disc-003-evidence.json`, `sources/disc-003-reconciliation.json`, `sources/disc-003-reconciliation.manifest.json`, `tasks/DISC-003.md`, `worklog/DISC-003.md`, `workspaces/DISC-003/progress.md`, and `workspaces/DISC-003/source-map.json`.
- `git diff --name-only -- ralph.json` returned no path; no runtime source, SHARE/WEB/INT task card, credential, deployment configuration, accepted-task state, verifier/controller state, or `ralph.json` acceptance field changed.
- DISC-003 remains **IN PROGRESS / NOT ACCEPTED** with reconciliation status `in-progress-not-release-evidence`; these are local source-reconciliation results only and do not claim verifier/controller completion.

## Final client-surface reconciliation slice

The next source-only slice keeps DISC-003 **IN PROGRESS / NOT ACCEPTED** and promotes the final three queued client families — `opencode.app-client`, `opencode.clients-ui`, and `opencode.desktop-client` — to reviewed `partial` evidence records. Zero queued candidate families after this slice is an inventory milestone only; it is not DISC-003 acceptance, product implementation, or proof of exhaustive dynamic/generated/platform coverage.

- `opencode.app-client` maps WEB-001, WEB-002, WEB-005, UI-003, UI-006, UI-009, and UI-011. Pinned `OC-APP-CLIENT-ROOT` establishes router/provider composition, server-scoped SDK/sync providers, layout compatibility, startup/health gating, and client shell lifetime. `OC-APP-SERVER-PROTOCOL` plus `OC-APP-SERVER-PROTOCOL-TEST` establish bounded V1/V2 health probing with a 5-second request timeout. `OC-APP-TERMINAL-WS` plus its direct test establish V1/V2 terminal WebSocket URL/query construction, and `OC-APP-MODEL-SELECTION-E2E` supplies browser-level behavior evidence. All mapped UI/WEB rows remain unowned at a one-to-one task boundary; the target Rust workspace has no browser-client crate, and network/auth/browser/terminal dependencies remain outside the current native lane.
- `opencode.clients-ui` maps UI-001 through UI-013 plus WEB-001/002/003/005. `OC-CLI-TUI-BRIDGE` shows the CLI host invoking the shared TUI with transport/config/plugin-host inputs. `OC-TUI-RUNTIME` establishes scoped renderer acquisition/release, signal/finalizer cleanup, SDK/event/provider/plugin composition, terminal/process/environment integration, and local UI state. `OC-TUI-LIFECYCLE-TEST` directly checks SIGHUP cleanup and plugin disposal, while `OC-TUI-PACKAGE-SPEC` records the intended TUI/host/SDK ownership separation. The current pinned implementation still carries core/global/process/platform coupling and the target has no approved TUI architecture/dependency set, so no native UI/WEB task is inferred or opened.
- `opencode.desktop-client` maps WEB-003, WEB-005, and accepted BASE-005. `OC-DESKTOP-MAIN` and `OC-DESKTOP-MAIN-TEST` cover Electron main-process lifecycle, initialization failure propagation, sidecar ownership, signal/quit handling, and renderer startup. `OC-DESKTOP-SERVER` covers local sidecar process/network/auth lifecycle; `OC-DESKTOP-PRELOAD` records the IPC surface; `OC-DESKTOP-WSL-SERVERS` plus `OC-DESKTOP-WSL-TEST` cover persisted WSL server state, probing/install/start/stop behavior, stale-attempt rejection, and cleanup. BASE-005 remains closed; WEB-003/005 are not decomposed one-to-one across Electron IPC/window/update/packaging, sidecar, network/auth, filesystem/environment, and WSL/platform behavior. No desktop product implementation is authorized by this review.

The three records remain `implementationStatus: partial`, with explicit unresolved architecture, dependency, platform, and task-ownership findings. Dependency-constrained UI/TUI/desktop work remains excluded from the current product lane, and no task semantics are inferred from numeric IDs.

This moves the ledger from 29 partial / 3 queued to all 32 candidate families reviewed `partial` with **0 queued**. Implementation statuses are 24 `partial` and 8 `reference-implemented`; the reconciliation binds 179 evidence references and retains 79 explicit unresolved findings.

Validation for the final seven-file client slice:

- Dirty-tree inspection at HEAD `eb4234a` showed exactly the seven intended DISC-003 files: `sources/disc-003-evidence.json`, `sources/disc-003-reconciliation.json`, `sources/disc-003-reconciliation.manifest.json`, `tasks/DISC-003.md`, `worklog/DISC-003.md`, `workspaces/DISC-003/progress.md`, and `workspaces/DISC-003/source-map.json`.
- `git diff --name-only -- ralph.json` returned no path; no runtime source, product task card, accepted-task state, verifier/controller state, or `ralph.json` acceptance field changed.
- `/usr/bin/python3 -m unittest tests.bootstrap.test_disc003_reconciliation -v` => 8/8 passed, including exact candidate coverage, pinned-repository evidence, partial-state evidence, exhausted-queue negative coverage, and manifest hash binding.
- `/usr/bin/python3 tools/reconcile_surfaces.py` => passed with exactly 32 families: 32 `partial`, 0 `queued`; implementation statuses 24 `partial`, 8 `reference-implemented`; 179 evidence references; 79 unresolved findings; zero errors. The hash-bound manifest was regenerated.
- `/usr/bin/python3 tools/validate_plan.py` => `validate_plan: OK  stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- `jq empty` passed for `sources/disc-003-evidence.json`, `sources/disc-003-reconciliation.json`, `sources/disc-003-reconciliation.manifest.json`, and `workspaces/DISC-003/source-map.json`.
- `git diff --check` passed.
- DISC-003 remains **IN PROGRESS / NOT ACCEPTED** with reconciliation status `in-progress-not-release-evidence`; zero queued candidates and local validation do not claim verifier/controller completion or release evidence.

## Post-reconciliation full-backlog exhaustion audit

After the final client reconciliation and the independent INT-008 wire-fidelity correction, the live tree was re-audited from clean `main` HEAD `39887dda97eb7163c8d8725a5a9ecfddc00a9234` (`39887dd`, `fix(contracts): reject null event optionals`). This audit reconciles the canonical `ralph.json`, current task cards/worklogs, local git history, `FEATURES.md`, `sources/behavior-surface-rules.json`, the final 32-family DISC-003 evidence/reconciliation ledger, and the pinned upstream source/caller/test/spec evidence. It does not change controller acceptance.

The current Ralph status totals remain 219 stories: 133 `accepted`, 30 `in-progress`, and 56 `not-started`. Subtracting local implementation and recorded blockers produces an exact, disjoint exhaustion partition:

| Bucket | Count | Disposition |
| --- | ---: | --- |
| Ralph/controller accepted | 133 | Closed for local product ownership; do not reopen through broad source families. |
| Ralph stale but locally `IMPLEMENTED` | 21 | Local product work exists; verifier/controller acceptance remains external. |
| Explicit do-not-reopen blockers | 9 | Keep blocked unless materially new pinned source evidence resolves the recorded ownership/authority gap. |
| Dependency-constrained UI/client work | 22 | Excluded from the current native lane pending approved target architecture/dependencies. |
| Residual unresolved/decomposition-blocked | 34 | Mandatory scope remains, but no exact one-to-one source-grounded native product boundary is currently safe for strict TDD. |

The five buckets cover all 219 story IDs exactly once (`133 + 21 + 9 + 22 + 34 = 219`); no ID is missing or duplicated.

### Ralph-stale local implementations (21)

The following rows are still `in-progress` or `not-started` in Ralph but their local task cards explicitly say `IMPLEMENTED` (some with the candidate/verifier caveat) and the implementation history/worklogs were reconciled before subtracting them from the open-product queue:

`AUTO-003`, `AUTO-007`, `EXT-003`, `EXT-007`, `EXT-013`, `INT-004`, `INT-008`, `OPS-010`, `PROV-014`, `REL-004`, `ROUTE-001`, `ROUTE-002`, `ROUTE-003`, `ROUTE-004`, `ROUTE-005`, `ROUTE-007`, `ROUTE-011`, `ROUTE-012`, `SESS-019`, `SESS-020`, `TOOL-015`.

INT-008 remains one row in this bucket after its current/legacy projection (`b496878`), location-shape fidelity correction (`4e779be`), and present-null rejection correction (`39887dd`). Those local GREEN corrections do not authorize editing its Ralph/controller state.

### Explicit do-not-reopen blockers (9)

- `AUTO-004`, `AUTO-005`, `AUTO-006`: the live rows remain generic/TBD and have no task cards or source-to-task decomposition. Existing automation work does not prove one-to-one semantics for these three IDs, so strict TDD would require inventing ownership.
- `EXT-008`: pinned plugin-hook evidence is concrete (`packages/plugin/src/index.ts:266-281`; callers in `packages/opencode/src/session/prompt.ts:307-311,389-393`), but the legacy pre/post hook contract mutates structured arguments/results and propagates failures while the accepted native security HookBus owns a different decision/observation contract. `OC-PLUGIN-INTERNAL` is pinned to `packages/core/src/plugin/internal.ts` blob `d4ab71cb6b04d3c696b58bc865245b746cec5741`, while the V2 hook design remains planned rather than a resolved EXT-008 ownership boundary. A native bridge would currently invent mutation/error semantics or re-own accepted SEC behavior.
- `INT-002`: reviewed `opencode.repository-operations` binds pure reference parsing together with Git/filesystem/network/lock-backed cache materialization. `OC-REPOSITORY-RUNTIME` is `packages/core/src/repository.ts` blob `8ee5be600e3a4284092cb525dfa401a31dbb2075`; `OC-REPOSITORY-CACHE` is `packages/core/src/repository-cache.ts` blob `cab2f631ffb84c7df07dfe95dd6a25d8396b1d6d`. A parser-only slice would underclaim the story, while the full cache contract requires side-effect authority not assigned to INT-002.
- `OPS-007`: `opencode.effect-runtime` and `opencode.repository-operations` both nominate this ID. `OC-EFFECT-LAYER-NODE` (`packages/core/src/effect/layer-node.ts`, blob `9dbc3d51607be375675cc11bddcf181b17a58ad7`) proves a real dependency-graph/runtime contract, but the family also overlaps already accepted BASE/DB behavior and BASE-008's local effect-runtime ownership. No task card resolves what residual OPS-007 alone owns.
- `OPS-009`: `opencode.provider-recording` and `opencode.repository-operations` both nominate this ID. `OC-HTTP-RECORDER-API` (`packages/http-recorder/src/index.ts`, blob `eac4b75b4bba70f021246c8cc5646683e3a84953`) participates in deterministic replay, cassette persistence, live transport, environment mode, redaction/secret policy, and WebSocket limitations. A pure in-memory replay nucleus exists, but assigning only that subset to OPS-009 would invent a decomposition; the broader contract also lacks target resource caps/side-effect authority.
- `ROUTE-006`: the reviewed 9router network-proxy evidence remains broader than a safe target route slice. `NR-OUTBOUND-PROXY` (`src/lib/network/outboundProxy.js`, blob `f85c63a2b8d4228f0f9b8281ad7e4519e38b8243`) and `NR-CONNECTION-PROXY` (`src/lib/network/connectionProxy.js`, blob `9ecd25355122c745809b510db18aacec007354c2`) mix proxy selection, environment mutation, pool/relay behavior, and unresolved MITM/platform authority. The recorded blocker remains materially unchanged.
- `ROUTE-008`: the same ID is shared across routing plus dashboard/status/settings reference surfaces. Those records expose secret-filtered provider/model state and mutable settings, but no one-to-one native ownership separates routing status from dashboard/UI/config/security side effects. The user-requested blocker therefore stays closed.

### Dependency-constrained UI/client rows (22)

`UI-001` through `UI-018` plus `WEB-001`, `WEB-002`, `WEB-003`, and `WEB-005` remain outside the current implementation lane. The final `opencode.app-client`, `opencode.clients-ui`, and `opencode.desktop-client` records explicitly retain browser/router/SDK/terminal, TUI renderer/process/plugin/runtime, Electron/sidecar/IPC/WSL, platform, network/auth, and dependency-architecture gaps. UI-014..UI-018 have target cards, but they are still TUI product surfaces and the user explicitly excluded dependency-constrained UI/TUI work until the native architecture/dependency boundary is approved. Zero queued DISC families is not authority to bypass those constraints.

### Residual unresolved/decomposition-blocked rows (34)

- Extensibility: `EXT-001`, `EXT-002`, `EXT-004`, `EXT-005`, `EXT-006`, `EXT-009`, `EXT-010`, `EXT-011`, `EXT-012`. None has a task card. The reviewed `opencode.extensibility` family still groups external JS/TS plugin hosting, hook surfaces, skills, slash commands, dynamic registration, reload/watch, and TUI compatibility. The pinned family source includes `OC-PLUGIN-INTERNAL` above; no source-to-feature split makes one residual EXT ID an exact native owner.
- Integrations: `INT-001`, `INT-003`, `INT-005`, `INT-006`, `INT-007`, `INT-009`, `INT-010`. None has a task card. `OC-CLIENT-CONTRACT` (`packages/client/src/contract.ts`, blob `413fea9dc338b345d40e1dc4b4cf7a962eba1376`) and `OC-INTEGRATION-HTTP-HANDLER` (`packages/server/src/handlers/integration.ts`, blob `6c29d58776078ad54c436cd91c73daba3feab495`) prove shared client/server contract identity and thin integration handlers, while the reviewed family still spans workspace/project/Git/PTY, generated SDKs, provider-specific methods, remote transports, and external auth. No one-to-one residual INT ownership follows from family membership.
- Operations: `OPS-001`, `OPS-002`, `OPS-003`, `OPS-004`, `OPS-005`, `OPS-006`, `OPS-008`. None has a task card. Configuration evidence is mixed across `OC-CONFIG-RUNTIME` (`packages/core/src/config.ts`, blob `c76486968b0784d57dab102e2d51a7e18f57b7f5`), `OC-STATE-RUNTIME` (`packages/core/src/state.ts`, blob `ab3457fc18143c10d830979dbde3a9375e194ccf`), and `OC-LOCATION-SERVICES` (`packages/core/src/location-services.ts`, blob `7da67673c31982abafc856f34fb52a2c82891525`), while repository-operations additionally mixes Git/cache/observability/install/container behavior. Accepted BASE configuration/lifecycle semantics must remain closed, and OPS-004/008 also overlap repository-operations, so numeric uniqueness or path membership is not sufficient ownership proof.
- Release assurance: `REL-001`, `REL-002`, `REL-003`. None has a task card or behavior-surface rule mapping. Their remaining role is release/feature-accounting, strict TDD/independent verification, and safety/resource assurance rather than a source-grounded native product contract. DISC-003's current `in-progress-not-release-evidence` state cannot satisfy those release obligations by itself.
- Routing: `ROUTE-009`, `ROUTE-010`. Neither has a task card. ROUTE-009 is shared across 9router account-storage, routing, dashboard API, and dashboard settings; ROUTE-010 appears only in the broad routing family, but singleton membership does not define its behavior. The reference ledger still requires task-level decomposition before either can own a native product slice.
- Sharing: `SHARE-001`, `SHARE-002`, `SHARE-003`, `SHARE-004`, `SHARE-005`. None has a task card. The one broad `opencode.sharing` family mixes `OC-SHARE-SQL` (`packages/core/src/share/sql.ts`, blob `a7a08d0c025436a1266cd642be984742925cd298`), `OC-ENTERPRISE-SHARE` (`packages/enterprise/src/core/share.ts`, blob `781bcd5cbeb86000cb76a50a247f4666b0b72f43`), `OC-ENTERPRISE-SHARE-ROUTE` (`packages/enterprise/src/routes/api/[...path].ts`, blob `4677d68d33c48c6a030ae66100d59823fc241aa9`), and `OC-SHARE-NEXT-CALLER` (`packages/opencode/src/share/share-next.ts`, blob `60112e10d964b3d9693727501e1e40dac6b1fdf1`). The record explicitly requires separation of deterministic merge/secret validation, DB persistence, legacy migration, remote HTTP, org/account auth, EventV2 subscriber lifetime, deletion/retry semantics, and bounded queue/backpressure before implementation.
- Hosted web/remote: `WEB-004`. There is no task card. It overlaps reviewed `opencode.server-control-plane` and `opencode.enterprise-remote`; `OC-FUNCTION-REMOTE` is `packages/function/src/api.ts` blob `e57a567dca24a9dfa2283551f1267a0768e4ff57` and combines hosted share routes, Durable Object/WebSocket state, object storage, support relay, GitHub OIDC/PAT exchange, and installation lookup. Credentials/network/platform/deployment behavior is not authorized, and the family does not isolate a one-to-one WEB-004 native boundary.

### Exhaustion result and acceptance semantics

No materially new pinned source evidence on HEAD `39887dd` resolves the nine explicit blockers or assigns any of the 34 residual rows an exact one-to-one native product contract. After subtracting the 133 controller-accepted rows, 21 locally implemented stale-controller rows, and 22 dependency-constrained UI/client rows, there is therefore **no additional source-grounded native task currently safe for independent strict RED→GREEN TDD** without inventing semantics or requiring a hidden JS runtime, credentials, user-DB mutation, unapproved dependencies, or unauthorized network/filesystem/platform effects.

This is an exhaustion/blocker result, not project completion. Mandatory scope remains unresolved. DISC-003 stays **IN PROGRESS / NOT ACCEPTED**, all 32 reviewed records remain `partial`, the manifest remains `in-progress-not-release-evidence`, and zero queued surface families remains an inventory milestone only. `ralph.json` acceptance stays unchanged; verifier/controller acceptance remains external.
