use opencode_rk_sessions::ui_004::{
    fork_chain, mark_fork, ForkMark, ForkViewError, MAX_FORK_DEPTH, MAX_FORK_MARKS,
};

fn mark(id: &str, fork_of: Option<&str>, depth: u32) -> ForkMark {
    ForkMark {
        session_id: id.to_owned(),
        fork_of: fork_of.map(str::to_owned),
        depth,
    }
}

#[test]
fn ui004_t01_mark_and_chain() {
    let mut marks = Vec::new();
    mark_fork(&mut marks, mark("a", None, 0)).unwrap();
    mark_fork(&mut marks, mark("b", Some("a"), 1)).unwrap();
    mark_fork(&mut marks, mark("c", Some("b"), 2)).unwrap();
    assert_eq!(fork_chain(&marks, "c"), vec!["c", "b", "a"]);
    // same id replaces
    mark_fork(&mut marks, mark("b", Some("a"), 1)).unwrap();
    assert_eq!(marks.len(), 3);
}

#[test]
fn ui004_t02_root_chain_single() {
    let mut marks = Vec::new();
    mark_fork(&mut marks, mark("root", None, 0)).unwrap();
    assert_eq!(fork_chain(&marks, "root"), vec!["root"]);
}

#[test]
fn ui004_t03_empty_rejected() {
    let mut marks = Vec::new();
    assert!(matches!(
        mark_fork(&mut marks, mark("", None, 0)),
        Err(ForkViewError::EmptyId)
    ));
}

#[test]
fn ui004_t04_too_deep_rejected() {
    let mut marks = Vec::new();
    assert!(matches!(
        mark_fork(&mut marks, mark("x", None, MAX_FORK_DEPTH + 1)),
        Err(ForkViewError::TooDeep { .. })
    ));
}

#[test]
fn ui004_t05_overflow_rejected() {
    let mut marks: Vec<ForkMark> = (0..MAX_FORK_MARKS)
        .map(|i| mark(&format!("s{i}"), None, 0))
        .collect();
    assert!(matches!(
        mark_fork(&mut marks, mark("overflow", None, 0)),
        Err(ForkViewError::TooManyForks { .. })
    ));
    // replace at cap still ok
    mark_fork(&mut marks, mark("s0", None, 0)).unwrap();
    assert_eq!(marks.len(), MAX_FORK_MARKS);
}
