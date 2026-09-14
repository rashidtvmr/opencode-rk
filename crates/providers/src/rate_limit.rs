//! Provider account lock planning helpers.

pub const MAX_FAILURE_REASON_CHARS: usize = 100;
pub const MAX_RATE_LIMIT_COOLDOWN_MS: u64 = 30 * 60 * 1_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccountLockScope {
    Account,
    Model(String),
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
