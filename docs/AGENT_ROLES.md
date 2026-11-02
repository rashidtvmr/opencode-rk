# Expert delivery roles

Current catalog for `IMPLEMENTATION_FIRST_DELIVERY.md`. Roles are task
responsibilities, NOT provider/model names, installed OpenCode agents, permissions
or reserved worker quotas. Select an authorized harness route separately. An
expert title without concrete ownership and acceptance is not an assignment.

## Implementation owners (default allocation)

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
Base: <SHA; absolute canonical and private candidate paths>
Reference: <selected upstream SHA, absolute paths and entry symbols>
Production chain: <input/API -> state -> effect -> output; existing Rust callers>
Ownership: <checkpoint write paths, shared-file seam owner, read-only paths>
Checkpoints: <independently testable slices; submit early; continue independent work>
Dependencies: <interface/accepted revision; unsupported stacks are not assumed>
Resources: <permitted cheap checks, build-service/lock owner, time/output limits>
Artifacts: <unique private path; patch, raw failure/check receipts, short notes>
Review: <independent reviewer count/security role; author fixes, never self-accepts>
Stop/escalate: <precise missing authority or blocker; next independent work>
Read docs/AGENT_WORKER_PRACTICES.md and applicable scoped rules.
```

Implementation instruction: "Read the pinned source directly and implement the
behavior in the existing Rust application. Include production wiring and tests.
Do not return only a research report or plan unless a named blocker prevents
implementation. Keep ownership through review and integration repairs. Do not
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
