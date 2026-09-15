//! Deterministic account-storage sync planning (ROUTE-009).
//!
//! Pure computation: given a caller-owned cached snapshot and a freshly
//! fetched remote list, derive the bounded sync plan (upserts + deletes).
//! No persistence, no I/O, no wall-clock access.

use std::collections::{HashMap, HashSet};

/// One account record participating in a sync plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncedAccount {
    pub account_id: String,
    pub provider_id: String,
    pub active: bool,
    pub updated_at: u64,
}

/// Deterministic bounded plan: entries to upsert plus cached ids to delete.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AccountSyncPlan {
    pub upsert: Vec<SyncedAccount>,
    pub delete_ids: Vec<String>,
}

/// Typed sync-plan failures.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AccountSyncError {
    #[error("too many accounts: max {max}, actual {actual}")]
    TooManyAccounts { max: usize, actual: usize },
    #[error("duplicate account id: {account_id}")]
    DuplicateAccountId { account_id: String },
}

/// Hard bound on either input side.
pub const MAX_SYNC_ACCOUNTS: usize = 128;

fn check_side(accounts: &[SyncedAccount]) -> Result<(), AccountSyncError> {
    if accounts.len() > MAX_SYNC_ACCOUNTS {
        return Err(AccountSyncError::TooManyAccounts {
            max: MAX_SYNC_ACCOUNTS,
            actual: accounts.len(),
        });
    }
    let mut seen = HashSet::with_capacity(accounts.len());
    for account in accounts {
        if !seen.insert(account.account_id.as_str()) {
            return Err(AccountSyncError::DuplicateAccountId {
                account_id: account.account_id.clone(),
            });
        }
    }
    Ok(())
}

/// Compute the deterministic sync plan bringing `cached` in line with `remote`.
///
/// `upsert` holds remote entries absent from the cache or differing in any
/// compared field (`provider_id`, `active`, `updated_at`), sorted by
/// `account_id`. `delete_ids` holds cached ids absent from remote, sorted.
/// Identical snapshots yield an empty plan.
pub fn plan_account_sync(
    cached: &[SyncedAccount],
    remote: &[SyncedAccount],
) -> Result<AccountSyncPlan, AccountSyncError> {
    check_side(cached)?;
    check_side(remote)?;

    let cached_by_id: HashMap<&str, &SyncedAccount> =
        cached.iter().map(|entry| (entry.account_id.as_str(), entry)).collect();
    let remote_ids: HashSet<&str> = remote
        .iter()
        .map(|entry| entry.account_id.as_str())
        .collect();

    let mut upsert: Vec<SyncedAccount> = remote
        .iter()
        .filter(|entry| match cached_by_id.get(entry.account_id.as_str()) {
            None => true,
            Some(known) => {
                known.provider_id != entry.provider_id
                    || known.active != entry.active
                    || known.updated_at != entry.updated_at
            }
        })
        .cloned()
        .collect();
    upsert.sort_by(|left, right| left.account_id.cmp(&right.account_id));

    let mut delete_ids: Vec<String> = cached
        .iter()
        .filter(|entry| !remote_ids.contains(entry.account_id.as_str()))
        .map(|entry| entry.account_id.clone())
        .collect();
    delete_ids.sort();

    Ok(AccountSyncPlan { upsert, delete_ids })
}
