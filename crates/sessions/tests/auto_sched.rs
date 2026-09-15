use opencode_rk_sessions::auto_sched::{MAX_SCHED, SchedEntry, SchedError, add_sched};

#[test]
fn sch_t01_add() {
    let mut list: Vec<SchedEntry> = Vec::new();
    add_sched(&mut list, "nightly", 3600).unwrap();
    add_sched(&mut list, "hourly", 60).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].name, "nightly");
    assert_eq!(list[0].every_secs, 3600);
    assert_eq!(list[1].name, "hourly");
    assert_eq!(list[1].every_secs, 60);
}

#[test]
fn sch_t02_empty() {
    let mut list: Vec<SchedEntry> = Vec::new();
    let err = add_sched(&mut list, "", 10).unwrap_err();
    assert!(matches!(err, SchedError::EmptyName));
    assert!(list.is_empty());
}

#[test]
fn sch_t03_zero() {
    let mut list: Vec<SchedEntry> = Vec::new();
    let err = add_sched(&mut list, "nightly", 0).unwrap_err();
    assert!(matches!(err, SchedError::ZeroInterval));
    assert!(list.is_empty());
}

#[test]
fn sch_t04_dup_updates() {
    let mut list: Vec<SchedEntry> = Vec::new();
    add_sched(&mut list, "nightly", 3600).unwrap();
    add_sched(&mut list, "nightly", 7200).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "nightly");
    assert_eq!(list[0].every_secs, 7200);
}

#[test]
fn sch_t05_overflow() {
    let mut list: Vec<SchedEntry> = Vec::new();
    for i in 0..MAX_SCHED {
        add_sched(&mut list, &format!("job-{i}"), 10).unwrap();
    }
    assert_eq!(list.len(), MAX_SCHED);
    let err = add_sched(&mut list, "one-more", 10).unwrap_err();
    match err {
        SchedError::TooMany { max, actual } => {
            assert_eq!(max, MAX_SCHED);
            assert_eq!(actual, MAX_SCHED);
        }
        other => panic!("expected TooMany, got {other:?}"),
    }
    assert_eq!(list.len(), MAX_SCHED);
}
