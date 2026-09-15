//! Deterministic catalog sync planner (opencode.catalog-provider slice).
//!
//! Pure computation: diff caller-owned current snapshot against next snapshot,
//! derive bounded add/remove id lists. No persistence, no I/O.

use std::collections::{HashMap, HashSet};

/// One catalog record participating in a sync plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogEntry {
    pub id: String,
    pub version: u64,
}

/// Typed catalog-sync failures.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CatalogSyncError {
    #[error("catalog id must not be empty")]
    EmptyId,
    #[error("too many catalog entries: max {max}, actual {actual}")]
    TooManyEntries { max: usize, actual: usize },
    #[error("duplicate catalog id: {id}")]
    DuplicateId { id: String },
}

/// Hard bound on either input side.
pub const MAX_CATALOG_ENTRIES: usize = 512;

fn check_side(entries: &[CatalogEntry]) -> Result<(), CatalogSyncError> {
    if entries.len() > MAX_CATALOG_ENTRIES {
        return Err(CatalogSyncError::TooManyEntries {
            max: MAX_CATALOG_ENTRIES,
            actual: entries.len(),
        });
    }
    let mut seen = HashSet::with_capacity(entries.len());
    for entry in entries {
        if entry.id.is_empty() {
            return Err(CatalogSyncError::EmptyId);
        }
        if !seen.insert(entry.id.as_str()) {
            return Err(CatalogSyncError::DuplicateId {
                id: entry.id.clone(),
            });
        }
    }
    Ok(())
}

/// Compute `(to_add, to_remove)` id lists bringing `cur` in line with `nxt`.
///
/// `to_add` holds ids absent from `cur` or with a changed `version`, sorted.
/// `to_remove` holds `cur` ids absent from `nxt`, sorted.
pub fn plan_catalog_sync(
    cur: &[CatalogEntry],
    nxt: &[CatalogEntry],
) -> Result<(Vec<String>, Vec<String>), CatalogSyncError> {
    check_side(cur)?;
    check_side(nxt)?;

    let cur_by_id: HashMap<&str, &CatalogEntry> = cur.iter().map(|e| (e.id.as_str(), e)).collect();
    let nxt_ids: HashSet<&str> = nxt.iter().map(|e| e.id.as_str()).collect();

    let mut to_add: Vec<String> = nxt
        .iter()
        .filter(|e| match cur_by_id.get(e.id.as_str()) {
            None => true,
            Some(known) => known.version != e.version,
        })
        .map(|e| e.id.clone())
        .collect();
    to_add.sort();

    let mut to_remove: Vec<String> = cur
        .iter()
        .filter(|e| !nxt_ids.contains(e.id.as_str()))
        .map(|e| e.id.clone())
        .collect();
    to_remove.sort();

    Ok((to_add, to_remove))
}
