# Delivery operating contract — compact restart reference

Current task sizing, expert roles and staffing are defined in
`AGENT_STRATEGY_V2.md` and `AGENT_WORKER_PRACTICES.md`. Historical audit baselines
below are not current controller telemetry. Preserve exact-SHA acceptance and
existing guarded publication while improving the runtime incrementally.

## Authority and purpose

On 2026-10-28 the user requested a complete audit and preservation of
`/Users/mymac/Projects/.oc2-delivery-dynamic-20261028-78571fd6/source`, instructed
the agent to read its files and follow them strictly, and requested subagent
investigation of the shared conversation:
<https://chatgpt.com/share/e/6ae1f178-6dd0-83ee-807a-591a9be0f06a>.

This document compacts the locally inspected delivery package. It does not
replace `docs/CONTINUOUS_DELIVERY.md`, executable checks, or a faithfully obtained
conversation transcript. The first computer-use capture is preserved under
`docs/worklogs/research/shared-chat-6ae1f178/`: two observed turns, with the final
item of the assistant response missing. The completed follow-up recovered no text
after its empty list marker. Read its README coverage/evidence corrections and
the versioned v2 transcript and instruction summary. Do not invent missing content.

## Mandatory workflow

1. Admit expert-owned end-to-end behaviors with production wiring, small delivery
   checkpoints, exact upstream evidence, path grants, dependencies, trusted argv
   checks and meaningful RED patterns. Source inspection is implementation work.
2. Allocate a single dynamic pool by current eligible demand. Twenty independent
   tasks may use twenty implementation slots; forty slots do not mean forty
   invented tasks. No fixed role quotas or permanent four-lane scheduler.
3. Own a complete feature through delivery with fresh fenced leases. Required
   workflow: a registered persistent-disk Git worktree outside `.git`/temporary/
   RAM-backed storage, retained through repairs, and a mergeable
   revision; no fragment-only handoff requiring main to rewrite/wire it. Current
   detached per-job repositories are a runtime limitation to repair, not proof
   persistent feature workspaces exist. A worker cannot publish or self-accept.
   Missing provisioning is a concrete controller repair, not permission to fall
   back to temp/private fragments. Disposable test fixtures may be temporary;
   required evidence must be retained in the granted durable evidence root.
4. Require independent diff-first, structured exact-candidate reviews. P0/P1 need
   two approvals; sensitive changes need security review. Negative findings block.
5. Run trusted focused checks and meaningful RED probes. Test logs must contain
   positive executed counts; compiler errors alone are not behavioral RED proof.
6. Prepare on current main. A changed/rebased candidate invalidates its approvals
   and focused results; repeat them. Four parallel preparation lanes are authorized,
   not four concurrent canonical Git writers. The current engine's integration
   turn remains serialized; another publication can stale a prepared base.
7. Require full checks, a clean configured canonical branch, approved policy,
   shared publication lock and existing hook/capability. Publish fast-forward only.
8. Run post-merge checks in a fresh checkout of the exact published SHA. Only
   verified ACCEPTED tasks unlock dependents. Installed product acceptance still
   requires exact-artifact Phase-1/native/browser journeys.
9. Refill eligible work continuously when capacity exists. A running foreground
   supervisor is required; chat prose does not establish a running service.

## Preservation and failure rules

- Never delete dirty work to satisfy a clean-source gate. No auto-stash, reset,
  force-push, broad commit or persistent hook disabling.
- Existing policy and semantic acceptance assertions are protected. Owner-reviewed
  policy/test-contract changes are explicit; workers cannot rewrite their judge.
- After bounded failed attempts, require separate read-only diagnosis and a
  materially different approach; do not recursively generate branch families.
- Publication journals distinguish unlanded intent, landed candidate needing
  postchecks, and ambiguous other HEAD. Reconcile; never blindly replay or reset.
- One supervisor per product repository, even if callers name different state
  directories. Reuse the existing state; do not start the old scheduler in parallel.
- Before supervisor launch, satisfy both route and automatic-collector preflight
  in `CONTINUOUS_DELIVERY.md`. `run` without `--publish` still invokes destructive
  collection; the manual reclaim script's report-only rule does not protect it.
- Heavy validation has an authorized ceiling of 3 builds, subject to actual
  memory/CPU/I/O admission. `jobs.py:heavy_lock` still serializes delivery checks
  at this audit. A shared resource scheduler must replace it safely before
  claiming parallel execution; no ad-hoc lock bypass.
- After merged-and-pushed proof, apply the complete ownership/unique-content/
  frozen-ref/race checks in `AGENT_STRATEGY_V2.md` before retirement.
  The current reclaim script is report-only until those missing guards are fixed
  and verified. Never infer remote ancestry solely from stale tracking refs.
- Preserve failed/ambiguous candidates. Retire only proven controller-owned,
  obsolete workspaces after required patches are retained and live ownership ends.
- Do not infer health or quota from a configured route. Respect current exact
  user-authorized routes, availability pauses and the eight-concurrent Vyce cap.
  Usage remains UNKNOWN without actual telemetry.
- Keep controller authority out of agents/tests; scoped permissions are defense
  in depth, not an OS sandbox. Provider credentials stay with the authorized runtime.

## Current audit baseline

- Subsequent user-authorized one-time bootstrap committed exactly the 19 reviewed
  package files at `3fae911a15f34496b81ec7526d7c43d014c8a3fe`. Other dirty work was
  preserved; existing hooks remained enabled. This exception is exhausted and
  does not authorize further manual canonical commits.
- Controller init succeeded after the commit, pinning policy to that SHA.
  State: `/Users/mymac/Projects/opencode-rk/.git/delivery-state`. No supervisor is
  running and no tasks were admitted. Doctor failed on bounded agent inventory;
  dirty canonical remains a publication blocker. Earlier bullets below describe
  the original inspection state, not current tracking/controller initialization.
- Inspected canonical HEAD: `92b216d04e5e71370247105e7cf119dce0b23f1f`.
- All 19 supplied files (3,177 lines) were read completely and compared to this
  repository: byte-identical, but untracked at audit start. No matching file history
  was found with `git log --all -- <package paths>`; copied is not merged.
- The supplied `source/` directory is not itself a Git repository. Its older
  `source-manifest-v2.json` lists a different package snapshot and is not authority
  to overwrite the actual supplied source bytes.
- The recent footer, snapshot and horizontal commits contain modifications and
  additions, no deletions. Earlier patch-tool Delete/Move operations replaced the
  same paths. Prefer ordinary update patches so intent remains transparent.
- Dirty web/auth/docs and untracked user work remain preserved. Product integration
  is paused during this audit. No controller has been started or accepted-task
  state fabricated; canonical delivery-state was absent at inspection.
- Private UI assignments already in flight retain their artifacts and negative
  verdicts; they are not automatically imported as controller-accepted tasks.

## Read order after restart

Read this file, `docs/CONTINUOUS_DELIVERY.md`, the current `AGENTS.md` operating
instructions, `config/delivery-workflow.json`, and the audit/transcript evidence.
Inspect actual repository/state/locks before invoking any controller. Do not use
the older Phase-1 direct-commit override to bypass this latest delivery direction
for new dynamically scheduled product work.
