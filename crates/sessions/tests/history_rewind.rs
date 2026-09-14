use opencode_rk_sessions::state::{
    HistoryBoundary, HistoryEntry, HistoryError, SessionHistory,
};

fn entry(id: u64, text: &str) -> HistoryEntry {
    HistoryEntry::new(id, text)
}

fn history() -> SessionHistory {
    SessionHistory::new(vec![
        entry(1, "one"),
        entry(2, "two"),
        entry(3, "three"),
    ])
}

#[test]
fn sess_019_t01_revert_moves_head_without_removing_retained_entries() {
    let mut history = history();
    let retained = history.entries().to_vec();

    history.revert(HistoryBoundary::new(2)).unwrap();

    assert_eq!(history.head(), Some(HistoryBoundary::new(2)));
    assert_eq!(history.entries(), retained.as_slice());
}

#[test]
fn sess_019_t02_rollback_truncates_entries_after_the_boundary() {
    let mut history = history();

    history.rollback(HistoryBoundary::new(2)).unwrap();

    assert_eq!(history.head(), Some(HistoryBoundary::new(2)));
    assert_eq!(history.entries(), &[entry(1, "one"), entry(2, "two")]);
}

#[test]
fn sess_019_t03_invalid_boundaries_fail_without_mutating_history() {
    let mut history = history();
    let before_entries = history.entries().to_vec();
    let before_head = history.head();
    let invalid = HistoryBoundary::new(99);

    assert!(matches!(
        history.revert(invalid),
        Err(HistoryError::InvalidBoundary(boundary)) if boundary == invalid
    ));
    assert_eq!(history.entries(), before_entries.as_slice());
    assert_eq!(history.head(), before_head);

    assert!(matches!(
        history.rollback(invalid),
        Err(HistoryError::InvalidBoundary(boundary)) if boundary == invalid
    ));
    assert_eq!(history.entries(), before_entries.as_slice());
    assert_eq!(history.head(), before_head);
}

#[test]
fn sess_019_t04_revert_and_rollback_are_distinct_operations() {
    let boundary = HistoryBoundary::new(2);
    let mut reverted = history();
    let mut rolled_back = history();

    reverted.revert(boundary).unwrap();
    rolled_back.rollback(boundary).unwrap();

    assert_eq!(reverted.head(), rolled_back.head());
    assert_eq!(reverted.entries().len(), 3);
    assert_eq!(rolled_back.entries().len(), 2);
    assert_ne!(reverted.entries(), rolled_back.entries());
}

#[test]
fn sess_019_t05_append_requires_branch_or_rollback_when_future_history_is_retained() {
    let mut history = history();
    let boundary = HistoryBoundary::new(2);
    let retained = history.entries().to_vec();

    history.revert(boundary).unwrap();
    assert_eq!(
        history.append(entry(4, "four")),
        Err(HistoryError::FutureHistoryRetained)
    );
    assert_eq!(history.entries(), retained.as_slice());
    assert_eq!(history.head(), Some(boundary));

    history.rollback(boundary).unwrap();
    history.append(entry(4, "four")).unwrap();

    assert_eq!(history.head(), Some(HistoryBoundary::new(4)));
    assert_eq!(
        history.entries(),
        &[entry(1, "one"), entry(2, "two"), entry(4, "four")]
    );
}
