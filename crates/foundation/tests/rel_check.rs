use opencode_rk_foundation::rel_check::{set_done, RelCheck, RelCheckError};

#[test]
fn rch_t01_add() {
    let mut items = Vec::new();
    let state = set_done(&mut items, "docs", true).unwrap();
    assert!(state);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].item, "docs");
    assert!(items[0].done);
}

#[test]
fn rch_t02_toggle() {
    let mut items = vec![RelCheck {
        item: "docs".to_string(),
        done: true,
    }];
    let state = set_done(&mut items, "docs", false).unwrap();
    assert!(!state);
    assert_eq!(items.len(), 1);
    assert!(!items[0].done);
}

#[test]
fn rch_t03_empty() {
    let mut items = Vec::new();
    assert_eq!(
        set_done(&mut items, "", true),
        Err(RelCheckError::EmptyItem)
    );
    assert_eq!(
        set_done(&mut items, "   ", false),
        Err(RelCheckError::EmptyItem)
    );
    assert!(items.is_empty());
}

#[test]
fn rch_t04_missing_adds() {
    let mut items = vec![RelCheck {
        item: "a".to_string(),
        done: true,
    }];
    let state = set_done(&mut items, "b", true).unwrap();
    assert!(state);
    assert_eq!(items.len(), 2);
    assert_eq!(items[1].item, "b");
    assert!(items[1].done);
}

#[test]
fn rch_t05_state() {
    let mut items = Vec::new();
    assert_eq!(set_done(&mut items, "x", true).unwrap(), true);
    assert_eq!(set_done(&mut items, "x", true).unwrap(), true);
    assert_eq!(set_done(&mut items, "x", false).unwrap(), false);
    assert_eq!(items.len(), 1);
    assert!(!items[0].done);
}
