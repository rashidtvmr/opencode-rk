# GUARD-EXT-012 reconciliation audit

## Scope and status

- Task: `EXT-012`; audit-only guard. Owned files: this worklog plus the
  `EXT-012` row in `tasks/completion/claims.json`.
- Candidate base: `b3e0d01` (`GUARD: integrate wave 1 blocked audits`).
- Branch: `lane/GUARD-EXT-012-20260923`.
- No product, test, controller, source-ledger, Ralph, FEATURES, validator, or
  policy file was edited.
- Final ledger status: `blocked`. This report is a reconciliation proposal,
  not ownership, integration, or acceptance evidence.

## Source and card evidence

- `tasks/EXT-012.md:1-7`: product story, mandatory full-release scope,
  `REQ-005`, five obligations, no dependencies. `:9-20` narrows the intended
  outcome to an inert, caller-owned UI declaration registry and explicitly
  excludes lifecycle, manifest validation, scoped execution, hooks, rendering,
  and client architecture.
- `tasks/EXT-012.md:22-66`: source evidence is the pinned OpenCode
  `95daf90670b7c039c436c85537da5fbfe2205b41` plugin family. The checked-in
  ownership-gap record says the `EXT-009`/`EXT-012` pair is requirement
  identical and that requirement subtraction does not assign ownership.
- `tasks/EXT-012.md:67-144`: local proposed contract is bounded pure memory:
  `MAX_UI_DECLS=64`, label 64 bytes, detail 256 bytes, synchronous caller
  ownership, no I/O, no host, no renderer, no persistence. `:145-172`
  specifies T01-T05.
- `sources/extensibility-remaining-ownership-gap.json:32-59`: `EXT-009` and
  `EXT-012` remain in one `REQ-005` equivalence group; `ownershipDecision` is
  null for both and the source concludes the rows are requirement-identical.
- `sources/extensibility-remaining-ownership-gap.json:80-126`: pinned
  lifecycle, external JS/npm loading, and built-in composition partitions have
  no exact residual owner. `:128-163` retains six design gaps and requires a
  direct source, caller, test, and spec distinction before closure.
- `sources/completion/audits/AUD-012.json:28-67`: the audit classifies the
  native UI boundary as verified only at module level, while native plugin host
  plus approval turn adapter and slash/mention/queue endpoints are missing.
  `:114-119` names `UI-decl to renderer binding` as a mandatory repair child;
  the audit explicitly claims no acceptance.
- `sources/completion/surface-evidence.json:1-7,25-53`: no surface is marked
  implemented; certification is false; dynamic command/config/event/route/tool
  surfaces are unverified; the OpenTUI fork is absent from the lock and
  inventory.
- `ralph.completion.json:23-35`: legacy accepted flags are not release
  evidence, vertical wiring is mandatory, and unmapped or partial behavior
  requires child tasks.

## Attributable guard reconciliation

### Ralph, card, worklog, ledger

| Surface | Current evidence | Finding |
|---|---|---|
| `ralph.json:1015-1029` | `EXT-012` is `accepted`, story is `TBD - see source audit`, `REQ-005`, T01-T05 | Stale generic acceptance projection. It contradicts the card's specific inert-registry contract and the ownership-gap null decision. It is not release evidence. |
| `tasks/EXT-012.md:1-20` | Card says `NOT STARTED`, but defines a complete narrow candidate | Candidate contract exists without controller ownership binding. Card presence itself is a validator-visible reconciliation event. |
| `worklog/EXT-012.md:17-67` | Claims `crates/tools/src/plugin_ui_boundary.rs`, canonical tests, 5/5 GREEN, and no remaining slice unknowns | Implementation/test evidence is not source ownership or integrated acceptance. It also predates this guard lease and says no RED was rerun before lease. |
| `tasks/completion/claims.json:316-320` | This guard row is now `in-progress`, session `ses_f320d7e2dffeAfAjMr7WiTtYZZ` | Guard claim is valid. It must end `blocked`, not `completed`, because this audit cannot establish ownership or acceptance. |
| `FEATURES.md:41-55,211-216,610-625` | `EXT-012` appears accepted in sync rows, REQ-005, and EXT table | Stale accepted mirrors. They must be changed only by controller authority in the same reconciliation transaction as Ralph and the gap ledger. |

### Backlog ledger and validator

- `sources/backlog-exhaustion.json:318-411` projects `EXT-009` through
  `EXT-012` as `unresolved-decomposition`, `controllerStatus: in-progress`,
  `reasonKey: extensibility-family-not-decomposed`, no task card, no worklog,
  and no implementation commits. The projection correctly refuses to treat
  the local module as closed, but now conflicts with the present card/worklog
  artifacts and Ralph/FEATURES accepted mirrors.
- `tools/validate_backlog_exhaustion.py:1851-1908` hard-codes the remaining
  six rows as source-reviewed with null ownership, requires Ralph `in-progress`
  plus the generic/TBD story, and rejects any `tasks/<id>.md` or
  `worklog/<id>.md` appearance. `:1943-1955` separately requires the exhaustion
  projection to remain unresolved with no implementation commits.
- The command `python3 tools/validate_backlog_exhaustion.py` returned 51
  errors. EXT-attributable errors were stale ownership-gap semantics for
  `EXT-004`, `EXT-006`, `EXT-009`, `EXT-010`, `EXT-011`, `EXT-012`, plus the
  accepted/unknown backlog classification and the task/worklog appearance
  errors for `EXT-012`. The remaining errors belong to sibling families or
  global accounting drift.
- `python3 tools/validate_repository.py` returned `FAIL backlog exhaustion
  exit=1`, with the same 51-error set. This guard did not attempt to make the
  validator green by changing protected authority files.
- `python3 tools/convergence_gate.py` returned `CONVERGENCE BLOCKED`,
  `total=80`, including off-plan completed rows and notes admitting no
  acceptance. This is a repository-wide blocker, not evidence that EXT-012 is
  accepted.

## Production caller and wiring audit

### Existing module

- `crates/tools/src/lib.rs:39-49` exposes `pub mod plugin_ui_boundary;`. This
  is compile-time module exposure only.
- `crates/tools/src/plugin_ui_boundary.rs:104-121` constructs an empty,
  caller-owned in-memory registry. `:134-159` validates and records bounded
  declarations. `:161-190` revokes, describes, lists, and always returns
  `UiError::Deferred` from `request_render`.
- `crates/tools/tests/plugin_ui_boundary.rs:1-4` imports the implementation
  with `#[path]`; the frozen tests do not call the crate's public module. The
  mirror `crates/tools/src/ext_ui_boundary_lane.rs` is likewise a duplicate
  test-path artifact, as recorded in `worklog/EXT-009-DEDUP.md:11-18`.

### Missing production caller

- Repository search found no production call to `UiBoundary`, `UiDecl`,
  `request_render`, or `ActivationGate`; all matches are the module, its twin,
  tests, and the unrelated hook gate. No plugin lifecycle callback registers a
  UI declaration, no server route exposes one, and no CLI/TUI renderer reads
  `list` or `describe`.
- `crates/tools/src/plugin_lifecycle.rs:85-183` is a separate in-memory
  plugin registry. Its `add`, `mark_ready`, `remove`, and `wait_ready` methods
  do not own or invoke `UiBoundary`.
- `crates/foundation/src/config.rs:180-223,225-254` defines `Plugins` and
  `JsPluginHost`; all feature flags default off and require an explicit call
  site. No call site constructs `UiBoundary` when `Plugins` is enabled, and no
  JS host is started by this slice.
- `crates/server/src/web_turn_adapter.rs:34-76,123-169` reports
  `plugin_host: false` in `WebTurnAvailability::current`; the capability
  report consequently marks plugins unavailable. This confirms there is no
  web production caller either.
- `crates/sessions/src/reference.rs:18-117` and the provider integration
  bridge have caller-owned plugin-scope metadata registries, but neither is a
  UI declaration transport or renderer path. Reusing either by name would be
  an ownership inference, not evidence.

**Wiring conclusion:** EXT-012 currently proves a standalone bounded helper
and direct path-inclusion tests. It does not prove a production user-visible
journey. The required caller is a future plugin-scope adapter owned by the
integration/client authority, followed by a renderer consumer under UI-012.

## Protocol, permission, and resource gaps

### Protocol gaps

1. No versioned manifest/config field maps a validated plugin contribution to
   `UiDecl`; EXT-005 only validates manifest metadata
   (`tasks/EXT-005.md:68-93`).
2. No protocol binds a declaration to a live EXT-001 plugin scope, removes it
   on lifecycle close, or defines replacement/replay ordering. The local scope
   is an unconstrained `u64`, not a lifecycle capability.
3. No RPC/event/schema route carries `list`/`describe` to a client, and no
   renderer consumer exists. `request_render` is deliberately deferred, so
   the helper cannot satisfy the REQ-005 user-visible UI behavior by itself.
4. No explicit error/version compatibility policy exists for unknown UI
   surfaces, duplicate labels across future clients, renderer disconnect, or
   stale declaration IDs. The local duplicate rule is only
   `(scope, surface, label)`.
5. No installed entrypoint reaches this module. A successful focused test is
   therefore state/module evidence, not caller wiring.

### Permission and host gaps

1. Current code performs no effect, so it cannot currently bypass a broker.
   However, `ActivationGate::open` (`plugin_ui_boundary.rs:64-79`) has no
   capability, identity, expiration, policy-version, or human-grant input.
   If a future caller treats opening as activation, it would violate
   `docs/SECURITY.md:9-22` and PLAN ADR-006 unless the broker owns that decision.
2. No renderer capability is represented in `UiDecl`; no distinction exists
   between metadata declaration and an actionable command/panel/status effect.
   A renderer bridge must be read-only until an explicit capability and client
   ownership contract exists.
3. Native mode correctly starts no hidden JS host in this slice. External JS/npm
   plugin loading remains unowned and blocked by
   `sources/extensibility-remaining-ownership-gap.json:98-110`; do not infer
   EXT-012 ownership of it.

### Resource and lifecycle gaps

1. Count and byte bounds are present (`plugin_ui_boundary.rs:11-16,145-157`),
   and operations are synchronous (`:104-110`). No queue, task, process,
   filesystem, network, secret, or DB resource is retained.
2. `plugin_ui_boundary.rs:155-156` increments `next_id` without an explicit
   overflow check. After enough revoke/redeclare cycles, `u64::MAX` can panic
   in debug or wrap in release, violating monotonic ID semantics. Authority
   must choose a typed ID-exhaustion failure or a stronger lifetime bound before
   production wiring; this guard does not patch product code.
3. `list()` and `describe()` clone bounded data (`:169-184`), but no client
   backpressure, snapshot byte budget, or renderer-disconnect cleanup exists.
   Those bounds belong to the future transport/client owner, not this helper.
4. Scope lifetime is caller convention only. No automatic revoke occurs when a
   plugin is removed, activation fails, a daemon restarts, or a client
   disconnects. EXT-001 lifecycle ownership must be joined before this helper
   can be called production-wired.
5. T05's `render_invocations` counter is a local zero-valued variable and does
   not instrument a real renderer (`plugin_ui_boundary.rs:198-251`). The test
   proves the current helper has no renderer reference, not that an integrated
   process tree, thread set, socket set, or DB stays unchanged.

## Atomic authority proposal

**Proposal ID:** `EXT-012-RECON-01`

**Disposition:** blocked, controller-only review. Do not flip accepted status.

### Required single transaction

The controller should choose one path and update all projections atomically;
partial edits will continue to produce contradictory guard output.

**Recommended path: bind a narrow candidate, not the full REQ-005 behavior.**

1. Add a distinct source-gap partition named, for example,
   `inert-plugin-ui-declaration-registry`, with direct local path evidence for
   `crates/tools/src/plugin_ui_boundary.rs`, direct frozen test evidence, and a
   specification that explicitly distinguishes it from EXT-009 namespacing,
   EXT-001 lifecycle, EXT-005 manifests, and UI-012 rendering. Keep external
   JS/npm loading, lifecycle, transport, renderer, and user-visible behavior
   unresolved.
2. Record the current module and direct tests as candidate implementation
   evidence only. Do not record a release acceptance receipt or claim that
   REQ-005 is complete. Add the missing production caller and renderer as
   explicit repair children owned by the integration/client authorities.
3. Reconcile `ralph.json`, `FEATURES.md`, `sources/backlog-exhaustion.json`,
   `sources/extensibility-remaining-ownership-gap.json`, and the validator's
   expected decomposition in one controller-authorized change. The selected
   state must be internally consistent: either all show a task-specific
   blocked/in-progress candidate, or all retain unresolved decomposition with
   an explicit candidate disposition. Do not leave Ralph/FEATURES `accepted`
   beside a null ownership gap and an unresolved ledger row.
4. Bind `tasks/EXT-012.md` and `worklog/EXT-012.md` in the source-gap and
   backlog records only if the controller accepts the narrow partition as
   genuinely distinct. Otherwise archive/retire the candidate artifacts through
   controller authority and retain `ownershipDecision.EXT-012 == null`.
5. Keep `tasks/completion/claims.json` `EXT-012` blocked until the controller
   has reconciled authority and an independent verifier has checked the exact
   integrated revision. Never change this row to `completed` from the focused
   5/5 helper test.

### Preconditions for any later unblocking

- A real production caller from plugin scope to UI declaration registry.
- Explicit protocol version, scope-close/revoke semantics, bounded snapshot
  transport, stale-ID behavior, and renderer/client owner.
- Permission-broker decision before any activation or actionable UI effect;
  wildcard ordinary permissions cannot bypass human-only/system protection.
- Typed ID exhaustion behavior, client backpressure, disconnect cleanup, and
  restart/replay policy.
- Installed end-to-end evidence from actual entrypoint through broker and
  client presentation, plus independent verification. Focused module GREEN is
  insufficient under `docs/CONVERGENCE.md:10-24,54-68`.

### Forbidden authority shortcuts

- Do not mark `EXT-012` accepted because `FEATURES.md` or `ralph.json` already
  says accepted.
- Do not assign the whole `opencode.extensibility` surface by task number,
  requirement subtraction, local module presence, or duplicate test twins.
- Do not wire the renderer, JS host, lifecycle, hooks, manifest, or permissions
  inside this audit lane.
- Do not weaken `validate_backlog_exhaustion.py`, remove unresolved partitions,
  or suppress convergence errors to obtain GREEN.

## Verification receipt

- `python3 tools/validate_backlog_exhaustion.py` -> FAIL, 51 errors. Expected
  guard RED; EXT-specific stale/null-binding and artifact-appearance findings
  preserved.
- `python3 tools/validate_repository.py` -> FAIL, backlog exhaustion exit 1.
  No protected authority file changed.
- `python3 tools/convergence_gate.py` -> `CONVERGENCE BLOCKED`, `total=80`.
- No Cargo, workspace, browser, database, network, or heavy build command run.
- `git diff --check` -> PASS after this worklog and ledger update.
- No tests authored or edited. Existing EXT-012 5/5 claims were not treated as
  acceptance because this lane audits authority and wiring, not implementation.
