# Security policy: capability-based permissions and sandbox

This document is the authoritative security policy for the Lean Harness. It
operationalizes PLAN.md ADR-006 (authorization outside the model), sections 7-8,
and the AGENTS.md non-negotiable engineering rules. It is binding on every
worker: safety remains deterministic and capability-based even when ordinary
permissions are set to `*` (PLAN.md section 1).

## 1. Capability-based permissions

- Safety is deterministic and **capability-based** when ordinary permissions are
  set to `*`. A broad wildcard permission must never bypass a human-only grant or
  a mandatory system protection (AGENTS.md).
- Agent-originated operations go through a trusted **policy broker**. Prompts and
  pre/post hooks improve behavior but cannot grant access, disable isolation or
  undo completed damage (ADR-006).
- Explicit human grants carry resource, operation, identity, expiration and
  policy-version bounds. Project config is not a source of human authority
  (ADR-006).
- Use explicit capabilities, scoped cancellation, byte budgets and lazy services
  (AGENTS.md). No unbounded queue, unbounded retained output, detached task
  without an owner, or per-agent OS process for orchestration.

## 2. Secret access

- **No direct secret file access** and no unrestricted inherited environment
  (AGENTS.md). Workers must not log secrets or embed them in transcripts.
- Never grant broad filesystem access and assume a prompt protects `.env`.
  Permission `*` does not make a prompt a real boundary.
- No changes to the user's existing OpenCode database in the implementation loop.
  Use generated test datasets or an explicitly provided read-only copy. Never run
  host-destructive test commands; use disposable restricted fixtures.
- Ensure `POSTGRES_DB` and `POSTGRES_PORT` point at the test database before
  running backend tests; the test global-setup truncates all tables.

## 3. Sandbox requirements

- A regex or prompt cannot be advertised as a sandbox. A real OS isolation
  backend (for example Landlock) is required for generated executables
  (ADR-006, AGENTS.md).
- Any OS backend must **close inherited capabilities** and be tested on the
  actual platform. Do not assume a config file grants isolation.
- Native mode starts no optional JavaScript host (ADR-005). A plugin must declare
  its supported contract version and granted capabilities.
- The implementation must not silently run a hidden JS runtime in native mode.
- Mandatory system protections and human-only grants cannot be overridden by the
  policy broker, the wildcard permission, or a prompt.

## 4. Landlock and capability tests

- Landlock (or any OS backend) must be validated by a test that actually runs on
  the target platform and verifies inherited capabilities are closed. A passing
  unit suite that never exercises the real backend is not evidence of isolation.
- A denied permission must assert **absence of side effects**, not merely an
  error string: a blocked file write leaves no file; a blocked process does not
  start (PLAN.md section 6).
- Scope, verifier configuration, resource limits and reference pins are not
  writable by the implementation sandbox (ADR-007). The trusted controller and
  verifier are assumed trustworthy; the system does not claim to make a
  malicious verifier safe through JSON validation.

## 5. Human-in-the-loop approvals

- Runtime human approvals are product behavior. Test them with **isolated fake
  grant issuers** in disposable fixtures, never the user's real secrets, files
  or `.env` (PLAN.md section 8, START_HERE.md).
- The builder must not ask the live user to approve hundreds of test deletions
  or expose real credentials. Actual OAuth consent, signing identities,
  production accounts and grants cannot be fabricated by subagents.
- Stop when required credentials are unavailable or mandatory human authority is
  required. Classify and record a blocker rather than bypassing authority,
  reducing scope, fabricating credentials or suppressing failures.

## 6. Enforcement and stop semantics

- Never disable a safeguard to keep the loop moving. On failure, preserve a
  minimal reproduction and stop or request the next safe task (AGENTS.md).
- Explicit human grants have expiration and policy-version bounds; a grant cannot
  grant more than the policy allows.
- Every growing resource category has a retention/archival policy or an explicit
  quota behavior; never silently delete precious user history to meet a byte
  target (ADR-004).
