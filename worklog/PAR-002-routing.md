# PAR-002-routing worklog

Claim: app-level routing types in owned file only.
Source: HEAD 5af7884. `router.rs:53,88,232-243` (MAX_ACCOUNT_SELECTION_CANDIDATES=16, MAX_ACCOUNT_ID_BYTES=128, caller-owned `now`). `rate_limit.rs:6` (MAX_FAILURE_REASON_CHARS=100). `budget.rs:152-174` (validate-before-mutate consume). Parity card: `tasks/completion/parity.json:5` quota/rate-limit/failover preserve budgets + redacted export.
Target: `crates/providers/src/app_routing.rs` only. No other edits. `#![forbid(unsafe_code)]`, std only.
Tests: 7 in-file (`rustc --edition 2021 --test ... -o /tmp/opencode/pr && /tmp/opencode/pr`): id validation, quota-vs-rate classifier, quota preserves budget, holder bounds/unknown, failover safe, export redacted, reason truncation.
Decisions: AccountId validates pre-alloc; FailureKind quota-wins incl. 402; holder mirrors budget.rs failed-consume-no-mutate, MAX_ACCOUNTS=16; decide_failover total, never self/excluded; Debug shows reason len only, export scrubs denylist.
Unknowns: none in slice. Wiring into lib.rs left to integrator (out of lease).
