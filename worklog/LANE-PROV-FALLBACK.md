# LANE-PROV-FALLBACK

Claim: LANE-PROV-FALLBACK in-progress, session ses_f423cd0daffezwT8o66LUJl0eU.
Source: fallback.rs:1 stub (`//! Provider fallback module stub.`), exported lib.rs:17 (`pub mod fallback;`). Rev 6092853 (drift from 6b19524).
Observed: ordered-failover surface absent. Nearby: router.rs AccountFallbackCoordinator (account-level record/finish), route_compose.rs compose_route (selection), int_retry.rs plan_retry (attempt caps). None is ordered provider failover runner.
Target boundary: own only fallback.rs. No lib.rs edits (integrator owns exports).
Tests: authored in-file RED-equivalent 5 tests (primary-fail→secondary, first-win stops, all-fail ordered exhaustion, empty/oversized bounds, invalid/duplicate pre-validation no-attempt).
Decisions: pure generic run_ordered_fallback over &[String] + FnMut closure; validation before any attempt; MAX 16 mirrors selection bound; no I/O/clock/globals; std + thiserror only.
Unknowns: none.
