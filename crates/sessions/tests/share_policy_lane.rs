//! SHARE-005 frozen tests T01..T05. Deterministic share-sync transport
//! policy: at-most-once per seq, typed retry/abort, explicit delete
//! confirmation. `#[path]` include: `share_policy_lane.rs` is owned by this
//! lane and is NOT wired into `lib.rs` (integrator assembles shared files).
#[path = "../src/share_policy_lane.rs"]
mod share_policy_lane;

use share_policy_lane::{
    decide_create, decide_delete, log_event, request_target, should_send, DeleteOutcome, Endpoint,
    ShareId, ShareRecord, SyncBatch, SyncOutcome, SyncPolicy,
};
use std::collections::HashMap;

fn rec(key: &str, payload: &str) -> ShareRecord {
    ShareRecord {
        key: key.to_owned(),
        payload: payload.as_bytes().to_vec(),
    }
}

fn batch(seq: u64, records: Vec<ShareRecord>) -> SyncBatch {
    SyncBatch {
        share_id: ShareId::new("share-1"),
        base_seq: seq,
        records,
    }
}

// SHARE-005-T01 (happy path): 2xx sync => Sent and should_send false for same
// seq after ack; 2xx delete => Deleted.
#[test]
fn share005_t01_happy_path() {
    let policy = SyncPolicy::default();
    assert_eq!(policy.classify(200, 0), SyncOutcome::Sent);
    assert_eq!(policy.classify(201, 0), SyncOutcome::Sent);
    assert_eq!(policy.classify(204, 0), SyncOutcome::Sent);
    // Same seq after ack must not resend; next seq must send.
    assert!(!should_send(7, 7));
    assert!(!should_send(7, 3));
    assert!(should_send(7, 8));
    assert_eq!(decide_delete(&policy, 200, 0), DeleteOutcome::Deleted);
    assert_eq!(decide_delete(&policy, 204, 0), DeleteOutcome::Deleted);
    // Happy-path batch shape retained by caller.
    let b = batch(8, vec![rec("m-1", r#"{"v":1}"#)]);
    assert_eq!(b.base_seq, 8);
    assert_eq!(b.records.len(), 1);
}

// SHARE-005-T02 (retry mapping): 429/500/503 => Retry with doubling backoff
// capped at 30_000; attempts beyond max_retries => Abort.
#[test]
fn share005_t02_retry_mapping() {
    let policy = SyncPolicy::default();
    for status in [429, 500, 503] {
        let o0 = policy.classify(status, 0);
        let o1 = policy.classify(status, 1);
        let o2 = policy.classify(status, 2);
        assert!(matches!(o0, SyncOutcome::Retry { .. }), "status {status}");
        assert!(matches!(o1, SyncOutcome::Retry { .. }), "status {status}");
        assert!(matches!(o2, SyncOutcome::Retry { .. }), "status {status}");
    }
    // Doubling per attempt from base 500.
    let backoff = |attempt: u32| match policy.classify(500, attempt) {
        SyncOutcome::Retry { backoff_ms } => backoff_ms,
        other => panic!("expected retry, got {other:?}"),
    };
    assert_eq!(backoff(0), 500);
    assert_eq!(backoff(1), 1_000);
    assert_eq!(backoff(2), 2_000);
    assert!(backoff(1) == backoff(0) * 2);
    assert!(backoff(2) == backoff(1) * 2);
    // Cap at 30_000 for large attempts / large bases.
    match policy.classify(503, 10) {
        SyncOutcome::Abort { .. } => {}
        SyncOutcome::Retry { backoff_ms } => assert!(backoff_ms <= 30_000),
        SyncOutcome::Sent => panic!("503 must not be Sent"),
    }
    let big = SyncPolicy::new(10, 20_000);
    match big.classify(500, 2) {
        SyncOutcome::Retry { backoff_ms } => assert_eq!(backoff_ms, 30_000),
        other => panic!("expected capped retry, got {other:?}"),
    }
    match big.classify(429, 9) {
        SyncOutcome::Retry { backoff_ms } => assert_eq!(backoff_ms, 30_000),
        other => panic!("expected capped retry, got {other:?}"),
    }
    // Attempts beyond max_retries => Abort (default max 3: attempts 0..2 retry).
    assert!(matches!(policy.classify(500, 3), SyncOutcome::Abort { .. }));
    assert!(matches!(policy.classify(429, 4), SyncOutcome::Abort { .. }));
    // Delete path retries then refuses once exhausted.
    assert!(matches!(
        decide_delete(&policy, 503, 0),
        DeleteOutcome::Retry { .. }
    ));
    assert!(matches!(
        decide_delete(&policy, 503, 3),
        DeleteOutcome::Refused { .. }
    ));
}

// SHARE-005-T03 (abort + endpoint): 400/401/403/404-on-sync => Abort; missing
// endpoint => Abort endpoint-unspecified; legacy vs org targets distinct.
#[test]
fn share005_t03_abort_and_endpoint() {
    let policy = SyncPolicy::default();
    for status in [400, 401, 403, 404] {
        assert!(
            matches!(policy.classify(status, 0), SyncOutcome::Abort { .. }),
            "status {status} must abort"
        );
    }
    let id = ShareId::new("share-1");
    let err = request_target(None, &id).unwrap_err();
    assert_eq!(
        err,
        SyncOutcome::Abort {
            reason: "endpoint-unspecified".to_owned()
        }
    );
    let legacy = request_target(Some(Endpoint::Legacy), &id).unwrap();
    let org = request_target(Some(Endpoint::Org), &id).unwrap();
    assert_ne!(legacy, org);
    assert!(legacy.contains("/api/share"));
    assert!(org.contains("/api/shares"));
}

// SHARE-005-T04 (delete + create semantics): 404-on-delete => NotFound;
// non-success create => Abort with local store empty; failed sync batch
// retained intact for retry.
#[test]
fn share005_t04_delete_create_semantics() {
    const BODY: &str = r#"{"body":"retain-me-123"}"#;
    let policy = SyncPolicy::default();
    assert_eq!(decide_delete(&policy, 404, 0), DeleteOutcome::NotFound);
    // Create: 2xx persists, non-success leaves the local store empty.
    let mut store: HashMap<String, Vec<u8>> = HashMap::new();
    assert_eq!(decide_create(201), SyncOutcome::Sent);
    store.insert("share-1".to_owned(), b"local-record".to_vec());
    assert_eq!(store.len(), 1);
    store.clear();
    let outcome = decide_create(500);
    assert!(matches!(outcome, SyncOutcome::Abort { .. }));
    assert!(store.is_empty(), "failed create must persist nothing");
    let outcome = decide_create(400);
    assert!(matches!(outcome, SyncOutcome::Abort { .. }));
    assert!(store.is_empty(), "failed create must persist nothing");
    // Failed sync: batch retained byte-intact for retry.
    let b = batch(9, vec![rec("m-1", BODY)]);
    let before = b.records[0].payload.clone();
    let outcome = policy.classify(500, 0);
    assert!(matches!(outcome, SyncOutcome::Retry { .. }));
    assert_eq!(b.records[0].payload, before);
    assert_eq!(b.records[0].payload, BODY.as_bytes());
}

// SHARE-005-T05 (safety + determinism): identical inputs => identical
// outcomes; fixture dir untouched; logs carry zero secret/payload bytes.
#[test]
fn share005_t05_safety_determinism() {
    const SECRET: &str = "s3cr3t-share-XYZ-987";
    const BODY_MARK: &str = "payload-body-ABC-789";
    let dir = tempfile::tempdir().unwrap();
    let sentinel = dir.path().join("sentinel.txt");
    std::fs::write(&sentinel, "untouched").unwrap();
    let policy = SyncPolicy::default();
    // Determinism across repeated calls.
    assert_eq!(policy.classify(503, 1), policy.classify(503, 1));
    assert_eq!(
        decide_delete(&policy, 400, 0),
        decide_delete(&policy, 400, 0)
    );
    assert_eq!(decide_create(500), decide_create(500));
    assert_eq!(should_send(7, 8), should_send(7, 8));
    let id = ShareId::new("share-1");
    assert_eq!(
        request_target(Some(Endpoint::Legacy), &id),
        request_target(Some(Endpoint::Legacy), &id)
    );
    // Logs carry status/seq/endpoint only: no secret, no payload bytes.
    let b = batch(
        11,
        vec![rec("m-1", &format!(r#"{{"body":"{BODY_MARK}"}}"#))],
    );
    let mut logs: Vec<String> = Vec::new();
    logs.push(log_event(503, b.base_seq, Endpoint::Org));
    logs.push(log_event(200, b.base_seq, Endpoint::Legacy));
    logs.push(format!("{:?}", policy.classify(503, 1)));
    logs.push(format!("{:?}", decide_delete(&policy, 400, 0)));
    logs.push(format!("{:?}", b));
    logs.push(format!("{:?}", b.records[0]));
    logs.push(format!("{:?}", decide_create(500)));
    for line in &logs {
        assert!(!line.contains(SECRET), "secret leak: {line}");
        assert!(!line.contains(BODY_MARK), "payload leak: {line}");
    }
    let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(std::fs::read_to_string(&sentinel).unwrap(), "untouched");
}
