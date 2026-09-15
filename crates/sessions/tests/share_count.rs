use opencode_rk_sessions::share_count::count_by_actor;

#[test]
fn shc_t01_counts() {
    let entries = [("alice", "create"), ("bob", "join"), ("alice", "leave")];
    assert_eq!(count_by_actor(&entries, "alice"), 2);
}

#[test]
fn shc_t02_empty_zero() {
    let entries: [(&str, &str); 0] = [];
    assert_eq!(count_by_actor(&entries, "alice"), 0);
    assert_eq!(count_by_actor(&entries, ""), 0);
}

#[test]
fn shc_t03_missing() {
    let entries = [("alice", "create")];
    assert_eq!(count_by_actor(&entries, ""), 0);
}

#[test]
fn shc_t04_all() {
    let entries = [("alice", "a"), ("alice", "b")];
    assert_eq!(count_by_actor(&entries, "alice"), 2);
}

#[test]
fn shc_t05_none() {
    let entries = [("alice", "a"), ("bob", "b")];
    assert_eq!(count_by_actor(&entries, "carol"), 0);
}
