//! Provider account lock planning helpers.

pub const MAX_FAILURE_REASON_CHARS: usize = 100;
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

    if clear_locks.is_empty()
        && !state.test_status_unavailable
        && !state.last_error_present
    {
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

const GITHUB_MONTHLY_USAGE_LIMIT: &str =
    "you've reached your additional usage limit for your plan";

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
        let lock_until_ms = if provider
            .is_some_and(|provider| provider.eq_ignore_ascii_case("antigravity"))
        {
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
