use opencode_rk_providers::integration_sync::{
    MAX_SYNC_ITEMS, SyncError, SyncItem, plan_sync,
};

fn item(id: &str, rev: u64) -> SyncItem {
    SyncItem {
        id: id.to_owned(),
        rev,
    }
}

#[test]
fn sync_t01_empty_plan() {
    let cur = vec![item("a", 1), item("b", 2)];
    let (fetch, drop) = plan_sync(&cur, &cur).expect("identical snapshots must plan");
    assert!(fetch.is_empty());
    assert!(drop.is_empty());

    let (fetch, drop) = plan_sync(&[], &[]).expect("empty snapshots must plan");
    assert!(fetch.is_empty());
    assert!(drop.is_empty());
}

#[test]
fn sync_t02_new_fetched() {
    let cur = vec![item("a", 1)];
    let nxt = vec![item("c", 3), item("b", 2), item("a", 1)];
    let (fetch, drop) = plan_sync(&cur, &nxt).expect("new entries must be fetched");
    assert_eq!(fetch, vec!["b".to_owned(), "c".to_owned()]);
    assert!(drop.is_empty());
}

#[test]
fn sync_t03_changed_rev_fetched() {
    let cur = vec![item("a", 1)];
    let nxt = vec![item("a", 2)];
    let (fetch, drop) = plan_sync(&cur, &nxt).expect("changed rev must be fetched");
    assert_eq!(fetch, vec!["a".to_owned()]);
    assert!(drop.is_empty());

    let same = vec![item("a", 1)];
    let (fetch, drop) =
        plan_sync(&same, &same).expect("same rev must not fetch");
    assert!(fetch.is_empty());
    assert!(drop.is_empty());
}

#[test]
fn sync_t04_missing_dropped() {
    let cur = vec![item("c", 3), item("a", 1), item("b", 2)];
    let nxt = vec![item("b", 2)];
    let (fetch, drop) = plan_sync(&cur, &nxt).expect("missing entries must drop");
    assert!(fetch.is_empty());
    assert_eq!(drop, vec!["a".to_owned(), "c".to_owned()]);
}

#[test]
fn sync_t05_dup_overflow_rejected() {
    let dup_cur = vec![item("a", 1), item("a", 1)];
    let nxt = vec![item("a", 1)];
    assert_eq!(
        plan_sync(&dup_cur, &nxt).expect_err("cur dup must fail"),
        SyncError::DuplicateId { id: "a".to_owned() }
    );

    let cur = vec![item("a", 1)];
    let dup_nxt = vec![item("b", 1), item("b", 2)];
    assert_eq!(
        plan_sync(&cur, &dup_nxt).expect_err("nxt dup must fail"),
        SyncError::DuplicateId { id: "b".to_owned() }
    );

    let big: Vec<SyncItem> = (0..MAX_SYNC_ITEMS + 1)
        .map(|i| item(&format!("id-{i:04}"), i as u64))
        .collect();
    assert_eq!(
        plan_sync(&big, &[]).expect_err("cur overflow must fail"),
        SyncError::TooManyItems {
            max: MAX_SYNC_ITEMS,
            actual: MAX_SYNC_ITEMS + 1
        }
    );
    assert_eq!(
        plan_sync(&[], &big).expect_err("nxt overflow must fail"),
        SyncError::TooManyItems {
            max: MAX_SYNC_ITEMS,
            actual: MAX_SYNC_ITEMS + 1
        }
    );

    let empty_cur = vec![item("", 1)];
    assert_eq!(
        plan_sync(&empty_cur, &[]).expect_err("empty id must fail"),
        SyncError::EmptyId
    );
    let empty_nxt = vec![item("", 1)];
    assert_eq!(
        plan_sync(&[], &empty_nxt).expect_err("empty id must fail"),
        SyncError::EmptyId
    );
}
