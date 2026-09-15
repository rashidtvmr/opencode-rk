//! Deterministic integration sync planning (INT-005).
//!
//! Pure computation: given the current cached snapshot and the next desired
//! snapshot, derive the bounded sync plan (ids to fetch + ids to drop).
//! No persistence, no I/O, no wall-clock access.

use std::collections::{HashMap, HashSet};

/// One integration record participating in a sync plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncItem {
    pub id: String,
    pub rev: u64,
}

/// Typed sync-plan failures.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SyncError {
    #[error("empty id")]
    EmptyId,
    #[error("too many items: max {max}, actual {actual}")]
    TooManyItems { max: usize, actual: usize },
    #[error("duplicate id: {id}")]
    DuplicateId { id: String },
}

/// Hard bound on either input side.
pub const MAX_SYNC_ITEMS: usize = 256;

fn check_side(items: &[SyncItem]) -> Result<(), SyncError> {
    if items.len() > MAX_SYNC_ITEMS {
        return Err(SyncError::TooManyItems {
            max: MAX_SYNC_ITEMS,
            actual: items.len(),
        });
    }
    let mut seen = HashSet::with_capacity(items.len());
    for item in items {
        if item.id.is_empty() {
            return Err(SyncError::EmptyId);
        }
        if !seen.insert(item.id.as_str()) {
            return Err(SyncError::DuplicateId {
                id: item.id.clone(),
            });
        }
    }
    Ok(())
}

/// Compute the deterministic sync plan bringing `cur` in line with `nxt`.
///
/// `to_fetch` holds `nxt` ids absent from `cur` or with a differing `rev`,
/// sorted. `to_drop` holds `cur` ids absent from `nxt`, sorted.
/// Identical snapshots yield an empty plan.
pub fn plan_sync(
    cur: &[SyncItem],
    nxt: &[SyncItem],
) -> Result<(Vec<String>, Vec<String>), SyncError> {
    check_side(cur)?;
    check_side(nxt)?;

    let cur_by_id: HashMap<&str, &SyncItem> =
        cur.iter().map(|entry| (entry.id.as_str(), entry)).collect();
    let nxt_ids: HashSet<&str> = nxt.iter().map(|entry| entry.id.as_str()).collect();

    let mut to_fetch: Vec<String> = nxt
        .iter()
        .filter(|entry| match cur_by_id.get(entry.id.as_str()) {
            None => true,
            Some(known) => known.rev != entry.rev,
        })
        .map(|entry| entry.id.clone())
        .collect();
    to_fetch.sort();

    let mut to_drop: Vec<String> = cur
        .iter()
        .filter(|entry| !nxt_ids.contains(entry.id.as_str()))
        .map(|entry| entry.id.clone())
        .collect();
    to_drop.sort();

    Ok((to_fetch, to_drop))
}
