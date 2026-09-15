use opencode_rk_providers::int_events::{push_event, IntEvent, IntEventsError, MAX_INT_EVENTS};

#[test]
fn intev_t01_push_seq() {
    let mut buf: Vec<IntEvent> = Vec::new();
    push_event(&mut buf, "started").unwrap();
    assert_eq!(buf.len(), 1);
    assert_eq!(buf[0].kind, "started");
    assert_eq!(buf[0].seq, 1);
}

#[test]
fn intev_t02_empty() {
    let mut buf: Vec<IntEvent> = Vec::new();
    assert!(matches!(
        push_event(&mut buf, ""),
        Err(IntEventsError::EmptyKind)
    ));
    assert!(buf.is_empty());
}

#[test]
fn intev_t03_overflow() {
    let mut buf: Vec<IntEvent> = Vec::new();
    let kinds: Vec<String> = (0..MAX_INT_EVENTS).map(|i| format!("e{i}")).collect();
    for k in &kinds {
        push_event(&mut buf, k).unwrap();
    }
    let err = push_event(&mut buf, "one-too-many").unwrap_err();
    assert!(matches!(
        err,
        IntEventsError::TooManyEvents { max, actual }
        if max == MAX_INT_EVENTS && actual == MAX_INT_EVENTS
    ));
    assert_eq!(buf.len(), MAX_INT_EVENTS);
}

#[test]
fn intev_t04_order() {
    let mut buf: Vec<IntEvent> = Vec::new();
    for kind in ["a", "b", "c"] {
        push_event(&mut buf, kind).unwrap();
    }
    let kinds: Vec<&str> = buf.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(kinds, ["a", "b", "c"]);
}

#[test]
fn intev_t05_seq_increments() {
    let mut buf: Vec<IntEvent> = Vec::new();
    let kinds: Vec<String> = (1..=5u64).map(|i| format!("e{i}")).collect();
    for (idx, k) in kinds.iter().enumerate() {
        push_event(&mut buf, k).unwrap();
        assert_eq!(buf.last().unwrap().seq, idx as u64 + 1);
    }
}
