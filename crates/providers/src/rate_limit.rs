//! Provider account lock planning helpers and bounded request rate limiting.

use std::collections::HashMap;
use std::time::{Duration, Instant};

pub const MAX_FAILURE_REASON_CHARS: usize = 100;

/// Maximum number of provider buckets retained by a limiter.
const MAX_PROVIDER_BUCKETS: usize = 1024;

#[derive(Debug)]
struct Window {
    started: Instant,
    used: u32,
}

/// Bounded per-provider fixed-window request limiter.
///
/// A bucket is reset when its configured window elapses. Provider identifiers
/// are bounded to prevent untrusted IDs from growing memory without limit.
#[derive(Debug)]
pub struct RateLimiter {
    capacity: u32,
    window: Duration,
    providers: HashMap<String, Window>,
}

impl RateLimiter {
    #[must_use]
    pub fn new(capacity: u32, window: Duration) -> Self {
        Self {
            capacity,
            window,
            providers: HashMap::new(),
        }
    }

    /// Consume one request token, returning whether the request is admitted.
    pub fn check(&mut self, provider_id: &str) -> bool {
        if self.capacity == 0 {
            return false;
        }
        let now = Instant::now();
        let expired = self
            .providers
            .get(provider_id)
            .is_some_and(|bucket| now.duration_since(bucket.started) >= self.window);
        if expired {
            self.providers.remove(provider_id);
        }
        if !self.providers.contains_key(provider_id) && self.providers.len() >= MAX_PROVIDER_BUCKETS
        {
            if let Some(oldest) = self
                .providers
                .iter()
                .min_by_key(|(_, bucket)| bucket.started)
                .map(|(id, _)| id.clone())
            {
                self.providers.remove(&oldest);
            }
        }
        let bucket = self
            .providers
            .entry(provider_id.to_owned())
            .or_insert(Window {
                started: now,
                used: 0,
            });
        if bucket.used >= self.capacity {
            return false;
        }
        bucket.used += 1;
        true
    }

    #[must_use]
    pub fn remaining(&self, provider_id: &str) -> u32 {
        let Some(bucket) = self.providers.get(provider_id) else {
            return self.capacity;
        };
        if bucket.started.elapsed() >= self.window {
            self.capacity
        } else {
            self.capacity.saturating_sub(bucket.used)
        }
    }

    pub fn reset(&mut self, provider_id: &str) {
        self.providers.remove(provider_id);
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(60, Duration::from_secs(60))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_returns_true_when_empty() {
        let mut limiter = RateLimiter::new(10, Duration::from_secs(60));
        assert!(limiter.check("provider-a"));
    }

    #[test]
    fn check_returns_false_when_empty() {
        let mut limiter = RateLimiter::new(2, Duration::from_secs(60));
        assert!(limiter.check("provider-b"));
        assert!(limiter.check("provider-b"));
        assert!(!limiter.check("provider-b"));
    }

    #[test]
    fn refills_after_window() {
        let mut limiter = RateLimiter::new(1, Duration::from_millis(100));
        assert!(limiter.check("provider-c"));
        assert!(!limiter.check("provider-c"));
        std::thread::sleep(Duration::from_millis(150));
        assert!(limiter.check("provider-c"));
    }

    #[test]
    fn remaining_decreases() {
        let mut limiter = RateLimiter::new(5, Duration::from_secs(60));
        assert_eq!(limiter.remaining("provider-d"), 5);
        limiter.check("provider-d");
        assert_eq!(limiter.remaining("provider-d"), 4);
        limiter.check("provider-d");
        assert_eq!(limiter.remaining("provider-d"), 3);
    }

    #[test]
    fn reset_clears_state() {
        let mut limiter = RateLimiter::new(3, Duration::from_secs(60));
        limiter.check("provider-e");
        limiter.check("provider-e");
        assert_eq!(limiter.remaining("provider-e"), 1);
        limiter.reset("provider-e");
        assert_eq!(limiter.remaining("provider-e"), 3);
        assert!(limiter.check("provider-e"));
    }
}
pub const MAX_RATE_LIMIT_COOLDOWN_MS: u64 = 30 * 60 * 1_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccountLockScope {
    Account,
    AccountWide,
    Model(String),
}

pub const MAX_ACCOUNT_MODEL_LOCKS: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountLock {
    pub scope: AccountLockScope,
    pub expires_at: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountRecoveryState {
    pub test_status_unavailable: bool,
    pub last_error_present: bool,
    pub backoff_level: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountRecoveryPatch {
    pub clear_locks: Vec<AccountLockScope>,
    pub set_test_status_active: bool,
    pub clear_last_error: bool,
    pub reset_backoff_level: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountRecoveryError {
    TooManyLocks { max: usize, actual: usize },
}

pub fn plan_account_recovery(
    connection_id: Option<&str>,
    successful_model: Option<&str>,
    state: &AccountRecoveryState,
    locks: &[AccountLock],
    now: u64,
) -> Result<Option<AccountRecoveryPatch>, AccountRecoveryError> {
    if connection_id.is_none_or(|id| id.is_empty() || id == "noauth") {
        return Ok(None);
    }

    if locks.len() > MAX_ACCOUNT_MODEL_LOCKS {
        return Err(AccountRecoveryError::TooManyLocks {
            max: MAX_ACCOUNT_MODEL_LOCKS,
            actual: locks.len(),
        });
    }

    if locks.is_empty() && !state.test_status_unavailable && !state.last_error_present {
        return Ok(None);
    }

    let mut clear_locks = Vec::with_capacity(locks.len());
    let mut remaining_active_locks = 0usize;

    for lock in locks {
        let successful_scope = successful_model.is_some_and(|model| match &lock.scope {
            AccountLockScope::Model(locked_model) => locked_model == model,
            AccountLockScope::Account | AccountLockScope::AccountWide => true,
        });
        let expired = lock.expires_at <= now;

        if successful_scope || expired {
            clear_locks.push(lock.scope.clone());
        } else if lock.expires_at > now {
            remaining_active_locks += 1;
        }
    }

    if clear_locks.is_empty() && !state.test_status_unavailable && !state.last_error_present {
        return Ok(None);
    }

    let reset_error_state = remaining_active_locks == 0;
    Ok(Some(AccountRecoveryPatch {
        clear_locks,
        set_test_status_active: reset_error_state,
        clear_last_error: reset_error_state,
        reset_backoff_level: reset_error_state,
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenericFallbackOutcome {
    pub should_fallback: bool,
    pub cooldown_ms: u64,
    pub backoff_level: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountFailurePatch {
    pub connection_id: String,
    pub lock_scope: AccountLockScope,
    pub lock_until_ms: u64,
    pub backoff_level: u32,
    pub error_code: u16,
    pub last_error_at_ms: u64,
    pub reason: String,
    pub mark_unavailable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccountFailureDecision {
    Noop,
    Update(AccountFailurePatch),
}

const GITHUB_MONTHLY_USAGE_LIMIT: &str = "you've reached your additional usage limit for your plan";

#[allow(clippy::too_many_arguments)]
pub fn plan_account_failure_lock(
    connection_id: Option<&str>,
    status: u16,
    error_text: &str,
    provider: Option<&str>,
    model: Option<&str>,
    resets_at_ms: Option<u64>,
    now: u64,
    generic: GenericFallbackOutcome,
    next_month_reset_ms: u64,
) -> AccountFailureDecision {
    let Some(connection_id) = connection_id.filter(|id| !id.is_empty() && *id != "noauth") else {
        return AccountFailureDecision::Noop;
    };

    let github_monthly = provider.is_some_and(|provider| provider.eq_ignore_ascii_case("github"))
        && status == 402
        && error_text
            .to_ascii_lowercase()
            .contains(GITHUB_MONTHLY_USAGE_LIMIT);

    let (lock_scope, lock_until_ms, backoff_level) = if github_monthly {
        (AccountLockScope::Account, next_month_reset_ms, 0)
    } else if let Some(reset_at) = resets_at_ms.filter(|reset_at| *reset_at > now) {
        let lock_until_ms =
            if provider.is_some_and(|provider| provider.eq_ignore_ascii_case("antigravity")) {
                reset_at
            } else {
                now.saturating_add((reset_at - now).min(MAX_RATE_LIMIT_COOLDOWN_MS))
            };
        (
            model.map_or(AccountLockScope::Account, |model| {
                AccountLockScope::Model(model.to_owned())
            }),
            lock_until_ms,
            0,
        )
    } else {
        if !generic.should_fallback {
            return AccountFailureDecision::Noop;
        }
        (
            model.map_or(AccountLockScope::Account, |model| {
                AccountLockScope::Model(model.to_owned())
            }),
            now.saturating_add(generic.cooldown_ms),
            generic.backoff_level,
        )
    };

    AccountFailureDecision::Update(AccountFailurePatch {
        connection_id: connection_id.to_owned(),
        lock_scope,
        lock_until_ms,
        backoff_level,
        error_code: status,
        last_error_at_ms: now,
        reason: error_text.chars().take(MAX_FAILURE_REASON_CHARS).collect(),
        mark_unavailable: true,
    })
}
