# Subagent assignments and durable research

`AGENTS.md` is authoritative for the exclusive route allowlist, budgets, parallelism,
upstream reference, integration writer and product acceptance rules. This file
supplies best practices after the orchestrator has assigned a concrete role.

## Assignment template

The orchestrator fills in every applicable field before dispatch:

```text
Role: researcher / implementer / reviewer / auditor / verifier / integration-prep
Goal: one coherent observable behavior or evidence question
Success criteria: concrete deliverables and assertions; what constitutes failure
Base: exact SHA and absolute canonical/candidate source directory
Reference: exact upstream SHA, absolute checkout and relevant requirement
Scope: read-only paths; explicit, non-overlapping write grants
Artifacts: unique absolute private directory outside canonical
Scratchpad: unique absolute research-note path; existing notes to consult first
Dependencies: inputs required; independent work possible while waiting
Resources: allowed commands, heavy-validation ownership, output/time limits
Evidence: exact diff/source hashes, citations, commands, raw logs and limitations
Handoff: verdict, artifact/scratchpad paths and hashes, unresolved issues, next step
Then read docs/AGENT_WORKER_PRACTICES.md and applicable repository instructions.
```

## Execution

- Verify the actual source directory and SHA before drawing conclusions. A second
  checkout with a similar name is not interchangeable with the assigned candidate.
- Stay in the assigned role and path grant. Do not delegate children or take the
  heavy validation slot unless explicitly authorized. Integration preparation is
  not permission to advance `main-v2`; the single writer lands reviewed packages.
- Read upstream implementation, normalization and callers before proposing parity
  fixes. Identically named fields need not have identical accounting semantics.
- Preserve user files, credentials, databases and daemon processes. Use disposable
  fixtures and bounded resources. Never claim a test ran when it was only read.
- Research findings need source citations. Rendered parity needs actual frames;
  source presence, assertion counts and self-checks alone do not prove it.
- On a route error/quota failure, preserve the partial result and scratchpad so
  the orchestrator can continue on another allowed route under `AGENTS.md`.

## Research scratchpad template

```markdown
# <package / research question>
Updated: <date>; role: <role>; session: <session ID>
Base/candidate SHA: <exact SHA>; upstream SHA: <exact SHA>
Absolute source and artifact directories: <paths>

## Goal and success criteria
## Evidence read (file, symbol/line, revision)
## Findings (confirmed / hypothesis / refuted)
## Decisions and rationale
## Alternatives rejected and why
## Verification actually executed (commands, results, raw log paths)
## Artifacts and hashes
## Remaining uncertainties, blockers and next steps
```

Update it during research and before a handoff or compaction. Reuse validated
existing evidence instead of repeating the same investigation. Return the actual
scratchpad path and hash, including when the task is incomplete. Private temporary
notes must be preserved by the orchestrator before their environment is retired.

## Handoff

State the package/gate, base and candidate hashes, changed paths, source evidence,
actual verification results, independent-review status and unresolved behavior.
Return `INCOMPLETE`, `NEEDS_FIX` or `UNEXECUTED` where appropriate. A worker cannot
self-accept its candidate; only exact integrated release verification establishes
product acceptance.
