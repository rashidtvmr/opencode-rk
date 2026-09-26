#![forbid(unsafe_code)]
//! Sorted string index (std-only).
//!
//! Evidence (TS checkout):
//! - `packages/tui/src/context/sync.tsx:41-52` `search(items, target, key)`
//!   binary search over a sorted array, returning `{ found, index }` where a
//!   miss reports the lower-bound insertion point (`left`).
//!
//! Divergence: TS is generic over `T` with a `key` accessor; std-only here,
//! so this models the `String`-keyed case as a sorted `Vec<String>` using
//! slice `binary_search` (same lower-bound insertion-point semantics).

/// Max retained entries; unique inserts at capacity are refused.
pub const MAX_ITEMS: usize = 5000;

/// Exact-match binary search; `None` when absent.
pub fn find_index(items: &[String], target: &str) -> Option<usize> {
    items
        .binary_search_by(|probe| probe.as_str().cmp(target))
        .ok()
}

/// Insert `target` at its lower-bound index unless already present.
/// Returns the index of `target` afterwards; at capacity a new `target` is
/// refused and `items.len()` is returned unchanged.
pub fn sorted_insert(items: &mut Vec<String>, target: &str) -> usize {
    upsert_sorted(items, target).0
}

/// Insert `target` if absent; returns `(index, inserted_new)`.
/// Duplicates and at-capacity new targets report `inserted_new = false`
/// without growing the vec.
pub fn upsert_sorted(items: &mut Vec<String>, target: &str) -> (usize, bool) {
    match items.binary_search_by(|probe| probe.as_str().cmp(target)) {
        Ok(index) => (index, false),
        Err(index) => {
            // ponytail: refuse at cap; caller evicts first if growth needed.
            if items.len() >= MAX_ITEMS {
                return (items.len(), false);
            }
            items.insert(index, target.to_owned());
            (index, true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_insert_returns_zero() {
        let mut items = Vec::new();
        assert_eq!(sorted_insert(&mut items, "b"), 0);
        assert_eq!(items, vec!["b".to_string()]);
    }

    #[test]
    fn keeps_sorted_order() {
        let mut items = Vec::new();
        for key in ["c", "a", "b"] {
            sorted_insert(&mut items, key);
        }
        assert_eq!(
            items,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn find_present() {
        let items = vec!["a".to_string(), "b".to_string()];
        assert_eq!(find_index(&items, "b"), Some(1));
    }

    #[test]
    fn find_absent_is_none() {
        let items = vec!["a".to_string()];
        assert_eq!(find_index(&items, "z"), None);
        assert_eq!(find_index(&[], "a"), None);
    }

    #[test]
    fn duplicate_index_stable() {
        let mut items = vec!["a".to_string()];
        assert_eq!(upsert_sorted(&mut items, "a"), (0, false));
        assert_eq!(sorted_insert(&mut items, "a"), 0);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn cap_refuses_new() {
        let mut items = vec!["z".to_string(); MAX_ITEMS];
        assert_eq!(upsert_sorted(&mut items, "a"), (MAX_ITEMS, false));
        assert_eq!(sorted_insert(&mut items, "a"), MAX_ITEMS);
        assert_eq!(items.len(), MAX_ITEMS);
    }
}
