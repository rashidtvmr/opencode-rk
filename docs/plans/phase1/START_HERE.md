# Phase 1: native TUI + local web release

**DRAFT FOR REVIEW. No implementation is started by this pack. No product is certified.**

Prepared: September 21, 2026. Audited main: `767a86a84376a69b2814ea959bb2c94831783768`. Separate, unmerged native candidate: `6f3737f10853d1ba08f396efa1be0e4a33616748` (PR #1).

## Rewritten task

Deliver an installable local coding application whose default terminal interface is rendered by the Rust/OpenTUI bridge and whose web interface is embedded in the same authenticated Rust/Tokio daemon. Both clients must use the same provider, agent, permission, tool, workspace, session, event and persistence services. Account for the pinned OpenCode pages and behaviors and the existing local web requirements. Finish through six dependency-gated waves, reusing existing code, with independently tested observable outcomes rather than accepted flags, static screens or isolated state machines.

Do not rebuild the project. First repair the task truth, native build and execution spine. Keep all original IDs and full-project requirements traceable. Proposed Phase 1 exclusions require explicit approval and do not close the later full-release backlog.

## Open these in order

1. `PHASE1_RELEASE_PLAN.md`: scope, priorities, six gates and feasibility conditions.
2. `audit/CURRENT_STATUS.md`: exact recorded counts, current code findings and limitations.
3. `scope-decisions.json`: unresolved human/integration decisions.
4. `PAGE_FEATURE_MATRIX.md` and `REQUIREMENT_MATRIX.md`: feature and requirement ownership.
5. `phase1-tasks.json`, `waves/`, and `tasks/`: machine-readable board and 60 detailed packages.
6. `prompts/START_WAVE_1.md`: the handoff to a non-Pro orchestrator after approval.

The board contains **60 work packages, 263 single-file role assignments and 6 gates**. Assignments include independent tests/reviews and repeated integrations into shared files; they are not 263 distinct new product features. A wave contains several bounded dispatch/verification rounds, not one prompt per model.

## Validate this draft without touching the application

```sh
python3 tools/validate_plan.py
python3 tools/render_lane.py --id P1-W1-04-L01
```

These validate/print the PLAN ONLY. Commands written on task cards under `tests/phase1/` are future application-test contracts, not existing or already-passing tests.

## Refresh the audit before activation

```sh
python3 tools/refresh_inventory.py --repo /absolute/path/to/opencode-rk --output /absolute/path/to/phase1-audit-refresh.json
```

This reads source/task records and writes a separate inventory. It never changes canonical ledgers. Diff its IDs and statuses against `audit/recorded-status-inventory.json`; classify newly discovered work before dispatch. Do not overwrite a dirty checkout, change the pre-existing provider edit or reclaim an active worker without evidence.

## Activation is a separate action

Keep these files separate from the canonical controller until the scope, source pins, platform matrix, test authority and execution paths have been approved. If copied into the repository, use a new documentation branch or isolated worktree; do not replace `ralph.json`, `prd.json`, `claims.json`, frozen tests or security policy. Register new IDs through the repository's authorized integration process and run its existing guards. Structural validation of this draft does not waive those guards.
