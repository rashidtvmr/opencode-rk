// SYNC-001 contract tests: versioned sync event log with projector replay.
// Maps to obligations SYNC-001-T01..T05 in tasks/SYNC-001.md.
use opencode_rk_server::sync_log::{
    SyncError, SyncEvent, SyncLog, MAX_PROJECTORS, MAX_SYNC_EVENTS,
};
use std::sync::{Arc, Mutex};

fn event(id: &str, kind: &str) -> SyncEvent {
    SyncEvent {
        event_id: id.to_owned(),
        event_type: kind.to_owned(),
        payload: vec![1, 2, 3],
    }
}

#[test]
fn sync001_t01_happy_path() {
    let mut log = SyncLog::new();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let probe = Arc::clone(&seen);
    log.register_projector("session.created", move |e| {
        probe.lock().unwrap().push(e.seq);
    })
    .unwrap();
    assert_eq!(log.append(event("e1", "session.created")).unwrap(), 1);
    assert_eq!(log.append(event("e2", "session.created")).unwrap(), 2);
    assert_eq!(log.append(event("e3", "session.created")).unwrap(), 3);
    assert_eq!(*seen.lock().unwrap(), vec![1, 2, 3]);
    assert_eq!(log.replay(0, 1).unwrap(), 3);
}

#[test]
fn sync001_t02_idempotence_freeze() {
    let mut log = SyncLog::new();
    let count = Arc::new(Mutex::new(0usize));
    let probe = Arc::clone(&count);
    log.register_projector("a", move |_| {
        *probe.lock().unwrap() += 1;
    })
    .unwrap();
    log.append(event("e1", "a")).unwrap();
    log.append(event("e2", "a")).unwrap();
    log.append(event("e3", "a")).unwrap();
    assert_eq!(log.append(event("e1", "a")).unwrap(), 1);
    assert_eq!(log.len(), 3);
    assert_eq!(*count.lock().unwrap(), 3);
    log.freeze();
    assert!(log.is_frozen());
    assert_eq!(
        log.append(event("e9", "brand-new-type")),
        Err(SyncError::UnknownType)
    );
    assert_eq!(log.len(), 3);
    assert_eq!(log.append(event("e4", "a")).unwrap(), 4);
}

#[test]
fn sync001_t03_caps_log_unchanged() {
    let mut log = SyncLog::new();
    log.register_projector("a", |_| {}).unwrap();
    for i in 0..MAX_SYNC_EVENTS {
        log.append(event(&format!("e{i}"), "a")).unwrap();
    }
    assert_eq!(log.len(), MAX_SYNC_EVENTS);
    assert_eq!(log.append(event("overflow", "a")), Err(SyncError::Full));
    assert_eq!(log.len(), MAX_SYNC_EVENTS);
    let mut log2 = SyncLog::new();
    for _ in 0..MAX_PROJECTORS {
        log2.register_projector("a", |_| {}).unwrap();
    }
    assert_eq!(
        log2.register_projector("a", |_| {}),
        Err(SyncError::TooManyProjectors)
    );
    assert_eq!(log2.projector_count(), MAX_PROJECTORS);
}

#[test]
fn sync001_t04_validation_replay_cursor() {
    let mut log = SyncLog::new();
    log.register_projector("a", |_| {}).unwrap();
    log.append(event("e1", "a")).unwrap();
    let before = serde_json::to_vec(&log.events()).unwrap();
    assert_eq!(log.append(event("", "a")), Err(SyncError::InvalidInput));
    let big = SyncEvent {
        event_id: "big".to_owned(),
        event_type: "a".to_owned(),
        payload: vec![0u8; 65 * 1024 + 1],
    };
    assert_eq!(log.append(big), Err(SyncError::InvalidInput));
    assert_eq!(log.replay(99, 1), Err(SyncError::UnknownProjector));
    assert_eq!(log.replay(0, 9999), Err(SyncError::BadCursor));
    assert_eq!(serde_json::to_vec(&log.events()).unwrap(), before);
}

#[test]
fn sync001_t05_purity_safety() {
    let mut log = SyncLog::new();
    let calls = Arc::new(Mutex::new(0usize));
    let probe = Arc::clone(&calls);
    log.register_projector("a", move |_| {
        *probe.lock().unwrap() += 1;
    })
    .unwrap();
    log.append(event("e1", "a")).unwrap();
    log.append(event("e2", "a")).unwrap();
    log.replay(0, 1).unwrap();
    assert_eq!(*calls.lock().unwrap(), 4);
    let dumped = format!("{:?}", log.events());
    assert!(!dumped.contains("secret"));
}
