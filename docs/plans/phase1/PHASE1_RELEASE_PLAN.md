# Phase 1 release plan: six gated waves

## 1. The finish line

Phase 1 is an installed, local Rust/OpenTUI terminal application plus an embedded web application on one authenticated Rust/Tokio/SQLite backend. A fresh user should launch the binary, complete provider setup, choose a workspace and model, submit a coding task, review the real permission request, execute an allowed fixture edit, inspect the result, open the same session in the other client, and recover the same history after restart. Neither a development web server nor a separate manual daemon command is part of that normal journey.

The native UI remains Rust -> OpenTUI native ABI. Do not replace it with Ratatui, a hidden JavaScript TUI or text snapshots while retaining the native label. Using an appropriate bounded input library does not change renderer ownership; its dependency and platform behavior still require approval and measurement.

This is a proposed Phase 1 certificate, NOT a certificate that every remote, mobile, hosted or compatibility feature in the whole repository is finished.

## 2. Audit baseline and what the numbers mean

The audit used main `767a86a84376a69b2814ea959bb2c94831783768` in an isolated worktree. It separately retained draft PR #1 at `6f3737f10853d1ba08f396efa1be0e4a33616748`. The original working checkout and its uncommitted `crates/providers/src/responses.rs` edit were left intact. The candidate branch has not been treated as merged or validated.

Across the canonical legacy plan, exported plan, additive completion definitions, audit shards, claims and task cards, there are 468 distinct IDs. Every ID appears in the crosswalk. Recorded statuses were inventoried completely; the most important live entrypoints and contradictory completion claims were source-checked. This is not a claim that every one of those 468 behavioral contracts was executed or manually inspected line by line. The 82 TBD titles, 96 generic titles and family-based mappings still need semantic reconciliation in W1.

`ralph.json` says 258/258 accepted. The conventional `prd.json` has 231 entries, 221 not-started, 9 accepted and one in-progress. There are 222 conflicting shared statuses and 27 omitted legacy IDs. The separate claims ledger has 96 completed, 58 in-progress and 7 blocked entries. These are incompatible measurements, not three valid product-completion percentages.

The rewrite preserves useful code and evidence but does not inherit release acceptance from any of those flags. A module that exists may need only wiring and app-level verification; do not automatically rewrite it. A task without a claim is unclaimed, not necessarily untouched.

## 3. Scope and approval boundaries

### Required local product

Include native home/session views; actual keyboard/paste/resize/focus behavior; provider/account/model/variant/effort setup; command palette and registered commands; sessions/workspaces/history/tabs; Markdown/code/diff; approval/question/error/retry flows; settings/themes/help/status/context/memory; parent/child agents; local tools, MCP, LSP, Git and constrained terminals; safe export/import and local sharing; shared durability/replay; and the full approved local web experience.

The web side includes navigation and message actions, structured reasoning summaries/tool activity/references, structured composition, actual attachment transmission and Library, workspace context, agent/tool controls, editable artifacts and safe preview/run/apply. Existing search/deep-research and voice/dictation requirements remain scheduled in W5. An unavailable adapter must not become an invented implementation or a silently excluded requirement.

### Explicit later-phase proposals

Separate hosted account/control-plane deployment, Cloudflare tunnel, multi-PC remote control, native iOS/Android apps and their device acceptance. Separate arbitrary Solid/TypeScript UI-plugin compatibility from native extension support. Also separate a standalone general-purpose workflow builder/autocodegen product and full production twenty-worker orchestration platform from the minimum safe execution process needed to build Phase 1.

Public hosted sharing and full external protocol/spawner compatibility need explicit boundary decisions; local shared API/event behavior cannot be deferred. All deferred old IDs remain open in the full-release crosswalk. These are proposals in `scope-decisions.json`, not approvals made by this document.

### Platform, source and command freeze

The proposed supported matrix is macOS arm64, Linux x86_64 and Windows x86_64. Each declared target needs its actual native library, loader path, terminal acceptance, sandbox evidence and approved distribution/signing posture. No worker may silently reduce that matrix to the platform it can run.

Keep OpenCode canonical baseline `95daf90670b7c039c436c85537da5fbfe2205b41`. Separately classify the previously observed TUI target `45ad8dc38aa1d65480a9961a3ae1b3c95d46796c`, then freeze adoption choices. Keep OpenTUI at `c01292fd0837bafd07ce458c74416b2b375a41ab` unless an independently reviewed compatibility change is necessary. Do not continuously chase upstream dev during implementation.

Resolve `oc2` versus the planned `opencode2` spelling once, preserving required compatibility aliases without overwriting the user's existing `opencode` installation.

## 4. Six waves and their gates

| Wave | Work | Proof required to advance |
|---|---|---|
| 1 | Reconcile task truth; review PR #1; native builds and FFI; deterministic tests; CI; daemon lifecycle; shared contracts; provider and harness preflight. | Approved scoped baseline, runnable native build/ABI smoke, a real test harness, validated external prerequisites and canonical guards. |
| 2 | Connect native input/layout/composer, compatible provider transport, one execution engine, actual approvals/cancel, embedded authenticated web and durable replay. | Installed native -> provider -> authorized file edit -> continuation -> web co-client -> restart/resume; deny and cancel also pass. |
| 3 | Finish native page/navigation/presentation integrations and already-live actions. | Real terminal interactions and state effects for available services; explicit pending service rows, not false feature completion. |
| 4 | Complete subagents, routing, MCP/LSP/Git/PTY, rules/extensions, web navigation/actions/attachments/artifacts. | Complete core TUI and web workflows use real services; every W3 service-pending row is rerun and closed. |
| 5 | Complete required research/voice/graph features; OS/browser security, recovery, resource tests, differential audit and repairs. | All approved local capabilities work; no blocking integration/security defect or unapproved exclusion; feature scope frozen for RC. |
| 6 | Reproducible artifacts, clean install/upgrade/recovery, platform/browser/canaries, independent review and verified docs. | One exact candidate revision and artifact set passes the whole declared matrix; independent scoped decision and human release approval. |

There are ten packages per wave. The detailed cards enumerate precise acceptance and one-file assignments. They are not instructions to give an entire wave or package to one small model.

### Avoid the W3/W4 dependency trap

Agent and terminal screens can be wired before their remaining W4 service integration, but their full feature cannot be marked complete at W3. The board explicitly records `release_completion_dependencies` for affected W3 packages. G3 is a presentation milestone: already-live actions and truthful unavailable/error behavior can be accepted at that stage. Unavailable required scenarios remain pending with zero claimed passes. G4 performs mandatory rechecks. Original TUI/PAR parents and release acceptance stay open until then.

### Do not leave setup to the end

The minimal in-app endpoint/credential/model setup needed for the W2 fresh-install journey must be present in W2. W3 adds richer account/model/variant management; it is not permission to require manual configuration for the W2 demonstration.

## 5. How to execute with non-Pro and heterogeneous models

Use one non-Pro reasoning orchestrator to maintain a small task board and assemble context packets. Use fast coding models for bounded, low-risk implementation tasks. Use stronger non-Pro reasoning models for FFI, concurrency, protocol, persistence and permission work. Assign test authoring and verification to identities distinct from the implementer; model diversity helps challenge assumptions but is not proof of independence by itself.

Resolve exact available model IDs from the connected harness/provider catalog at execution time. No particular model identifier or Pro subscription is assumed. The supplied 9router model is a test endpoint selection, not automatically the required implementation-worker model.

Each worker receives one owned product file, relevant API contracts, source excerpts, a frozen behavioral test and the real caller it must affect. A worker returns a candidate patch and evidence, not parent acceptance. A serialized integrator applies shared root/router/manifest wiring and lands candidates on the integration branch. An independent verifier reruns the same frozen suite and real journey on the exact integrated SHA.

For planning purposes allow up to eight active worker chats on the constrained host, for example four bounded implementers, two integration/review roles and two independent test/verification roles. This is a cap, not a utilization target. If the harness safely supports twenty workers, preserve at least four integration and two independent verification slots before adding breadth. Cloud worker count is separate from local process/build concurrency.

Keep one resource-heavy validation token globally. Use `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2` or lower on pressure, an 8 GiB host ceiling and at least 2 GiB reserve. No concurrent workspace builds or multiple browser runners. Use bounded one-test -> target -> crate -> integrated progression and preserve logs rather than repeatedly dumping entire files or rerunning timeouts.

Each packet should normally include at most eight relevant source excerpts and roughly 14,000 tokens of task-specific context. The existing repository/security/TDD rules remain binding even when a packet is compact. A role that cannot resolve a contract escalates rather than inventing another local state machine.

## 6. Task status and acceptance

Use separate fields for coordination, implementation and proof. A useful state sequence is draft -> ready -> claimed -> candidate-ready -> integration-pending -> integrated-verified -> release-accepted. Blocked is available at every stage. Preserve the original ledger status as historical metadata; never reset it merely to make a new report look consistent.

`candidate-ready` requires a real implementation and focused frozen tests. `integrated-verified` requires a registered real caller, passing parent journey and regressions at the integrated revision. `release-accepted` additionally requires the complete scoped surface/platform evidence. A W3 presentation-stage receipt is deliberately narrower and keeps release acceptance false.

All original IDs retain a destination. Several old IDs may map to one user journey, and a legacy ID may split between Phase 1 and later phases. Exact semantic mappings, especially generic/TBD stories, are verified before those parent features close. No new task is accepted simply because all old records have a nonempty mapping array.

## 7. Testing and real evidence

Tests must exercise the actual application, daemon, storage, permission broker and tools in disposable restricted workspaces. Scripted external provider/MCP/search/audio fixtures are appropriate for deterministic protocol failures; internal mocked success is not a substitute for integration. Live canaries complement, not replace, those fixtures.

Required evidence includes compiling behavioral RED, immutable test hash, candidate and integrated GREEN with actual executed counts, source/artifact/native/web hashes, platform and terminal/browser versions, real caller trace, relevant sanitized recordings, process/resource measurements, and unresolved failures. Zero-test success, allow-refusal assertions, module registration, a static menu label or a claimed sandbox policy are never product proof.

An invalid frozen test needs independent contract adjudication, retained original hash/rationale and a newly approved compiling RED. Implementers do not edit, skip, narrow or regenerate frozen tests to obtain GREEN. W1 owns the known MCP/environment/fixture disputes rather than deferring them until the final full-suite run.

## 8. Provider and safety constraints

Use `http://localhost:20128/v1` on the user's host with literal model `vyce/deepseek-v4-flash` when the catalog and protocol probe confirm support. Do not assume that every compatible endpoint supports the Responses API. Request protocol, account and upstream model must be distinct fields; do not incorrectly strip `vyce/` or reject it because it is not `openai`.

Obtain the already-authorized secret at runtime from the approved local secret store/environment. This pack contains no credential. Freeze small call/token/concurrency/time budgets before live tests and stop on authentication, capability or policy failures. Missing image/tool/audio capability calls for a separately observed authorized model/service, never fabricated output.

Never test against the user's original OpenCode database, `.env`, real projects or unrelated credential stores. A model cannot grant itself filesystem/network/process authority. Keep mandatory-human and destructive controls, cross-project approval, inherited-capability closure and no-automatic-replay safeguards.

## 9. Feasibility and stopping rules

Six waves are a dependency structure, not a fixed amount of elapsed time or six model exchanges. Native platform builds, broken frozen-test authority, actual provider protocols, signing/runners, real OS isolation and research/audio adapters are schedule risks. W1 must resolve or explicitly disposition these prerequisites before calling the six-wave target credible.

Within each wave reserve integration/review/repair capacity rather than filling every slot with new features. One bounded repair loop may fix a known defect; repeated failing assumptions must trigger a contract or scope decision. Do not declare a gate green because a desired number of waves has elapsed.

A five-wave variant is only appropriate if W1 proves enough W5 hardening already implemented to combine W5 and W6 without weakening evidence. The supplied plan targets six. If required work does not fit, change capacity or obtain explicit scope approval; never silently remove local requirements or reduce tests.

## 10. What this drafting pass delivered

It produced a complete ID/status inventory and proposed crosswalk, 60 user-journey packages, 263 single-file role assignments, six gates, a page/feature matrix, requirement dispositions, role prompts, evidence templates and offline plan validators. All product work remains draft/unverified. Structural plan checks are not application tests, and this pack does not modify canonical acceptance state or merge PR #1.
