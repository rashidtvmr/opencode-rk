# APP-012-TOOL-RED

## Claim
- Task/session: `APP-012-TOOL-RED` / `ses_f2ec3a38fffer2ivp1cowx22WB`.
- Owned path: `crates/server/tests/app012_tool_journey_red.rs`.
- Scope: real authenticated HTTP/router/storage/provider loop RED; no product edits.
- Claimed before file edits via `tools/completion_claims.py`.

## Source evidence
- Pending source inspection. Required inputs: `PLAN.md`, `docs/TDD.md`, `docs/SECURITY.md`, `docs/CONVERGENCE.md`, `tasks/completion/local.json`, `worklog/APP-012.md`, server wiring, turn stream, tools executor/file ops, security broker policies.

## Observed scenario
- To establish: disposable workspace + disposable DB; loopback scripted provider round 1 requests bounded `read`; HTTP router advertises enabled tool; broker authorizes concrete path; real bounded file op; durable tool result; round 2 exact result; final stream; restart/rebuild; outside/secret denial persists and feeds back.

## Target boundary
- Test must fail for current missing wiring (`ToolExecutor` unknown `read` and/or broker bypass), not compile/harness failure.
- Bounded provider rounds/captures, loopback only, no host/user files/secrets, no shell, no mock executor, serial test.

## Tests
- Pending authoring and focused RED run.

## Decisions
- Product source and existing/frozen tests remain untouched.
- Capture source line evidence, SHA-256, exact command/failure, then mark ledger `blocked` with RED-ready-for-implementation.

## Remaining unknowns
- Public test harness APIs and exact route/provider/storage setup.
