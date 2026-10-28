# Continuous, dynamically owned software delivery

`tools/continuous_delivery.py` is the executable supervisor. It replaces the
previous unconnected allocation function + integration pump for new work.
It does not use the legacy `ralph.json` driver or assign four permanent coding
lanes. A task is owned through delivery; each next stage receives a fresh fenced
worker lease. Ordinary work is admitted from an owner-controlled task manifest.

## What it runs

```
admit task -> independent implementation workspace -> independent review
          -> focused tests -> prepare on current main
          -> (new SHA? review and test again)
          -> full suite -> one fast-forward publication -> fresh post-merge suite
          -> ACCEPTED -> unlock dependents -> immediately fill available slots
```

A single worker pool is allocated from **current eligible tasks**, not reserved
role counts. With 20 independent implementation tasks and no downstream work,
all 20 slots may implement. As candidates arrive, reviewers, test runners,
conflict repair and integration preparation take slots. Raising `workers` to 40
permits 40 eligible jobs; it does not create tasks or override a route's real
concurrency/authorization limit. Heavy validation stays serialized on this Mac;
20 agents do not authorize 20 Cargo builds. One dynamic integration turn is held
while a candidate is reprepared/reviewed/tested/published, avoiding an endless
rebase storm after each individual merge. Other independent work continues.

## Installation and first run

Python 3.9+ and Git are sufficient for the controller. The included production
adapter uses the installed OpenCode V2 `run --agent ... --format json` interface.
A separate, explicitly configured command adapter is available for another
trusted runtime; it is **not** an automatic fallback on provider failure.

```sh
cd /Users/mymac/Projects/opencode-rk-recovery-20261005
python3 tools/continuous_delivery.py init --repo "$PWD" \
  --config config/delivery-workflow.json
```

`init` prints the exact state directory. The default is
`<git-common-dir>/delivery-state`, outside tracked source. Reuse this directory
for all commands; do not create a second controller for the same queue.

```sh
STATE="$(git rev-parse --path-format=absolute --git-common-dir)/delivery-state"
python3 tools/continuous_delivery.py doctor --state "$STATE"
python3 tools/continuous_delivery.py import --state "$STATE" /absolute/path/tasks.json
# Implement/review/test without canonical publication:
python3 tools/continuous_delivery.py run --state "$STATE"
# Explicitly authorize publication and continue accepting newly added tasks:
python3 tools/continuous_delivery.py run --state "$STATE" --publish --watch \
  --tasks-file /absolute/path/tasks.json
python3 tools/continuous_delivery.py status --state "$STATE"
```

`run` is a foreground supervisor. A terminal or OS service must actually keep it
running. Chat instructions are not a daemon and no background service is silently
installed. Interrupting the supervisor revokes only its own job leases; wrappers
stop their own child groups. On restart, expired jobs and publication journals
are reconciled rather than guessed from prose. Exit 0 means admitted tasks were
accepted, 2 means blocked/drained, 3 means a caller-specified time budget ended.
`--watch` keeps waiting for new work rather than declaring the product finished.

## Task contract and TDD

`config/delivery-tasks.example.json` is an **example**, not an automatically
imported backlog. Replace its paths, goal and checks with real behavior. Every
specification requires:

* Stable ID, observable goal, evidence/source reference, exact path grants and
  dependencies. A manifest update may add new tasks but cannot silently change
  an admitted task's contract. Dependency cycles and missing parents are errors.
* Trusted `checks` containing argv arrays, timeouts and optional required-output
  patterns. Agent prose cannot supply executable commands or mark tests passed.
  Known Cargo/unittest/pytest runners must show a positive executed test count.
* Optional `red_checks`: explicit baseline probes with precise expected failure
  patterns. They must fail on the base revision and pass the normal GREEN
  obligations on the candidate. A random compiler error is not RED evidence.

The owner/planning orchestrator creates meaningful behavioral tasks and may keep
adding them to the watched manifest. This is not an agent inventing unbounded
new features, and branch inventory is never imported as a task queue. The same
pipeline handles additions and fixes; independent checks determine completion.

Existing semantic acceptance tests are frozen by configured prefixes. New tests
can be included in a task grant, but an ordinary implementation worker cannot
weaken existing assertions. Policy/test-contract changes need a separate explicit
owner review and configuration change, not a worker editing its own judge.

## Evaluator-optimizer behavior

The author and reviewer are different job identities and sessions. Review is
against the real candidate diff, without inheriting the author's justification.
Reviewers return one structured `APPROVE`, `NEEDS_FIX` or `REJECT` verdict with
findings; tool output is not accepted as an approval. P0/P1 require at least two
independent approvals. Sensitive paths require a security evaluator. A revised
or rebased SHA invalidates its former approvals and focused test result.

A negative review or failed check returns concrete evidence to the same task's
optimizer. After the configured failed-attempt limit, a separate read-only
worker must diagnose the root cause and propose a new approach. Repeated failure
then blocks the task instead of spawning another branch family. Route outages
get bounded retries and backoff on explicitly authorized alternatives only.
Reported token usage is UNKNOWN unless actually supplied; no invented balance.

## Git safety and failure handling

Each job uses a separate detached Git repository under the controller-owned
state directory. It does not add a branch or worktree to the product repository.
Successful obsolete workspaces are retired after audit patches are retained.
Failed/ambiguous partial candidates remain available for investigation.

The trusted publisher requires a clean canonical worktree on the configured
branch, a current prepared base, exact-candidate independent reviews, current
policy guards and a passing full suite. It uses the shared integration lock and
an ff-only update. The optional existing Recovery Mode write token is passed
**only** to the trusted Git publication command, never to an agent or test.
No Git hook is disabled. No auto-stash, reset, force push or broad commit occurs.

Before changing Git, a durable publication intent is written. After a crash:

| Actual canonical state | Recovery |
| --- | --- |
| Still the old SHA | Retry the unlanded publication after validation. |
| Exactly the candidate SHA | Run missing postchecks; do not merge again. |
| Another SHA | Block for explicit reconciliation; do not infer acceptance. |

Post-merge checks run in a **fresh checkout of the published SHA**, not the
pre-merge test workspace. A failure stops unrelated new implementation and
publication and sends repair to the owning task. The system records the last
verified SHA; it does not blindly reset a published commit or manufacture PASS.
Cancellation during publication is rejected until its journal is reconciled.

## Existing repository constraints

The current repository has pre-existing uncommitted product work and legacy
Recovery Guard differences. The new source gate deliberately replaces the old
fixed-lane/worktree-census scheduling guard for this new queue; it does not
rewrite old recovery evidence or pretend the old guard passed. It checks clean
canonical state, branch identity, ancestry of verified commits, and protected
policy/acceptance-test changes against an owner-approved SHA. The existing Git
reference hook is still enforced. Historical branch counts are not new work.

After explicitly reviewing a legitimate owner-authored policy/test-contract
change, a clean, idle queue can record a new pin with:

```sh
python3 tools/continuous_delivery.py approve-policy --state "$STATE" \
  --reason "Describe the independently reviewed policy/test-contract change"
```

This command is not available to scoped model workers. It cannot bypass an
unreconciled publication or running job. Ordinary product workers can never
rewrite their own evaluator. Do not delete dirty work to make the guard green.

The new queue is authoritative for **new dynamically scheduled work**. The old
four-lane controller/state remains historical and is not run concurrently as a
second scheduler. Its product acceptance contract and security protections are
not weakened. `tools/recovery_delivery.py` remains a compatibility inspection/
publication bridge for old queue entries, not a second agent launcher.

The initial route configuration includes only the exact authorized alias observed
in the installed catalog (`9router-luna-free`). Inventory presence does not prove
service health or 40-way provider capacity. The selected agent definition is read
from the explicit `runtime.agent_config` JSON path and copied without provider
secrets into the temporary scoped overlay. Bounded inventory retries require
an observed complete route and exact effective permission tail. `doctor` reports missing aliases and
source cleanliness; the test fixture is not evidence that a live provider works.
Update configured routes only to exact user-authorized aliases verified on this
host. Standalone OpenCode sessions are optional via `runtime.standalone`; they
are not enabled automatically because many separate servers add host overhead.

## Verification and limits

```sh
python3 -m unittest discover -s tests/delivery -p 'test_*.py' -v
```

Tests run against disposable repositories and an explicitly named deterministic
subprocess agent fixture. They exercise real claims, subprocesses, Git rebases,
ff publication, review rejection, scope checks, dirty-work preservation, guarded
integration and crash journals. They do not claim to be 20 live model calls.

This is fault-handling software, not a mathematical guarantee of failure-proof
execution. Path grants and OpenCode permissions are defense in depth, **not an
OS sandbox**. A malicious test or a child that deliberately escapes its process
group requires stronger container/VM isolation. Tests receive a credential-
filtered environment and disposable HOME, but trusted build commands must still
be reviewed. Hardware, provider availability, flaky tests and human edits remain
external constraints.

An accepted task means its declared checks and configured full/post suite passed
on its recorded integrated SHA. It does **not** certify the application's entire
Phase 1 release. Installed binaries, native-library receipts, real terminal and
browser journeys still need the existing exact-artifact `phase1_judge.py` gates.
