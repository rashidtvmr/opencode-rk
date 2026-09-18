#![forbid(unsafe_code)]

//! App-level provider routing types (PAR-002).
//!
//! Commit `5af7884`. Source evidence:
//! - `crates/providers/src/router.rs:53,88,232-243`: `MAX_ACCOUNT_SELECTION_CANDIDATES
//!   = 16`, `MAX_ACCOUNT_ID_BYTES = 128`, caller-owned persistence with `now`
//!   supplied by the caller (no wall-clock dependency here either).
//! - `crates/providers/src/rate_limit.rs:6`: `MAX_FAILURE_REASON_CHARS = 100`.
//! - `crates/providers/src/budget.rs:152-174`: `consume_tokens` validates before
//!   mutating, so a failed consume preserves budget state; mirrored by
//!   [`AccountBudget::consume`].
//!
//! Contract: validated non-secret account identities, a deterministic
//! quota-exhausted vs rate-limited classifier, a bounded per-account budget
//! holder that never mutates on failed consume, a total failover function that
//! never selects the failed or an excluded account, and a debug exporter that
//! carries no credential material and redacts caller-listed secrets.
//!
//! Credential bytes cannot cross this API by type: no field stores tokens,
//! keys, or URLs. `export_redacted` additionally scrubs caller-supplied free
//! text (failure reasons) against an explicit secret denylist.

use std::collections::HashSet;
use std::fmt;

/// Maximum account identifier length in bytes. Mirrors
/// `router::MAX_ACCOUNT_ID_BYTES`.
pub const MAX_ACCOUNT_ID_BYTES: usize = 128;

/// Maximum accounts retained by a holder. Mirrors
/// `router::MAX_ACCOUNT_SELECTION_CANDIDATES` so routing callers can move
/// candidates through without re-bounding.
pub const MAX_ACCOUNTS: usize = 16;

/// Maximum retained failure-reason characters. Mirrors
/// `rate_limit::MAX_FAILURE_REASON_CHARS`.
pub const MAX_FAILURE_REASON_CHARS: usize = 100;

/// Marker substituted for redacted secret material.
pub const REDACTED: &str = "[REDACTED]";

/// Validated non-secret provider account identifier.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AccountId(String);

/// Failures when constructing an [`AccountId`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountIdError {
    Empty,
    TooLong { max: usize, actual: usize },
}

impl fmt::Display for AccountIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("account id is empty"),
            Self::TooLong { max, actual } => {
                write!(f, "account id too long: {actual} bytes, max {max}")
            }
        }
    }
}

impl std::error::Error for AccountIdError {}

impl AccountId {
    /// Validate before allocating; no partial id exists on failure.
    pub fn new(id: &str) -> Result<Self, AccountIdError> {
        if id.is_empty() {
            return Err(AccountIdError::Empty);
        }
        if id.len() > MAX_ACCOUNT_ID_BYTES {
            return Err(AccountIdError::TooLong {
                max: MAX_ACCOUNT_ID_BYTES,
                actual: id.len(),
            });
        }
        Ok(Self(id.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// Account ids are non-secret routing keys (router.rs derives Debug on String
// ids), so showing the id is safe. Failure reasons are NOT shown here; see
// AccountBudget's Debug impl.
impl fmt::Debug for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AccountId({:?})", self.0)
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Quota-exhausted vs rate-limited vs terminal/other provider failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureKind {
    QuotaExhausted,
    RateLimited,
    Other,
}

impl FailureKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::QuotaExhausted => "quota-exhausted",
            Self::RateLimited => "rate-limited",
            Self::Other => "other",
        }
    }
}

impl fmt::Display for FailureKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

const QUOTA_MARKERS: &[&str] = &[
    "quota",
    "exhausted",
    "usage limit",
    "usage_limit",
    "billing",
    "insufficient",
    "credit",
    "expired plan",
];

const RATE_MARKERS: &[&str] = &[
    "rate limit",
    "rate_limit",
    "ratelimit",
    "too many requests",
    "throttle",
    "slow down",
];

/// Deterministically classify a provider failure.
///
/// Quota markers win over rate markers (a 429 carrying quota text is a quota
/// problem, not a retry-soon problem). HTTP 402 is quota by status; 429 is
/// rate-limited by status when no quota text is present.
#[must_use]
pub fn classify_failure(status: u16, error: &str) -> FailureKind {
    let lower = error.to_ascii_lowercase();
    if QUOTA_MARKERS.iter().any(|m| lower.contains(m)) || status == 402 {
        return FailureKind::QuotaExhausted;
    }
    if status == 429 || RATE_MARKERS.iter().any(|m| lower.contains(m)) {
        return FailureKind::RateLimited;
    }
    FailureKind::Other
}

/// Truncate free-text failure detail to [`MAX_FAILURE_REASON_CHARS`].
#[must_use]
pub fn truncate_reason(reason: &str) -> String {
    reason.chars().take(MAX_FAILURE_REASON_CHARS).collect()
}

/// Stored per-account failure detail (bounded, truncated, never a secret).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredFailure {
    pub status: u16,
    pub kind: FailureKind,
    pub reason: String,
}

/// Per-account token budget with an optional last-failure annotation.
#[derive(Clone, Eq, PartialEq)]
pub struct AccountBudget {
    account: AccountId,
    used: u64,
    limit: u64,
    last_failure: Option<StoredFailure>,
}

// Custom Debug: reason text may embed caller secrets, so only its length is
// shown. Full text leaves the process exclusively via export_redacted, which
// applies the secret denylist.
impl fmt::Debug for AccountBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccountBudget")
            .field("account", &self.account)
            .field("used", &self.used)
            .field("limit", &self.limit)
            .field(
                "last_failure_reason_chars",
                &self.last_failure.as_ref().map(|fl| fl.reason.len()),
            )
            .finish()
    }
}

/// Budget-holder failures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BudgetError {
    UnknownAccount { account_id: String },
    TooManyAccounts { max: usize, actual: usize },
    BudgetExhausted {
        requested: u64,
        used: u64,
        limit: u64,
    },
}

impl fmt::Display for BudgetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownAccount { account_id } => {
                write!(f, "unknown account: {account_id:?}")
            }
            Self::TooManyAccounts { max, actual } => {
                write!(f, "too many accounts: {actual}, max {max}")
            }
            Self::BudgetExhausted {
                requested,
                used,
                limit,
            } => write!(
                f,
                "account budget exhausted: requested {requested}, used {used}, limit {limit}"
            ),
        }
    }
}

impl std::error::Error for BudgetError {}

impl AccountBudget {
    #[must_use]
    pub fn account(&self) -> &AccountId {
        &self.account
    }

    #[must_use]
    pub fn used(&self) -> u64 {
        self.used
    }

    #[must_use]
    pub fn limit(&self) -> u64 {
        self.limit
    }

    #[must_use]
    pub fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.used)
    }

    #[must_use]
    pub fn last_failure(&self) -> Option<&StoredFailure> {
        self.last_failure.as_ref()
    }

    /// Consume `amount` tokens. Validates first: on exhaustion `used` is
    /// unchanged (mirrors `budget.rs` consume-before-mutate).
    pub fn consume(&mut self, amount: u64) -> Result<u64, BudgetError> {
        let next = self.used.saturating_add(amount);
        if next > self.limit {
            return Err(BudgetError::BudgetExhausted {
                requested: amount,
                used: self.used,
                limit: self.limit,
            });
        }
        self.used = next;
        Ok(self.remaining())
    }

    /// Record a classified, truncated failure annotation.
    pub fn record_failure(&mut self, status: u16, error: &str) {
        self.last_failure = Some(StoredFailure {
            status,
            kind: classify_failure(status, error),
            reason: truncate_reason(error),
        });
    }

    pub fn reset(&mut self) {
        self.used = 0;
        self.last_failure = None;
    }
}

/// Bounded per-account budget holder. Caller owns persistence; `now`-style
/// clock input is unnecessary here because budgets carry no timestamps.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountBudgetHolder {
    budgets: Vec<AccountBudget>,
}

impl AccountBudgetHolder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.budgets.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.budgets.is_empty()
    }

    pub fn insert(&mut self, account: AccountId, limit: u64) -> Result<(), BudgetError> {
        if self.find(&account).is_none() && self.budgets.len() >= MAX_ACCOUNTS {
            return Err(BudgetError::TooManyAccounts {
                max: MAX_ACCOUNTS,
                actual: self.budgets.len().saturating_add(1),
            });
        }
        if let Some(slot) = self.find_mut(&account) {
            slot.limit = limit;
            return Ok(());
        }
        self.budgets.push(AccountBudget {
            account,
            used: 0,
            limit,
            last_failure: None,
        });
        Ok(())
    }

    pub fn consume(&mut self, account: &AccountId, amount: u64) -> Result<u64, BudgetError> {
        let slot = self.find_mut(account).ok_or_else(|| BudgetError::UnknownAccount {
            account_id: account.as_str().to_owned(),
        })?;
        slot.consume(amount)
    }

    pub fn record_failure(
        &mut self,
        account: &AccountId,
        status: u16,
        error: &str,
    ) -> Result<FailureKind, BudgetError> {
        let slot = self.find_mut(account).ok_or_else(|| BudgetError::UnknownAccount {
            account_id: account.as_str().to_owned(),
        })?;
        slot.record_failure(status, error);
        Ok(slot
            .last_failure
            .as_ref()
            .map_or(FailureKind::Other, |fl| fl.kind))
    }

    #[must_use]
    pub fn remaining(&self, account: &AccountId) -> Option<u64> {
        self.find(account).map(AccountBudget::remaining)
    }

    pub fn reset(&mut self, account: &AccountId) -> bool {
        if let Some(slot) = self.find_mut(account) {
            slot.reset();
            true
        } else {
            false
        }
    }

    fn find(&self, account: &AccountId) -> Option<&AccountBudget> {
        self.budgets.iter().find(|b| &b.account == account)
    }

    fn find_mut(&mut self, account: &AccountId) -> Option<&mut AccountBudget> {
        self.budgets.iter_mut().find(|b| &b.account == account)
    }

    /// Render a secret-free debug summary, scrubbing `secrets` from any
    /// caller-supplied free text. Output contains ids, counters, failure
    /// kinds/statuses, and redacted reasons only.
    #[must_use]
    pub fn export_redacted(&self, secrets: &[&str]) -> String {
        let mut out = String::new();
        for budget in &self.budgets {
            let (kind, status, reason) = budget.last_failure.as_ref().map_or(
                ("none", String::new(), "none".to_owned()),
                |fl| (fl.kind.as_str(), fl.status.to_string(), fl.reason.clone()),
            );
            out.push_str(&format!(
                "account id={:?} used={} limit={} remaining={} last={kind}:{status}:{reason}\n",
                budget.account.as_str(),
                budget.used,
                budget.limit,
                budget.remaining(),
            ));
        }
        redact_text(&out, secrets)
    }
}

/// Replace every non-empty secret occurrence with [`REDACTED`].
#[must_use]
pub fn redact_text(text: &str, secrets: &[&str]) -> String {
    let mut redacted = text.to_owned();
    for secret in secrets.iter().filter(|s| !s.is_empty()) {
        redacted = redacted.replace(secret, REDACTED);
    }
    redacted
}

/// Failover routing outcome for one failed account attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FailoverDecision {
    /// Failure is terminal/other: stay, no retry, no failover.
    Stay,
    /// Quota is exhausted on `from`; continue on `to`, never `from` itself.
    Failover { from: AccountId, to: AccountId },
    /// No eligible target remains.
    Exhausted,
    /// Rate-limited: retry the same account no earlier than `retry_at`
    /// (caller-supplied timestamp; no clock read here).
    RetryAfter { retry_at: u64 },
}

/// Total failover decision over an explicit candidate list.
///
/// Safety: the target is never `current` and never in `excluded`; when no
/// target qualifies the result is `Exhausted`, never a self-loop. Quota
/// exhaustion fails over, rate-limiting retries (or stays when no `retry_at`
/// is known), anything else stays.
#[must_use]
pub fn decide_failover(
    current: &AccountId,
    candidates: &[AccountId],
    excluded: &[AccountId],
    failure: FailureKind,
    retry_at: Option<u64>,
) -> FailoverDecision {
    match failure {
        FailureKind::Other => FailoverDecision::Stay,
        FailureKind::RateLimited => retry_at.map_or(FailoverDecision::Stay, |retry_at| {
            FailoverDecision::RetryAfter { retry_at }
        }),
        FailureKind::QuotaExhausted => {
            let denied: HashSet<&str> =
                excluded.iter().map(AccountId::as_str).collect();
            candidates
                .iter()
                .find(|candidate| {
                    candidate.as_str() != current.as_str()
                        && !denied.contains(candidate.as_str())
                })
                .map_or(FailoverDecision::Exhausted, |to| FailoverDecision::Failover {
                    from: current.clone(),
                    to: to.clone(),
                })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(name: &str) -> AccountId {
        AccountId::new(name).expect("valid test id")
    }

    #[test]
    fn account_id_validation() {
        assert_eq!(AccountId::new(""), Err(AccountIdError::Empty));
        let long = "a".repeat(MAX_ACCOUNT_ID_BYTES + 1);
        assert!(matches!(
            AccountId::new(&long),
            Err(AccountIdError::TooLong { .. })
        ));
        assert_eq!(id("acc-1").as_str(), "acc-1");
    }

    #[test]
    fn classifier_separates_quota_from_rate_limit() {
        assert_eq!(
            classify_failure(402, "payment required"),
            FailureKind::QuotaExhausted
        );
        assert_eq!(
            classify_failure(429, "quota exceeded for model"),
            FailureKind::QuotaExhausted
        );
        assert_eq!(
            classify_failure(400, "insufficient_quota: billing limit hit"),
            FailureKind::QuotaExhausted
        );
        assert_eq!(
            classify_failure(429, "too many requests, slow down"),
            FailureKind::RateLimited
        );
        assert_eq!(
            classify_failure(503, "model overloaded"),
            FailureKind::Other
        );
    }

    #[test]
    fn quota_preserves_budget_on_failed_consume() {
        let mut holder = AccountBudgetHolder::new();
        holder.insert(id("a"), 100).expect("insert");
        assert_eq!(holder.consume(&id("a"), 60), Ok(40));
        let err = holder.consume(&id("a"), 41).expect_err("must exhaust");
        assert_eq!(
            err,
            BudgetError::BudgetExhausted {
                requested: 41,
                used: 60,
                limit: 100
            }
        );
        assert_eq!(holder.remaining(&id("a")), Some(40));
        assert_eq!(holder.consume(&id("a"), 40), Ok(0));
    }

    #[test]
    fn holder_bounds_and_unknown_account() {
        let mut holder = AccountBudgetHolder::new();
        for n in 0..MAX_ACCOUNTS {
            holder
                .insert(id(&format!("acc-{n}")), 10)
                .expect("fits bound");
        }
        assert!(matches!(
            holder.insert(id("overflow"), 10),
            Err(BudgetError::TooManyAccounts { .. })
        ));
        assert!(matches!(
            holder.consume(&id("ghost"), 1),
            Err(BudgetError::UnknownAccount { .. })
        ));
    }

    #[test]
    fn failover_never_selects_current_or_excluded() {
        let current = id("a");
        let b = id("b");
        let c = id("c");
        let all = vec![current.clone(), b.clone(), c.clone()];

        let decision = decide_failover(
            &current,
            &all,
            &[b.clone()],
            FailureKind::QuotaExhausted,
            None,
        );
        assert_eq!(
            decision,
            FailoverDecision::Failover {
                from: current.clone(),
                to: c.clone()
            }
        );

        let decision = decide_failover(
            &current,
            &all,
            &[b.clone(), c.clone()],
            FailureKind::QuotaExhausted,
            None,
        );
        assert_eq!(decision, FailoverDecision::Exhausted);

        let decision = decide_failover(
            &current,
            &all,
            &[],
            FailureKind::RateLimited,
            Some(9_999),
        );
        assert_eq!(decision, FailoverDecision::RetryAfter { retry_at: 9_999 });

        let decision =
            decide_failover(&current, &all, &[], FailureKind::Other, Some(1));
        assert_eq!(decision, FailoverDecision::Stay);
    }

    #[test]
    fn export_redacts_secrets_and_hides_debug_reason() {
        let mut holder = AccountBudgetHolder::new();
        holder.insert(id("a"), 100).expect("insert");
        let secret = "sk-live-SECRET-123";
        holder
            .record_failure(&id("a"), 429, &format!("rate limited, key {secret} rejected"))
            .expect("record");
        // classify sanity: no quota marker present
        assert_eq!(
            holder
                .find(&id("a"))
                .and_then(AccountBudget::last_failure)
                .map(|fl| fl.kind),
            Some(FailureKind::RateLimited)
        );

        let exported = holder.export_redacted(&[secret]);
        assert!(!exported.contains(secret), "secret leaked: {exported}");
        assert!(exported.contains(REDACTED), "no redaction marker");
        assert!(exported.contains("rate-limited"), "kind missing");

        let debug = format!("{:?}", holder.find(&id("a")).expect("present"));
        assert!(!debug.contains(secret), "secret in Debug: {debug}");
    }

    #[test]
    fn reason_truncation_bounds_memory() {
        let long = "e".repeat(MAX_FAILURE_REASON_CHARS + 50);
        assert_eq!(truncate_reason(&long).chars().count(), MAX_FAILURE_REASON_CHARS);
    }
}
