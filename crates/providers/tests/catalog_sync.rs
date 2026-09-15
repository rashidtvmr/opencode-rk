use opencode_rk_providers::catalog_sync::{
    plan_catalog_sync, CatalogEntry, CatalogSyncError, MAX_CATALOG_ENTRIES,
};

fn entry(id: &str, version: u64) -> CatalogEntry {
    CatalogEntry {
        id: id.to_owned(),
        version,
    }
}

#[test]
fn cat_t01_empty() {
    let plan = plan_catalog_sync(&[], &[]).expect("empty snapshots must plan");
    assert!(plan.0.is_empty());
    assert!(plan.1.is_empty());
    let bad = vec![entry("", 1)];
    assert_eq!(
        plan_catalog_sync(&bad, &[]).expect_err("empty id must fail"),
        CatalogSyncError::EmptyId
    );
    assert_eq!(
        plan_catalog_sync(&[], &bad).expect_err("empty id must fail"),
        CatalogSyncError::EmptyId
    );
}

#[test]
fn cat_t02_new_added() {
    let cur = vec![entry("a", 1)];
    let nxt = vec![entry("c", 3), entry("b", 2), entry("a", 1)];
    let (to_add, to_remove) = plan_catalog_sync(&cur, &nxt).expect("new entries must plan");
    assert_eq!(to_add, vec!["b".to_owned(), "c".to_owned()]);
    assert!(to_remove.is_empty());
}

#[test]
fn cat_t03_changed_added() {
    let cur = vec![entry("a", 1)];
    let nxt = vec![entry("a", 2)];
    let (to_add, to_remove) = plan_catalog_sync(&cur, &nxt).expect("changed version must plan");
    assert_eq!(to_add, vec!["a".to_owned()]);
    assert!(to_remove.is_empty());
    let same = vec![entry("a", 1)];
    let (to_add, to_remove) =
        plan_catalog_sync(&same, &same).expect("identical snapshots must plan");
    assert!(to_add.is_empty());
    assert!(to_remove.is_empty());
}

#[test]
fn cat_t04_missing_removed() {
    let cur = vec![entry("c", 3), entry("a", 1), entry("b", 2)];
    let nxt = vec![entry("b", 2)];
    let (to_add, to_remove) = plan_catalog_sync(&cur, &nxt).expect("missing entries must plan");
    assert!(to_add.is_empty());
    assert_eq!(to_remove, vec!["a".to_owned(), "c".to_owned()]);
}

#[test]
fn cat_t05_dup_overflow() {
    let dup_cur = vec![entry("a", 1), entry("a", 1)];
    let nxt = vec![entry("a", 1)];
    assert_eq!(
        plan_catalog_sync(&dup_cur, &nxt).expect_err("cur dup must fail"),
        CatalogSyncError::DuplicateId { id: "a".to_owned() }
    );
    let dup_nxt = vec![entry("b", 1), entry("b", 2)];
    assert_eq!(
        plan_catalog_sync(&nxt, &dup_nxt).expect_err("nxt dup must fail"),
        CatalogSyncError::DuplicateId { id: "b".to_owned() }
    );
    let big: Vec<CatalogEntry> = (0..MAX_CATALOG_ENTRIES + 1)
        .map(|i| entry(&format!("id-{i:04}"), i as u64))
        .collect();
    assert_eq!(
        plan_catalog_sync(&big, &[]).expect_err("cur overflow must fail"),
        CatalogSyncError::TooManyEntries {
            max: MAX_CATALOG_ENTRIES,
            actual: MAX_CATALOG_ENTRIES + 1
        }
    );
    assert_eq!(
        plan_catalog_sync(&[], &big).expect_err("nxt overflow must fail"),
        CatalogSyncError::TooManyEntries {
            max: MAX_CATALOG_ENTRIES,
            actual: MAX_CATALOG_ENTRIES + 1
        }
    );
}
