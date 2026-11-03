# Agent worker practices — expert delivery roles and assignments

Current catalog for `AGENT_STRATEGY_V2.md`. `AGENTS.md` controls authorized routes,
upstream selection, permissions and product acceptance. Roles are task
responsibilities, NOT provider/model names, installed OpenCode agents, permissions
or reserved worker quotas. Select an authorized harness route separately. An
expert title without concrete ownership and acceptance is not an assignment.

## Implementation owners (default allocation)

Every implementation dispatch MUST select a role ID below and name the expert's
concrete feature ownership. Generic "researcher" or "coder" without domain and
delivery responsibility is insufficient. One expert may cover adjacent domains
for a complete feature; role boundaries must not manufacture handoffs.

| Role ID | Expert responsibility | Must deliver through the real path |
| --- | --- | --- |
| `native-interaction` | Rust TUI input, cursor, focus, paste, completion | Decoder -> owner routing -> editor -> submit; modal and Unicode cases |
| `session-navigation` | Session tabs, Home, close/reopen, drafts and persistence | Mouse/keyboard -> binding -> state save/restore -> restart |
| `terminal-rendering` | OpenTUI bridge, geometry, styled cells, Markdown and scrolling | Production frame/cell output, resize/clipping and pointer hit regions |
| `session-runtime` | Async daemon/session engine, turns, cancellation, child lifecycle | Real request/event -> execution -> cancellation/settlement -> replay |
| `provider-auth` | Provider setup, auth, models, request and error semantics | Persisted configuration -> actual outbound request -> stream/error |
| `storage-durability` | Transactions, schema, concurrency, typed history | Real storage API -> atomic writes -> restart/replay; disposable databases |
| `tools-permissions` | Tool execution, permission routing, process/filesystem lifetimes | Denial without effects, authorized execution, cleanup and continuation |
| `web-client` | Browser interaction against the shared daemon | Actual browser -> API/event stream -> approval/history/reload |
| `platform-release` | Native dependencies, packaging, install and terminal lifecycle | Exact artifact -> disposable install -> launch/restore on target platform |
| `delivery-runtime` | Existing scheduler, leases, resource admission, retention | Real controller transitions plus crash/concurrency boundary tests |

An owner may span adjacent domains to finish one behavior. Do not split into
one agent per file or artificially require a role handoff at every layer.
Assign only currently approved product scope; this catalog does not activate
deferred web/mobile/platform work or grant security-sensitive edits by itself.

## Supporting delivery roles (on demand)

| Role ID | Responsibility | Boundary |
| --- | --- | --- |
| `integration-repair` | Compose reviewed candidates; resolve actual conflicts; fix current-base wiring | Disjoint private scope; new code requires fresh review; guarded publisher alone updates canonical |
| `scenario-engineer` | Real end-to-end and differential fixtures, deterministic providers, negative controls | May add granted tests; cannot weaken protected acceptance; use real production entry points |
| `adversarial-reviewer` | Find concrete bugs, missing wiring and semantic drift in exact candidate | Read-only, independent of author; reproducible findings, no self-approval |
| `security-evaluator` | Auth, permissions, process/FS/secret boundaries | Independent approval required on sensitive change; no invented authority |
| `test-contract-evaluator` | Check assertion intent, fixture repair and disputed test/reference mismatch | Independent of repair author; explicit protected-policy process still applies |
| `blocker-specialist` | Resolve one missing-reference, architectural or runtime question | Bounded question/stop condition; research-only exception, not standing staffing |

Reviewers do not silently fix the candidate they approve. If a reviewer becomes
the author of a repair, another independent reviewer must evaluate those bytes.
An integration worker that changes behavior is an author, not its own evaluator.

## Mandatory dispatch template

```text
Role: <catalog role> — expert in <specific subsystem>
Goal: <one end-to-end behavior or coherent subsystem responsibility>
Success/failure: <observable journeys, negative cases, exact required checks>
Base: <SHA; absolute canonical and full feature-workspace paths>
Workspace: <controller-provisioned checkout/worktree; branch/ref owner; lease>
Git authority: <explicit worker commit grant OR controller exact-byte checkpoint>
Reference: <selected upstream SHA, absolute paths and entry symbols>
Production chain: <input/API -> state -> effect -> output; existing Rust callers>
Ownership: <checkpoint write paths, shared-file seam owner, read-only paths>
Checkpoints: <complete mergeable behavior slices; submit early; no main assembly>
Dependencies: <interface/accepted revision; unsupported stacks are not assumed>
Resources: <permitted author tests, build-service/lock owner, time/output limits>
Artifacts: <unique private path; patch, raw failure/check receipts, short notes>
Review: <independent reviewer count/security role; author fixes, never self-accepts>
Stop/escalate: <precise missing authority or blocker; next independent work>
Read docs/AGENT_WORKER_PRACTICES.md and applicable scoped rules.
```

Implementation instruction: "Read the pinned source directly and implement the
behavior in the existing Rust application. Include production wiring and tests.
Do not return only a research report or plan unless a named blocker prevents
implementation. Work in the full supplied source tree; do not return copied
files, .json.part code fragments or instructions for main to wire the feature.
Deliver a candidate revision (or exact changes for controller checkpointing),
test evidence and unresolved issues. Keep ownership through review, failed tests
and merge-conflict repairs. Do not
delegate children unless explicitly granted; do not broaden paths yourself."

Reviewer instruction: "Inspect diff and real callers before the author's
explanation. Return APPROVE / NEEDS_FIX with trigger, observable impact and exact
source evidence. Distinguish required corrections from optional cleanup. Never
infer PASS from a claim, compilation or zero tests. Use the controller's exact
output envelope when dispatched by it."

## Compact checkpoint handoff

Task/checkpoint ID; base/candidate SHA and patch hash; changed paths; production
caller evidence; executed checks/counts/artifact hashes; unresolved findings;
next checkpoint; lease/workspace retention status. Use INCOMPLETE or UNEXECUTED
honestly. Keep one concise subsystem source mapping, not repeated research essays.
The assignment may be large; each acceptance claim must remain narrowly proven.
Metadata JSON may accompany a candidate; it never replaces the implementation.
If there is no usable full workspace or no grant for a required production seam,
request that specific provisioning/grant correction and continue independent
work. Do not claim completion or shift the missing implementation to main.

## Execution and preservation

- Verify the actual source directory and SHA; similarly named checkouts are not
  interchangeable. Read upstream normalization, implementation and callers.
- Stay within the assigned role and write grants. Do not delegate children or
  take build resources without authorization. Respect current runtime locks.
- Retain ownership through implementation and repairs with fresh fenced leases;
  never mutate submitted candidate bytes while an evaluator reviews them.
- Submit small wired checkpoints promptly and continue independent eligible work
  while reviews/builds wait. No fixed research quota or mandatory research stage.
- Preserve user files, credentials, databases, preferences and daemon processes.
  Use disposable fixtures. Never claim an unexecuted test ran.
- Rendered parity needs actual matched frames; source presence, assertion counts
  and self-checks alone do not prove it.
- On route failure preserve partial work and concise notes for an authorized
  replacement. No invented usage balance or silent unauthorized fallback.
- Handoffs state independent-review status and distinguish proposed, executed,
  integrated and accepted outcomes. Only exact integrated verification establishes
  product acceptance; a worker cannot self-accept its candidate.

## Research exception scratchpad

Research-only assignments need a named blocker, decision to unlock and stop
condition. Ordinary mandatory review/verification does not require an invented
blocker. Keep source mappings concise and reuse validated existing evidence.

```markdown
# <package / unresolved source question>
Updated: <date>; role: <role>; session: <session ID>
Base/candidate SHA: <exact SHA>; upstream SHA: <exact SHA>
Absolute source/artifact paths: <granted private paths>

## Goal and success criteria
## Evidence read (file, symbol/line, revision)
## Findings (confirmed / hypothesis / refuted)
## Decision, rejected alternatives and uncertainties
## Verification actually executed (commands, results, raw logs)
## Artifact hashes, remaining blockers and next steps
```

Update notes before handoff/compaction. Return the path and hash even when
incomplete; preserve useful private notes before any authorized environment
retirement. Do not overwrite another worker's notes or shared scratchpad.
