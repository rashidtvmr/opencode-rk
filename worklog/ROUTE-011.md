# ROUTE-011 worklog

## Claim

Implement the remaining source-grounded REQ-022 multi-account routing phase: a bounded pure coordinator for account retry/exclusion and terminal exhaustion. Keep actual execution, lock persistence, refresh, and HTTP mapping outside the router.

## Source evidence

- Candidate base revision: `b76d612` (`feat(providers): bridge plugin integration metadata`).
- Pinned 9router commit: `17c4cc76877bd1755030a8414f8d0083f48dcccf`.
- Primary loop: `src/sse/handlers/chat.js:228-338`.
- Repeated loop callers: `src/sse/handlers/embeddings.js:92-164`, `src/sse/handlers/stt.js:55-87`, `src/sse/handlers/fetch.js:157-231`.
- Cancellation/direct-return evidence: `src/app/api/v1beta/models/[...path]/route.js:246-364`; `tests/unit/gemini-native-endpoint.test.js:151-236`.
- Exhaustion/direct-error evidence: `tests/unit/embeddings.cloud.test.js:432-468`.
- Ownership breadcrumbs: `tasks/ROUTE-002.md:18-19`, `worklog/ROUTE-002.md:44-47`, `worklog/ROUTE-003.md:44-46`; `REQ-022`; reviewed `9router.routing` surface.

## Target boundary

- Independent RED tests: one new provider integration test target only.
- Product candidate: additive pure routing state in `crates/providers/src/router.rs` only.
- No storage/network/runtime/dependency changes.
- Controller/verifier acceptance and `ralph.json` remain untouched.

## Contract decisions before RED

- ROUTE-011 consumes caller-provided fallback eligibility rather than re-owning generic error classification; ROUTE-003 already consumes the same classification for lock planning.
- Retry state contains only bounded excluded account ids and one bounded last fallback failure. It does not retain provider responses, credentials, bodies, or an attempt transcript.
- Initial ordinary unavailability, rate-limited unavailability, post-fallback exhaustion, success, cancellation, and direct non-fallback failure remain distinct typed outcomes so endpoint callers can preserve their own HTTP/status conventions.
- Account attempt capacity reuses `MAX_ACCOUNT_SELECTION_CANDIDATES`; duplicate exclusion is a typed invariant violation instead of silently growing/retrying.
- Caller cancellation is terminal/non-mutating, matching the pinned Gemini-native test that does not mark a credential unavailable on client abort.

## Tests

- Independent test file: `crates/providers/tests/account_fallback_execution.rs`.
- Frozen SHA-256: `b69aca11a52967651753fd8d9b1e0406393389b31b60d0531f84d58f504ff472`; it was rechecked unchanged after valid RED and after implementation.
- The independent author's first and only authoring run failed with `E0432` because the proposed ROUTE-011 public symbols did not yet exist. No behavioral test executed, so that run is authoring/compile feedback, **not RED evidence**.
- Prime then added only the public enum/struct/constant/method signatures with inert behavior. `cargo test -p opencode-rk-providers --test account_fallback_execution` compiled and executed all five cases, establishing valid behavioral RED: 0 passed / 5 failed.
- Post-implementation focused GREEN: the same target => 5 passed / 0 failed.
- Full provider regression: `cargo test -p opencode-rk-providers` passed provider library 45/45 and every current provider integration target, including ROUTE-001/002/003/004/005/007/011/012, INT-004, EXT-007, PROV-014, REL-004 and config tests.
- `/usr/bin/python3 tools/validate_plan.py` => `validate_plan: OK  stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- `git diff --check` passed and the frozen SHA was rechecked exactly.
- `tools/lane_gate.py` is explicitly the storage format-2 wave gate and has no providers/routing lane, so it is not applicable to ROUTE-011.
- Direct `rustfmt --check crates/providers/src/router.rs` still reports formatting drift in pre-existing ROUTE-001/002/012 sections of the shared file. The new ROUTE-011 block was kept rustfmt-shaped manually; no unrelated previously committed routing code or frozen test was reformatted in this task.

## Implementation

- Added `AccountFallbackCoordinator` as bounded caller-owned routing state in `crates/providers/src/router.rs` with typed attempt outcomes, routing decisions, terminal exhaustion states, and invariant errors.
- Account ids are rejected when empty or above `MAX_ACCOUNT_ID_BYTES = 128`; retry history is capped at existing `MAX_ACCOUNT_SELECTION_CANDIDATES = 16`, so the coordinator cannot retain an unbounded exclusion set.
- Fallback-eligible failures append exactly one distinct account id, preserve exclusion insertion order, and retain only the latest failure. Retained failure text is truncated at existing `rate_limit::MAX_FAILURE_REASON_CHARS = 100`; direct non-fallback error text is returned immediately and is not retained or rewritten.
- Success and caller cancellation return terminal decisions without adding an exclusion. A non-fallback failure likewise returns directly without changing previously retained fallback state.
- Final ordinary unavailability is `NoCredentials` before any fallback and `Exhausted { last_failure }` after fallback; rate-limited exhaustion always preserves the caller-provided retry timestamp plus optional bounded last failure.
- Duplicate fallback account ids and attempt overflow are checked before mutation and return typed errors, leaving exclusions/last failure unchanged.
- No DB/storage, secret access, token refresh, provider execution, network, env, clock, background work, HTTP response mapping, or new dependency was added.

## Remaining unknowns

- Combo-model runtime rotation/fallback remains separate from this account coordinator and is not claimed here.
- Token refresh, network/proxy routing, provider-specific quota probes, persistence and endpoint response formatting remain separate owners.
- Verifier/controller acceptance remains external and is not claimed here.
