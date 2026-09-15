//! Pure account-status projection for the status panel (ROUTE-008).
//!
//! Projects caller-owned account records into a bounded, deterministic status
//! list. No persistence, no network, no clock; `now` is caller supplied.

/// Caller-owned account record feeding the status panel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountStatusRecord {
    pub account_id: String,
    pub provider_id: String,
    pub active: bool,
    pub locked_until: Option<u64>,
    pub last_error: Option<String>,
}

/// Projected status row shown in the status panel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountStatusView {
    pub account_id: String,
    pub provider_id: String,
    /// One of `"active" | "inactive" | "locked"`.
    pub state: &'static str,
    pub retry_at: Option<u64>,
}

/// Typed projection failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AccountStatusError {
    #[error("no accounts to project")]
    EmptyAccounts,
    #[error("too many accounts: max {max}, actual {actual}")]
    TooManyAccounts { max: usize, actual: usize },
}

/// Upper bound on accounts per projection call.
pub const MAX_STATUS_ACCOUNTS: usize = 128;

/// Caller-visible bound for `last_error` text; the view itself carries no
/// error text, but overlong values must never panic the projection.
const MAX_LAST_ERROR_CHARS: usize = 100;

/// Project account records into a deterministic status list sorted by
/// `(provider_id, account_id)`.
pub fn project_account_status(
    accounts: &[AccountStatusRecord],
    now: u64,
) -> Result<Vec<AccountStatusView>, AccountStatusError> {
    if accounts.is_empty() {
        return Err(AccountStatusError::EmptyAccounts);
    }
    if accounts.len() > MAX_STATUS_ACCOUNTS {
        return Err(AccountStatusError::TooManyAccounts {
            max: MAX_STATUS_ACCOUNTS,
            actual: accounts.len(),
        });
    }

    let mut views = accounts
        .iter()
        .map(|record| {
            // ponytail: char-boundary-safe touch proves overlong last_error
            // cannot panic; view carries no error text. Upgrade path: add an
            // error_summary field if the panel needs caller-visible text.
            let _ = record
                .last_error
                .as_deref()
                .map(|error| error.chars().take(MAX_LAST_ERROR_CHARS).count());
            let locked = record.active && record.locked_until.is_some_and(|until| until > now);
            let (state, retry_at) = if !record.active {
                ("inactive", None)
            } else if locked {
                ("locked", record.locked_until)
            } else {
                ("active", None)
            };
            AccountStatusView {
                account_id: record.account_id.clone(),
                provider_id: record.provider_id.clone(),
                state,
                retry_at,
            }
        })
        .collect::<Vec<_>>();
    views.sort_by(|left, right| {
        left.provider_id
            .cmp(&right.provider_id)
            .then_with(|| left.account_id.cmp(&right.account_id))
    });
    Ok(views)
}
