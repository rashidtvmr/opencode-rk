//! SHARE-002 fallback lane frozen tests T01..T05. Bounded coalescing queue
//! with retain-on-failure requeue + finalize lifetime. `#[path]` include:
//! `share_queue_lane.rs` is owned by this lane and is NOT wired into `lib.rs`
//! (integrator assembles shared files).
#[path = "../src/share_queue_lane.rs"]
mod share_queue_lane;

use share_queue_lane::{LaneCoalescingQueue, LaneDataKey, LaneQueueCaps, LaneQueueError, LaneShareEvent};
use std::collections::HashSet;

const CAPS: LaneQueueCaps = LaneQueueCaps {
    max_items: 4096,
    max_bytes: 8_388_608,
    max_sessions: 256,
};

fn ev(session: &str, kind: &str, id: &str, value: &str) -> LaneShareEvent {
    LaneShareEvent {
        session: session.to_owned(),
        key: LaneDataKey {
            kind: kind.to_owned(),
            id: id.to_owned(),
        },
        value: value.as_bytes().to_vec(),
    }
}

fn accept_all(_session: &str) -> bool {
    true
}

// SHARE-002-T01: push 5 events with 2 duplicate keys => len 3, latest value
// wins, drain returns 3 sorted by (session, kind, id), second drain empty.
#[test]
fn share002_lane_t01_coalesce_and_drain() {
    let mut q = LaneCoalescingQueue::new(CAPS, accept_all);
    q.push(ev("s1", "message", "m1", r#"{"v":1}"#)).unwrap();
    q.push(ev("s1", "message", "m2", r#"{"v":1}"#)).unwrap();
    q.push(ev("s1", "part", "p1", r#"{"v":1}"#)).unwrap();
    q.push(ev("s1", "message", "m1", r#"{"v":2}"#)).unwrap();
    q.push(ev("s1", "message", "m2", r#"{"v":2}"#)).unwrap();
    assert_eq!(q.len(), 3);
    let batch = q.drain();
    assert_eq!(batch.len(), 3);
    let keys: Vec<(String, String, String)> = batch
        .iter()
        .map(|e| (e.session.clone(), e.key.kind.clone(), e.key.id.clone()))
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
    for e in &batch {
        if e.key.id == "m1" || e.key.id == "m2" {
            assert_eq!(e.value, br#"{"v":2}"#);
        }
    }
    assert!(q.drain().is_empty());
}

// SHARE-002-T02: same push sequence twice => identical drains;
// non-accepted locations dropped with filtered == 2, absent from drain.
#[test]
fn share002_lane_t02_deterministic_and_filter() {
    let seq = vec![
        ev("s1", "message", "b", r#"{"v":1}"#),
        ev("s2", "session", "a", r#"{"t":"old"}"#),
        ev("s9", "message", "x", r#"{"v":1}"#),
        ev("s8", "part", "y", r#"{"v":1}"#),
    ];
    let run = |seq: &[LaneShareEvent]| -> Vec<LaneShareEvent> {
        let mut q = LaneCoalescingQueue::new(CAPS, |s: &str| s == "s1" || s == "s2");
        for e in seq {
            let _ = q.push(e.clone());
        }
        q.drain()
    };
    let d1 = run(&seq);
    let d2 = run(&seq);
    assert_eq!(d1, d2);
    let mut q = LaneCoalescingQueue::new(CAPS, |s: &str| s == "s1" || s == "s2");
    for e in &seq {
        let _ = q.push(e.clone());
    }
    assert_eq!(q.filtered(), 2);
    let batch = q.drain();
    assert_eq!(batch.len(), 2);
    assert!(batch.iter().all(|e| e.session == "s1" || e.session == "s2"));
}

// SHARE-002-T03: overflow => evicted >= 1, len/bytes within caps;
// oversized single event => TooLarge, queue unchanged.
#[test]
fn share002_lane_t03_bounds() {
    let caps = LaneQueueCaps {
        max_items: 4,
        max_bytes: 256,
        max_sessions: 8,
    };
    let mut q = LaneCoalescingQueue::new(caps, accept_all);
    for i in 0..10 {
        q.push(ev("s1", "message", &format!("m-{i}"), r#"{"v":1}"#))
            .unwrap();
    }
    assert!(q.evicted() >= 1);
    assert!(q.len() <= 4);
    assert!(q.bytes() <= 256);
    let len_before = q.len();
    let bytes_before = q.bytes();
    let huge = LaneShareEvent {
        session: "s1".to_owned(),
        key: LaneDataKey {
            kind: "message".to_owned(),
            id: "huge".to_owned(),
        },
        value: vec![b'x'; 256 / 8 + 1],
    };
    let err = q.push(huge).unwrap_err();
    assert_eq!(err, LaneQueueError::TooLarge);
    assert_eq!(q.len(), len_before);
    assert_eq!(q.bytes(), bytes_before);
}

// SHARE-002-T04: drain then requeue => full batch present again;
// finalize => empty, push => Finalized.
#[test]
fn share002_lane_t04_retry_and_finalize() {
    let mut q = LaneCoalescingQueue::new(CAPS, accept_all);
    q.push(ev("s1", "message", "m1", r#"{"v":1}"#)).unwrap();
    q.push(ev("s2", "part", "p1", r#"{"t":"x"}"#)).unwrap();
    let batch = q.drain();
    assert_eq!(batch.len(), 2);
    assert!(q.drain().is_empty());
    q.requeue(batch.clone()).unwrap();
    let again = q.drain();
    assert_eq!(again.len(), 2);
    let a: HashSet<(String, String, String)> = batch
        .iter()
        .map(|e| (e.session.clone(), e.key.kind.clone(), e.key.id.clone()))
        .collect();
    let b: HashSet<(String, String, String)> = again
        .iter()
        .map(|e| (e.session.clone(), e.key.kind.clone(), e.key.id.clone()))
        .collect();
    assert_eq!(a, b);
    q.finalize();
    assert_eq!(q.len(), 0);
    assert!(q.drain().is_empty());
    let err = q.push(ev("s1", "message", "m9", r#"{"v":1}"#)).unwrap_err();
    assert_eq!(err, LaneQueueError::Finalized);
    let err = q.requeue(batch).unwrap_err();
    assert_eq!(err, LaneQueueError::Finalized);
}

// SHARE-002-T05: disposable fixture dir untouched; captured logs carry zero
// event-value bytes; dropped queue retains nothing observable.
#[test]
fn share002_lane_t05_safety_no_side_effects() {
    const MARK: &str = "queue-body-ABC-789";
    let dir = tempfile::tempdir().unwrap();
    let sentinel = dir.path().join("sentinel.txt");
    std::fs::write(&sentinel, "untouched").unwrap();
    let mut logs: Vec<String> = Vec::new();
    {
        let mut q = LaneCoalescingQueue::new(CAPS, accept_all);
        q.push(ev("s1", "message", "m1", &format!(r#"{{"body":"{MARK}"}}"#)))
            .unwrap();
        q.push(ev("s1", "part", "p1", r#"{"v":1}"#)).unwrap();
        logs.push(format!(
            "len={} bytes={} filtered={} evicted={}",
            q.len(),
            q.bytes(),
            q.filtered(),
            q.evicted()
        ));
        logs.push(format!("{:?}", q.stats()));
        let batch = q.drain();
        logs.push(format!("drained={}", batch.len()));
        for e in &batch {
            logs.push(format!("key={}/{}/{}", e.session, e.key.kind, e.key.id));
        }
    }
    for line in &logs {
        assert!(!line.contains(MARK), "value leak: {line}");
    }
    let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(std::fs::read_to_string(&sentinel).unwrap(), "untouched");
}
