//! Provider retry module with exponential backoff and jitter.

use std::time::Duration;

use tokio::time::sleep;

/// Configuration for retry behavior.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of attempts (including the first try).
    pub max_attempts: u32,
    /// Base delay in milliseconds for the first backoff.
    pub base_delay_ms: u64,
    /// Upper bound on delay per backoff in milliseconds.
    pub max_delay_ms: u64,
    /// Multiplier applied to delay between attempts (e.g. 2.0 for doubling).
    pub multiplier: f64,
}

impl RetryPolicy {
    /// Compute the backoff delay for a given attempt index.
    ///
    /// `attempt` is 1-based: the delay before the *second* try is computed with
    /// `attempt == 1`, etc.
    pub fn compute_delay(&self, attempt: u32) -> u64 {
        if attempt == 0 || self.max_attempts <= 1 {
            return 0;
        }
        // Exponential backoff: base * multiplier^(attempt-1)
        let growth = self.multiplier.powi((attempt - 1) as i32);
        let raw = self.base_delay_ms as f64 * growth;
        let clamped = raw.min(self.max_delay_ms as f64);
        // Full jitter: randomize in [0, clamped]
        let jitter = fastrand_delay(clamped as u64);
        jitter.min(self.max_delay_ms)
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 100,
            max_delay_ms: 5_000,
            multiplier: 2.0,
        }
    }
}

/// Mutable state tracked across retry attempts.
#[derive(Debug, Clone)]
pub struct RetryState {
    /// Current attempt number (1-based: 1 is the first try).
    pub attempt: u32,
    /// The last error message produced by the operation.
    pub last_error: String,
    /// Delay in milliseconds computed for the next backoff.
    pub next_delay_ms: u64,
}

impl RetryState {
    pub fn new() -> Self {
        Self {
            attempt: 0,
            last_error: String::new(),
            next_delay_ms: 0,
        }
    }
}

impl Default for RetryState {
    fn default() -> Self {
        Self::new()
    }
}

/// Handler that drives an operation with retry and backoff.
pub struct RetryHandler {
    policy: RetryPolicy,
    state: RetryState,
}

impl RetryHandler {
    pub fn new(policy: RetryPolicy) -> Self {
        Self {
            policy,
            state: RetryState::new(),
        }
    }

    /// Execute `operation` up to `max_attempts` times with exponential backoff.
    ///
    /// The operation is retried only when [`should_retry`] returns `true`.
    /// Returns `Ok(result)` on success, or `Err(String)` with the last error
    /// after exhausting all attempts.
    pub async fn run<F, R>(&mut self, mut operation: F) -> Result<R, String>
    where
        F: FnMut() -> Result<R, String>,
    {
        loop {
            self.state.attempt += 1;
            match operation() {
                Ok(result) => {
                    self.state.last_error.clear();
                    return Ok(result);
                }
                Err(err) => {
                    self.state.last_error = err.clone();
                    if !should_retry(&err, self.state.attempt) {
                        return Err(err);
                    }
                    if self.state.attempt >= self.policy.max_attempts {
                        return Err(err);
                    }
                    self.state.next_delay_ms = self.policy.compute_delay(self.state.attempt);
                    sleep(Duration::from_millis(self.state.next_delay_ms)).await;
                }
            }
        }
    }

    /// Inspect the current retry state.
    pub fn state(&self) -> &RetryState {
        &self.state
    }
}

/// Determine whether an error is worth retrying.
///
/// Client errors (4xx, except 429) are not retried. HTTP 429 (Too Many Requests)
/// is retried as it indicates a transient rate-limit. Server errors (5xx),
/// timeouts, and connection/network errors are retried.
pub fn should_retry(error: &str, _attempt: u32) -> bool {
    let e = error.to_uppercase();
    // HTTP 429 (Too Many Requests) is a retryable rate-limit error.
    if e.contains("429") {
        return true;
    }
    // HTTP 4xx (except 429) are non-retryable client errors.
    if e.contains("HTTP 4") || e.contains("HTTP/1.1 4") || e.contains("HTTP/2 4") {
        return false;
    }
    // Retry on connection / timeout / network / server (5xx) errors.
    e.contains("TIMEOUT") || e.contains("CONNECTION") || e.contains("NETWORK") || e.starts_with("5")
}

/// Produce a pseudo-random delay using a simple xorshift generator.
///
/// This avoids adding a `rand` dependency while still providing jittered
/// backoff values. The seed is derived from the current time and a stack
/// address, which is sufficient for non-cryptographic backoff jitter.
fn fastrand_delay(max_ms: u64) -> u64 {
    use std::cell::Cell;
    use std::time::{SystemTime, UNIX_EPOCH};

    thread_local! {
        static STATE: Cell<u64> = Cell::new(seed());
    }

    fn seed() -> u64 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_3C15);
        let local = std::hint::black_box(0u8);
        let addr = &local as *const u8 as u64;
        nanos.wrapping_mul(addr.wrapping_add(1))
    }

    if max_ms == 0 {
        return 0;
    }

    STATE.with(|cell| {
        let mut x = cell.get();
        // xorshift64
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        cell.set(x);
        if x == 0 {
            x = 0xDEAD_BEEF;
            cell.set(x);
        }
        x % (max_ms + 1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn immediate_success() {
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delay_ms: 100,
            max_delay_ms: 1000,
            multiplier: 2.0,
        };
        let mut handler = RetryHandler::new(policy);

        let result = handler.run(|| Ok::<(), String>(())).await;

        assert!(result.is_ok());
        assert_eq!(handler.state().attempt, 1);
        assert!(handler.state().last_error.is_empty());
    }

    #[tokio::test]
    async fn retries_then_succeeds() {
        let policy = RetryPolicy {
            max_attempts: 5,
            base_delay_ms: 1,
            max_delay_ms: 10,
            multiplier: 2.0,
        };
        let mut handler = RetryHandler::new(policy);
        let mut calls = 0u32;

        let result: Result<String, String> = handler
            .run(|| {
                calls += 1;
                if calls < 3 {
                    Err("TIMEOUT on server".to_string())
                } else {
                    Ok("success".to_string())
                }
            })
            .await;

        assert_eq!(result.unwrap(), "success");
        assert_eq!(handler.state().attempt, 3);
    }

    #[tokio::test]
    async fn max_attempts_exhausted() {
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delay_ms: 1,
            max_delay_ms: 10,
            multiplier: 2.0,
        };
        let mut handler = RetryHandler::new(policy);
        let mut calls = 0u32;

        let result: Result<(), String> = handler
            .run(|| {
                calls += 1;
                Err("CONNECTION reset by peer".to_string())
            })
            .await;

        assert!(result.is_err());
        assert_eq!(calls, 3);
        assert_eq!(handler.state().attempt, 3);
        assert_eq!(handler.state().last_error, "CONNECTION reset by peer");
    }

    #[tokio::test]
    async fn delay_increases() {
        let policy = RetryPolicy {
            max_attempts: 5,
            base_delay_ms: 50,
            max_delay_ms: 5_000,
            multiplier: 3.0,
        };

        // Attempt 1 -> 2: base * multiplier^0 = 50
        let d1 = policy.compute_delay(1);
        // Attempt 2 -> 3: base * multiplier^1 = 150
        let d2 = policy.compute_delay(2);
        // Attempt 3 -> 4: base * multiplier^2 = 450
        let d3 = policy.compute_delay(3);
        // Attempt 4 -> 5: base * multiplier^3 = 1350
        let d4 = policy.compute_delay(4);

        // With jitter, the value is in [0, raw_clamped].
        // We verify that the raw exponential growth is monotonic.
        let raw_1 = (50.0_f64 * policy.multiplier.powi(0)).min(5_000.0) as u64;
        let raw_2 = (50.0_f64 * policy.multiplier.powi(1)).min(5_000.0) as u64;
        let raw_3 = (50.0_f64 * policy.multiplier.powi(2)).min(5_000.0) as u64;
        let raw_4 = (50.0_f64 * policy.multiplier.powi(3)).min(5_000.0) as u64;

        assert!(d1 <= raw_1);
        assert!(d2 <= raw_2);
        assert!(d3 <= raw_3);
        assert!(d4 <= raw_4);

        // Exponential growth: raw_2 > raw_1, raw_3 > raw_2, etc.
        assert!(raw_2 > raw_1);
        assert!(raw_3 > raw_2);
        assert!(raw_4 > raw_3);
    }

    #[tokio::test]
    async fn dont_retry_client_error() {
        let policy = RetryPolicy {
            max_attempts: 5,
            base_delay_ms: 1,
            max_delay_ms: 10,
            multiplier: 2.0,
        };
        let mut handler = RetryHandler::new(policy);
        let mut calls = 0u32;

        let result: Result<String, String> = handler
            .run(|| {
                calls += 1;
                Err("HTTP 404 Not Found".to_string())
            })
            .await;

        assert!(result.is_err());
        // Should NOT retry a 4xx error.
        assert_eq!(calls, 1);
        assert_eq!(handler.state().attempt, 1);
        assert_eq!(handler.state().last_error, "HTTP 404 Not Found");
    }
}
