---
name: Security Broker
description: Enforce capability-broker checks before tool use, deny-unless-authorized
---
## Rules
- Default deny-unless-authorized; `*` never bypasses human-only or mandatory protection.
- Broker check per tool call: explicit capability (actor, resource, operation, expiry, policy-version).
- Scope capabilities narrowly; byte budgets, scoped cancellation, lazy services; no unbounded queue/output.
- No shell-string concatenation; no direct secret file access; no broad fs grant for `.env`.
- Never log secrets or embed in transcripts.
- Denied must assert absence of side-effects: no file created, no proc started.
- Human approvals use fake grant issuers in disposable fixtures only; never real secrets/files/`.env`.
- Stop on missing human authority; record blocker, never fabricate grant or disable safeguard.
- OS backend (e.g. Landlock) must close inherited caps, tested on platform; prompt/regex is not sandbox.
