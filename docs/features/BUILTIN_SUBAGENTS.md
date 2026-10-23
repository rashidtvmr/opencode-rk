# Built-in subagents and reliable delegation

Status: partial. Ten-role catalogue and execution are implemented in the working candidate; the documented role regressions pass. Automatic retry for transient provider failures is UNIMPLEMENTED/UNPROVEN. Exact-release installed gates remain pending.
Task: `tasks/NATIVE-SUBAGENTS-001.md`.

## Current observed failure

User reports that subagent delegation currently fails in installed native oc2.
Existing roster/control UI is insufficient evidence of a working child lifecycle.
Capture and repair the actual provider -> task -> child -> tools -> result path.

## Candidate catalogue and verification

The candidate includes ten canonical roles without requiring user configuration:

| Roles | Runtime tool policy |
| --- | --- |
| `explore`, `research`, `planner`, `reviewer`, `plan` | Read-only tool allowlist |
| `general`, `debug`, `implementer`, `tester`, `build` | Ordinary permissioned tool loop |

Configured agents take precedence over built-in roles with the same name.
Aliases resolve to a canonical built-in identity without duplicating catalogue
entries. Read-only restrictions are enforced by the runtime tool policy;
ordinary tool access still passes through the permission broker.

In the native candidate, `/subagents` opens the current session's child roster.
Select a child with arrows, a visible numbered row or the mouse, then open its
detail view to inspect history and status. The visible footer offers **Stop**,
**Retry** and **Follow-up**; corresponding shortcuts are `s`, `r` and `c`.
Follow-up opens a separate composer and requires an explicit send. Escape backs
out of the composer/detail view before closing the roster. These controls have
focused UI regressions; the final installed native journey remains unproven.

Executed candidate regression evidence:

| Scenario | Test / evidence | Result |
| --- | --- | --- |
| All ten roles execute with canonical prompts/models; read-only writes have no effects; full roles use parent-routed write approvals | `all_builtin_roles_execute_with_canonical_identity_and_enforced_read_only` | PASS |
| Catalogue and custom-agent precedence | `builtin_catalogue_lists_all_roles_and_preserves_custom_overrides` | PASS |
| Alias resolution | `alias_subagent_types_resolve_to_canonical_builtin_roles` | PASS |
| Invalid bare model rejected before creating a child | `config_agent_bare_model_fails_before_any_child_session_exists` | PASS |
| Foreground admission-capacity exhaustion and manual retry | `foreground_delegation_with_busy_second_permit_settles_quickly_and_manual_retry_recovers` | PASS |
| Automatic retry of transient provider failures | Required; current automatic retries cover pre-side-effect admission-capacity refusal only (`crates/server/src/task_children.rs`) | UNIMPLEMENTED/UNPROVEN |
| Bounded follow-up queue and generation draining | `active_child_followups_queue_bounded_and_drain_one_per_generation` | PASS |
| Cancellation preserves queued follow-ups without executing them | `cancelled_child_preserves_queued_followups_without_draining` | PASS |
| Child permission lifecycle and restart outcomes | `session_child_approval_lifecycle` (3 tests), `session_child_restart_outcomes` (1 test) | PASS |
| Native installed role controls, retry/follow-up UI and parent approval journey on final release | Exact installed-artifact journey | UNPROVEN |

The seven lifecycle tests are in
`crates/server/tests/subagent_builtin_lifecycle.rs`. Main-run logs and independent
review are recorded in
`docs/bugs/bug_2026-10-22_native_ui_and_config_import.md`. These results establish
the exercised runtime scenarios, not complete coverage of every failure mode or
installed-product acceptance.

## User requirements

Research https://github.com/gsd-build/get-shit-done and built-in agents and
delegation behavior in OpenCode, Codex and Claude Code. Record authoritative
source references and distinguish built-in agents from optional workflows.

Ship a useful, bounded collection of predefined agents out of the box, including
exploration, research, debugging and the other independently useful roles
identified by that research. No user-created config should be required. Preserve
configured custom agents and allow explicit per-agent/custom task prompts.

Required lifecycle and controls:

- Reliable foreground and background delegation through the real session engine.
- Custom initial prompt and additional prompts during an active child session.
- Inspect any child's status, transcript, tools, errors and pending decisions.
- Explicit follow-up prompt UI from every child detail view.
- Bounded automatic retry with exponential backoff for eligible transient provider failures. Not implemented: automatic retry currently applies only to pre-side-effect admission-capacity exhaustion; provider failures are terminal. Preserve this requirement.
- Visible manual retry action/button, retry state, attempts and final error.
- No automatic replay of completed or ambiguous side effects.
- Child HITL/permission requests appear as actionable popups in the parent.
- Parent decisions are correlated to the correct child/run/request, never silently
  approved, and child cancellation/restart cannot reuse a stale approval.
- Stop/cancel, concurrent children, failed setup, unavailable models, provider
  errors, session resume and daemon restart have defined observable outcomes.
- Native controls support keyboard, numeric shortcuts where appropriate and mouse.

## Acceptance and test discipline

Use TDD: meaningful compiling failing tests before repair, then minimal behavior,
regressions, independent review and real installed native journeys on the final
integrated release SHA. Maintain a scenario matrix with exact test names and
PASS/FAIL/UNPROVEN. Every predefined role must be enumerated and tested for
catalogue visibility, prompt/policy mapping and real child execution eligibility.

The user's target is complete scenario coverage. Do not claim “100% covered”
without a measured coverage report and an explicitly defined denominator; do not
substitute mocks/source presence for actual daemon/tool/permission execution.
Retain databases/preferences/history and mandatory capability-broker protections.

Research-derived agent prompts must be original implementation or appropriately
licensed/attributed; do not copy third-party proprietary prompt text.
