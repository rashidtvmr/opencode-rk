# GUARD-EXT-004

## Verdict

Blocked. Audit only. No canonical, product, test, controller, validator, or
FEATURES edits. Candidate HEAD at audit start: `06ed4861f83b08780f86579b8f55fc557f1d38a7`.

## Attributable guard errors

- `rtk sh -c 'python3 tools/validate_repository.py > /tmp/opencode/guard-ext004-validate.log 2>&1; ...'` -> exit 1; `validate_backlog_exhaustion: 51 error(s)` (log line 9). The aggregate accepted/unknown classification includes EXT-004 (log line 10); this is global accounting, not acceptance evidence.
- Direct EXT error: `EXT-004: remaining extensibility gap is stale after Ralph semantics changed` (validator log line 49). The check is implemented at `tools/validate_backlog_exhaustion.py:1851-1908`, especially the in-progress Ralph binding at `:1891-1896`.
- `ralph.json:897-907` says EXT-004 `status: accepted`; `sources/backlog-exhaustion.json:182-202` still classifies it `unresolved-decomposition`, `controllerStatus: in-progress`, with no card/worklog. `sources/extensibility-remaining-ownership-gap.json:38-52` records the same null ownership and no task binding. These records disagree with each other and with the task artifacts.
- `FEATURES.md:48`, `FEATURES.md:617`, and `FEATURES.md:878` already say EXT-004 `accepted`, matching Ralph, but cannot override the stale ownership-gap guard or establish acceptance. Earlier guard evidence explicitly lists EXT-004 among stale ownership gaps: `worklog/GUARD-TRIAGE-5.md:18-21`.

## Task and implementation evidence

- The task card narrows the partition: `tasks/EXT-004.md:11-21` defines a bounded reload/watch and deferred-activation record, not general plugin lifecycle. Its explicit exclusions are at `tasks/EXT-004.md:95-101`; the proposed crate/test paths are absent, as recorded at `tasks/EXT-004.md:88-91`.
- Existing candidate implementation is instead `crates/tools/src/plugin_deferred.rs:1-192`, exported by `crates/tools/src/lib.rs:40`. Existing worklog evidence reports 5/5 candidate tests and no code change: `worklog/EXT-004.md:15-42`. This is candidate evidence only; verifier acceptance remains external.
- The source gap remains broad and unresolved: `sources/extensibility-remaining-ownership-gap.json:24-35` groups EXT-004/006/010/011 under `generic-disc-placeholder` and EXT-009/012 under `req005-v2-plugin-ui`; `:82-110` says the lifecycle and external JS/TS loading have no exact residual owner. `sources/behavior-surface-rules.json:404-431` maps all EXT-001..012 to the same `opencode.extensibility` surface. `worklog/DISC-003.md:457` records the same equivalence classes.

## Duplicate and real-caller gaps

- Duplicate implementation surface: `crates/tools/src/plugin_deferred.rs:1-192` and `crates/tools/src/ext_deferred_lane.rs:1-193` both define `DeferredLog`, `PluginId`, caps, and all methods. `worklog/EXT-DEDUP.md:22-25` claims the lane file was converted to a shim, but the checked-out file remains a full duplicate. The paired frozen test surfaces also exist at `crates/tools/tests/plugin_deferred.rs` and `crates/tools/tests/ext_deferred_lane.rs`.
- No real production caller was found for `DeferredLog`, `request_reload`, `note_external`, or `add_watcher`. The only production wiring is the public module declaration at `crates/tools/src/lib.rs:40`; method definitions are at `crates/tools/src/plugin_deferred.rs:123-162`. Call references are confined to the two test files and the module itself. Thus the boundary is state/test-only, not wired into a live plugin reload or activation path.
- The task card permits an optional disabled path (`tasks/EXT-004.md:92-101`), so this caller gap is not silently reclassified as a product success. Convergence still requires an explicit controller disposition for the missing real caller.

## Atomic controller proposal

Proposal only. Controller/verifier should reconcile one transaction, preserving
all blockers:

1. Keep EXT-004 non-accepted and blocked pending a real caller, verifier rerun,
   crate-path disposition, and duplicate-surface disposition. Do not infer
   acceptance from `FEATURES.md` or `sources/completion/*`.
2. Bind the narrow reload/watch partition explicitly to EXT-004, or remove that
   partition from EXT-004 and assign it to a newly evidenced owner. Do not let
   the generic equivalence group decide ownership by row number.
3. Atomically update the controller status, task binding, backlog-exhaustion
   classification, extensibility ownership-gap record, and FEATURES mirror.
   Update the validator contract in the same controller-authorized change if
   task/worklog presence is now legal for a blocked owned row; otherwise leave
   the guard red and preserve the exact error above.
4. Before any accepted flip, select one canonical deferred module, remove the
   duplicate through the integrator, add a genuine runtime caller through the
   owning integration lane, and rerun the frozen EXT-004 tests plus independent
   verifier on the integrated revision.

## Commands and results

- `rtk sh -c 'python3 tools/validate_repository.py > /tmp/opencode/guard-ext004-validate.log 2>&1; ...'` -> exit 1, 51 errors. No product/test changes.
- No Cargo, workspace, browser, database, or host-destructive commands run.
- No test edits. No acceptance claimed. Ledger status set to `blocked` with the
  exact blocker summary after this scratchpad was written.

## Remaining blockers

Controller ownership/source reconciliation, real production caller, canonical
module selection, duplicate cleanup, and independent verifier acceptance remain
open. `ralph.json`, `FEATURES.md`, validator/config, source ledgers, product code,
and tests were intentionally untouched by this lane.
