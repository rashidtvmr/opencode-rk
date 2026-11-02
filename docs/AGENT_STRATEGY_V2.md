# Convergence V2 agent strategy — implementation-first delivery

Status: current operating instructions, 2026-11-02. Requested by the owner after
repeated review found unwired features, excessive research handoffs and growing
integration/storage queues. This is a policy update, not proof of new controller
capabilities. Read with `AGENTS.md`, `AGENT_WORKER_PRACTICES.md` and the runtime status below.

## 1. Authority and scope

For task sizing, staffing, handoffs and orchestration, this policy supersedes
conflicting historical prose in strategy, convergence, recovery and worklogs.
Current explicit user instructions remain higher authority. Preserve product
scope, selected upstream pin, semantic acceptance assertions, reviewer
independence, security permissions and exact-integrated-SHA acceptance.
Protected runtime policy is changed only through its authorized mechanism; prose
does not unlock a gate, approve a candidate, change a running configuration or
authorize a second scheduler. Do not rewrite historical evidence as current fact.

## 2. Large ownership; small complete deliveries

Assign a persistent expert owner to a coherent subsystem or end-to-end behavior,
not one researcher, one helper author and a later wiring task. The owner reads
the pinned source, implements, proposes meaningful tests, fixes review findings
and follows integration failures through verification. Ownership survives stage
and session changes; it does not grant permanent access or approval authority.

The assignment names the production input -> state -> effect -> rendered/output
path. Checkpoints must include actual callers, module declarations, configuration
and persistence where relevant. Uncalled helpers and model-only ports are
explicitly INCOMPLETE, even when their tests pass.

Split a subsystem into independently verifiable checkpoints, not a giant late
patch. Submit a checkpoint when one observable behavior is ready. Continue
independent work while review/build is pending; do not wait for a wave to finish.
Keep later edits isolated from the immutable submitted candidate. If the runtime
cannot track a dependent stack, only work on independent checkpoints; do not
silently bypass its ACCEPTED dependency gate.

Write grants cover the files needed for the checkpoint and remain non-overlapping.
A large responsibility is not a lease on an entire crate indefinitely. Narrow or
release grants at checkpoints. A shared-file seam has one active owner; peers
provide interface requirements or scoped patches. Prefer a small reviewed seam
extraction only when it removes an observed collision, not an architectural rewrite.

## 3. Upstream-guided porting, not research-first delivery

Use OpenCode `450e7b92412c780a7b5a75226bb8d49aaca24e22` at the selected reference
checkout. Preserve existing approved extensions. Prefer structure-preserving
translation: retain predicates, event ordering, state transitions, defaults and
observable behavior. Adapt ownership, async lifetimes, reactive effects and FFI
to Rust explicitly; syntactically similar code is not automatically equivalent.

Source inspection is part of implementation. Reuse a concise per-subsystem
mapping of upstream symbols to Rust production callers, validated against current
bytes. Do not repeatedly commission summaries of the same upstream files.
Independent research is allowed only for a named unresolved question blocking
implementation (missing reference, contradictory behavior, platform constraint or
unexplained failure). State the decision it unlocks, a stop condition and a bounded
attempt. Return a decision/patchable finding, not an open-ended report.

## 4. Implementation-heavy continuous scheduling

Start with roughly 70% of useful agent assignments as implementation owners when
independent implementation is eligible. This is an allocation preference, not a
quota or a reason to starve review. At 20 useful jobs, an example is 14 owners,
3 reviewers, 2 integration/repair workers and 1 scenario engineer. Automated test
processes are resource jobs, not additional research agents.

Refill on completion events. Do not wait for a batch; do not invent work to fill
30/48/96 slots. If ready-candidate age grows, move capacity to the actual queue
bottleneck. If implementations are waiting on tests, improve test throughput
rather than creating more reports. No standing research/audit allocation.
Dispatch acknowledgments are not measured concurrency. Respect actual provider
availability, authorized routes and harness limits. For this policy-update task
the user selected `9router-luna-free` and `9router-river-glm-5-3-flash`; these are
routes, not expert roles, and this sentence does not rewrite the product fleet.

## 5. Independent evaluator -> optimizer loop

Author owns implementation and repairs; independent evaluator owns verdicts.
Use one review for ordinary P2/P3 checkpoints, two for P0/P1, with a security
evaluator for sensitive changes as required by the existing contract. Independent
reviews can run concurrently. Do not add audit-of-audit stages without conflicting
evidence or a concrete incident. A negative verdict is not outvoted.

Review the diff and production callers first. Findings must name a trigger,
incorrect behavior and source evidence; distinguish blocker, uncertainty and
optional cleanup. Check for missing wiring, state loss, cross-session confusion,
unbounded work and test changes that erase the requirement. After a revision,
review the changed candidate and unresolved findings; reuse source understanding,
not an approval attached to a different SHA. Existing exact-SHA invalidation still
applies until dependency-aware reuse is implemented and independently verified.

After the configured failed-attempt limit, assign one independent diagnosis with
a different approach. No recursive review/custody branch family. Correct repeated
mistakes in the relevant rule/check and apply the correction to affected candidates.

## 6. Parallel preparation; atomic publication

Four integration-preparation lanes are authorized, each with disjoint candidate
ownership. They are not four concurrent writers to one canonical worktree/index.
All final `main-v2` updates share a publication lock, expected-base check and
fast-forward-only guarded transaction. An unrelated publication can still stale
a candidate. Reprepare and reverify rather than force-push or claim disjoint paths
eliminate Git ancestry races. Do not hold the final ref-update lock while agents
research or review. Current broader integration-turn serialization remains a
runtime limitation, not permission to evade the controller.

"Lane" must specify: a recovery branch, an integration-preparation worker, or a
legacy scheduler slot. Their numeric settings are not interchangeable. Use one
active scheduler and its actual state, not simultaneous recovery/completion/delivery
controllers. New actor topology requires implemented ownership and recovery checks.

## 7. Resource-aware builds and evidence

Configured maxima (currently up to 3 local builds, 3 Cargo jobs and 4 test threads
per build) are authorization ceilings, not evidence that the workload fits.
Admission must account for linker peak memory, test subprocesses, CPU, available
disk and active jobs. Do not consume the reserved-memory allowance or treat 512
MiB free as enough to start an arbitrary Rust build. Queue the expensive job when
capacity is unavailable; continue useful non-conflicting work and fix the constraint.

Never run concurrent builds against the same mutable CARGO_TARGET_DIR. A build
service may deduplicate identical requests and distribute immutable binaries to
isolated test fixtures once implemented. Until then obey the existing locks.
No worker bypasses a lock to reach a numeric concurrency target. Low disk is an
incident to resolve, not authorization for unsafe deletion or writing until ENOSPC.

Run focused checks, affected regressions and required full/post-merge/installed
checks. Evidence includes exact source/artifact identity, test count, exit status
and failure output. No PASS from compilation, zero tests, snapshots of OC2 alone
or source presence. Differential checks compare pinned upstream and Rust under
matched state/theme/viewport/input; validate the judge with negative controls.

## 8. Workspace lifecycle without losing work

Desired topology: bounded reusable execution slots, read-only reviewer snapshots,
separate build caches and compact recoverable evidence. This is NOT permission to
reuse a dirty directory or delete an unmerged candidate. Existing per-job detached
repositories remain governed by the current controller until slot pooling exists.

Before reclaiming a workspace/branch, individually prove:
1. explicit controller ownership, no active lease/process/task/dependent candidate;
2. exact current tip is merged into local `main-v2`;
3. that tip is advertised by the actual push remote OR is an ancestor of a freshly
   advertised remote `main-v2` whose commit graph is available and verified;
4. no staged, unstaged, untracked or unique ignored work, nested repository, user
   backup, active build artifact or unknown content is discarded;
5. it is not canonical, a configured remote path or a frozen legacy ref/worktree;
6. required source/evidence is retained, and all identities/ownership/cleanliness
   are rechecked immediately before exact-path removal under lifecycle coordination.

`ls-remote` advertises ref tips, not every reachable commit. A stale local remote
tracking ref is not push evidence. No broad prune, force removal, reset or stash.
If proof is missing, retain the workspace and assign concrete proof/repair work.
Report bytes actually reclaimed after successful removals, not the planned total.
Reproducible caches may have a separately authorized retention policy; do not
confuse cache classification with authority to delete evidence.

At the initial audit, `tools/worktree_reclaim.py` lacks the complete active-lease,
ignored-content, frozen-ref and race recheck guards above. Use its report/dry-run
only; unattended `--apply` is not authorized by this policy until these gaps are
fixed and verified. A failed candidate may be archived to save space only under
an explicit disposition that preserves every unique byte and proves restoration;
an archive alone does not waive the merged-and-pushed deletion rule.

There is also a separate automatic collector: `tools/delivery/engine.py` calls
`cleanup.collect()` every supervisor loop, including `run` without `--publish`.
`tools/delivery/cleanup.py` retains a commit diff for some superseded candidates,
then removes directories without the required merged-and-pushed or dirty/ignored-
content checks. Restricting the manual reclaim script does not control this path.
Supervisor launch against retained work requires a verified retention-safe
collector or a supported, verified non-destructive mode; none is established by
this policy. Fix this specific runtime hazard before launch, without turning it
into a workspace-pooling project or stopping independent isolated implementation.

## 9. Runtime capability ledger (inspect before relying on it)

Initial source audit: `ae20a31c` plus the subsequent protected-cap clarification.
This table is a point-in-time observation, not a permanent limit or fake rollout.

| Capability | Actual status / evidence | Next independent delivery slice |
| --- | --- | --- |
| Continuous dynamic dispatch | `tools/delivery/engine.py:dispatch`, eligible demand | Keep; measure queue age and useful outputs |
| Multiple integration preparation turns | NOT implemented: `eligible_tasks` gates prepare/publish/postcheck on one `integration_turn` | Parallel prepare with stale-base repair; retain atomic publish |
| Three concurrent heavy delivery checks | Config says 3; `jobs.py:heavy_lock` still takes one exclusive `heavy-validation.lock` | Shared resource admission across all lock consumers; concurrency/crash tests |
| Dependent candidate stacks | NOT implemented: dependencies require accepted state | Parent-version invalidation and stack tests before enabling |
| Review reuse across changed SHAs | NOT implemented; exact-candidate checks remain | Dependency/content-bound review reuse, separately evaluated |
| Reusable worker slots | NOT implemented: per-job detached repositories | Lease-safe capture/restore/recycle with failure recovery |
| Automatic safe workspace reclamation | `engine.py` unconditionally calls `cleanup.collect`, even without publication; commit-diff archival omits merged/pushed and dirty/ignored proofs. Manual reclaim-script restriction does not disable it | Verified retention-safe collector or supported non-destructive mode before supervisor launch; guard/ancestry/race tests |
| Distributed builds / cache deduplication | Proposed, not established by numeric configs | Isolated build workers and reproducible artifact keys |
| OS-enforced worker write grants | Do not infer from prompts or permissions | Demonstrated boundary tests before claiming isolation |

Do not start these all as a new framework project. Keep the existing controller;
implement the smallest observed throughput repair alongside product work. A change
to stored controller config must use the supported idle/reconciliation policy
path; editing a JSON file does not reconfigure an already running supervisor.

## 10. Outcome metrics and first pilot

Measure accepted user journeys/day, time from ready to integrated, review rework,
build queue time, escaped regressions, workspace count/bytes and useful completed
assignments. Record absent telemetry as UNKNOWN. Commit count, tokens, lines and
worker count are activity, not completion percentage. Freeze a requirement
denominator before publishing a percentage.

Pilot independent end-to-end owners on New/Home -> existing tab, paste-at-cursor
-> subsequent typing, and subagent row -> child navigation. Reproduce against the
current tree first; do not reimplement a behavior already fixed. Coordinate shared
`tui_entry.rs` seams explicitly. Release acceptance uses the actual installed
artifact and disposable data. Expand useful concurrency as integration/verification
throughput supports it; no whole-wave wait and no correctness waiver.

## Research informing the policy

- Bun, https://bun.com/blog/bun-in-rust : large subsystem work, few worktrees,
  parallel implement/review loops; not proof this repository has that runtime.
- Anthropic, https://www.anthropic.com/research/building-c-compiler : independent
  failures and a known-good oracle make parallelism productive.
- Anthropic, https://claude.com/resources/articles/ai-code-migration : translation
  rules, adversarial checks and amortized builds. File existence is only a
  translation-stage marker, never product acceptance here.
- OpenAI, https://openai.com/index/solving-math-word-problems/ and
  https://openai.com/index/formal-math/ : independent verification and machine
  feedback; these are principles, not a drop-in software scheduler.
- https://factory.strongdm.ai/ : scenario-driven independent acceptance. Keep
  deterministic protocol/security checks; an LLM satisfaction score cannot waive them.

## Product gates and preserved convergence rules

`main-v2` is the canonical product line. Merge into `main` only after the required
release gates pass on one exact SHA, without force-push. Preserve frozen legacy
refs and unique user work. The active Phase-1 boundary is defined by
`PHASE1_ACCEPTANCE.md`; this wider gate inventory does not activate deferred scope.

| Gate | Acceptance |
| --- | --- |
| G0 Preservation | Preserve unique/dirty work; exhaustive legacy disposition does not block delivery |
| G1 Startup | Fresh HOME launches oc2, creates/reuses one authenticated daemon, rejects foreign listeners |
| G2 Provider | Persisted/in-app configuration reaches the actual outbound request and first streamed response |
| G3 Tool | Provider call -> real permission broker -> execution -> typed result -> continuation |
| G4 Durability | Second turn, exit/restart with identical typed history, second-client attachment without duplication |
| G5 Native TUI | Real PTY/OpenTUI, raw input, streaming redraw, resize/suspend/resume and terminal restoration |
| G6 Web | Real browser uses the same daemon/session engine, including tool and reload/resume |
| G7 Ubuntu | Release archive installs with native dependencies and passes the golden journey |
| G8 Release | Required platforms pass on one exact release SHA with security/adversarial checks |

A checkpoint identifies its gate/failure, base SHA, exact write paths, semantic
test manifest, upstream evidence, primary owner and independent evaluator. At
most two competing implementation candidates per failure are allowed under the
current configuration; competition requires genuine uncertainty, not duplicate work.
Reprepare stale candidates, rerun required checks and publish atomically; rerun
the product gate on the integrated SHA before acceptance. If publication state is
ambiguous after a crash, inspect the journal/ancestry rather than blindly retrying.

Ask the user only for genuine product ambiguity, intentional deviation or missing
external authority. Routine Rust/interface/fixture choices follow the selected
reference. Mechanical test maintenance belongs to an independent test owner;
semantic contract supersession requires the applicable independent authority and
preserved evidence. No implementing worker weakens its judge to pass.

### Behavior-scoped salvage

1. Name the active failing behavior and missing implementation/test evidence.
2. Inspect relevant already-preserved revisions read-only and record identities.
3. Selectively port useful code/tests into the current behavior checkpoint.
4. Verify current-base production wiring and the same behavioral acceptance.
5. Record disposition only for evidence examined for that task.

Do not create archive/custom refs, inventory every branch, or require exhaustive
lineage disposition as an implementation prerequisite. Existing snapshots/refs
remain preserved. New custody/retention operations need explicit scope and all
section 8 lifecycle checks. No ancestry-only heritage merge is required for
Phase 1; repository-wide maintenance is separate from product acceptance.
Task cards, plans, handoff documents and isolated module compilation do not count
as completion. Progress is an integrated product behavior moving from RED to GREEN.
