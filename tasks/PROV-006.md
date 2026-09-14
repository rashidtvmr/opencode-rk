# PROV-006

Status: IN PROGRESS. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-042.
Dependencies: none.
Test obligations: PROV-006-T01, PROV-006-T02, PROV-006-T03, PROV-006-T04, PROV-006-T05.

## User-observable outcome

Provider-boundary retry policy with exponential backoff and jitter. Transparent
retries for transient failures (5xx, timeouts, connection resets, 429 rate limits)
while avoiding retries on permanent client errors (4xx except 429). Backoff delays
grow exponentially between attempts and are capped at a configurable maximum. A
randomized jitter prevents thundering-herd effects when many clients retry
simultaneously.

## Source evidence

- Retry pattern expected by upstream provider clients (see crates/providers/src
  lib.rs: pub mod retry).
- Exponential backoff with full jitter is the standard approach (AWS Architecture
  Blog: "Exponential Backoff and Jitter").

## Observable contract

- RetryPolicy: max_attempts u32, base_delay_ms u64, max_delay_ms u64, multiplier f64.
- RetryState: attempt u32, last_error String, next_delay_ms u64.
- RetryHandler: new(policy) -> Self; run<F, R>(operation: F) -> Result<R, String>
  where F: FnMut() -> Result<R, String>. Retries with backoff; stops on Ok or max
  attempts.
- compute_delay(attempt: u32) -> u64: exponential backoff with jitter.
- should_retry(error: &str, attempt: u32) -> bool: do not retry 4xx (except 429).
- Five named tests cover: immediate_success, retries_then_succeeds,
  max_attempts_exhausted, delay_increases, dont_retry_client_error.

## Resource bounds

- No blocking I/O in retry path; uses tokio::time::sleep for async backoff.
- No unbounded queues; state is a single struct on the stack.
- No detached tasks; run() is fully awaited.
- Jitter via xorshift PRNG (thread-local), no external dependency on rand.
- max_delay_ms caps every backoff; attempts capped by max_attempts.
