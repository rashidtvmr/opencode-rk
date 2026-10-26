# Agent operating-rules review

Date: 2026-10-26. Role: documentation auditor.
Reviewer session: `ses_e88d69cd4ffeXQ7i0yUzX5nXew`.
Reviewed base: `7b94da364a80b3993698b443185d3a927971433e`.
Upstream selected reference: `450e7b92412c780a7b5a75226bb8d49aaca24e22`.

## Question and verdict

Do `AGENTS.md`, `docs/AGENT_WORKER_PRACTICES.md` and the orchestrator
scratchpad preserve the user's operating instructions? **APPROVE**, with one
minor authority-list clarification. This is a documentation verdict, not product
acceptance or an assertion that all routes are currently working.

## Confirmed coverage

- Exclusive seven-route allowlist and per-route concurrency, token and error rules.
- Continuous 20+ useful concurrent assignments and prompt single-writer integration.
- Role, goal and observable success criteria assigned before shared worker rules.
- Unique research scratchpads and a durable main-orchestrator scratchpad.
- User-selected OpenCode V2 reference overriding the older lock pin.
- Phase-1 priority, preservation requirements and independent verification.

## Clarification and rationale

The authority list still named `sources/upstream.lock.json` without mentioning the
current user-selected pin. The opening override already resolved precedence, but
the orchestrator clarified authority item 3 to use the selected pin first and the
lock only when no user-selected revision is in effect. The lock itself is unchanged.

The user's `xkiro-gpt-6-luna` spelling remains subject to harness identifier
resolution before use; no new alias or fallback authorization was invented.

## Scratchpad preservation and process correction

The worker returned its note at canonical-relative
`.private-tmp/opencode/docs-auditor/scratchpad.md`, violating the assigned
external-private-path rule. The orchestrator read and preserved its exact bytes
without deleting the original:

`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/docs-audit-preserved-6nr0vm87/scratchpad.md`

Actual SHA256: `d4d81f8eb2f1bc0e3d92059cd0943e5c54f0e1135c6d3339b5e8897b38406648`.
The short hash originally reported by the worker is not treated as SHA256 evidence.
Future assignments must use a granted absolute directory outside canonical.

## Verification and next action

Read-only documentation review; no product tests or builds were needed. Keep the
rules and orchestrator scratchpad current as assignments complete and verified
packages integrate. Validate exact source revisions before reusing this review.
