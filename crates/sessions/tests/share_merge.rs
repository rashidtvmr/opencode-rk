//! SHARE-001 frozen tests T01..T05. Pure deterministic share-snapshot merge +
//! secret validation. `#[path]` include: `share_merge.rs` is owned by this lane
//! and is NOT wired into `lib.rs` (integrator assembles shared files).
#[path = "../src/share_merge.rs"]
mod share_merge;

use share_merge::{
    apply_sync, merge_share_records, MergeOutput, RecordKind, ShareError, ShareId, ShareRecord,
    ShareSecret, MAX_BATCHES, MAX_RECORDS_PER_BATCH, MAX_RECORD_BYTES,
};
use std::sync::atomic::AtomicBool;

fn rec(kind: RecordKind, key: &str, payload: &str) -> ShareRecord {
    ShareRecord {
        kind,
        key: key.to_owned(),
        payload: payload.as_bytes().to_vec(),
    }
}

fn idle() -> AtomicBool {
    AtomicBool::new(false)
}

fn payload_json(payload: &[u8]) -> serde_json::Value {
    serde_json::from_slice(payload).unwrap()
}

// SHARE-001-T01: overlapping keys, later batch wins, output sorted by (kind,key).
#[test]
fn share_merge_t01_last_write_wins_sorted() {
    let idle = idle();
    let b1 = vec![
        rec(RecordKind::Message, "m-2", r#"{"v":1}"#),
        rec(RecordKind::Session, "s-1", r#"{"title":"old"}"#),
        rec(RecordKind::Model, "m-1", r#"{"n":"a"}"#),
    ];
    let b2 = vec![
        rec(RecordKind::Session, "s-1", r#"{"title":"new"}"#),
        rec(RecordKind::Part, "p-1", r#"{"t":"x"}"#),
    ];
    let out: MergeOutput = merge_share_records(&[b1.as_slice(), b2.as_slice()], &idle).unwrap();
    assert_eq!(out.records.len(), 4);
    assert_eq!(out.skipped, 0);
    let s1 = out
        .records
        .iter()
        .find(|r| r.kind == RecordKind::Session && r.key == "s-1")
        .unwrap();
    assert_eq!(payload_json(&s1.payload)["title"], "new");
    let keys: Vec<(RecordKind, &str)> = out
        .records
        .iter()
        .map(|r| (r.kind, r.key.as_str()))
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
}

// SHARE-001-T02: same batches => identical order and value.
#[test]
fn share_merge_t02_deterministic() {
    let idle = idle();
    let b1 = vec![
        rec(RecordKind::Message, "b", r#"{"v":1}"#),
        rec(RecordKind::Session, "a", r#"{"title":"old"}"#),
    ];
    let b2 = vec![
        rec(RecordKind::Session, "a", r#"{"title":"new"}"#),
        rec(RecordKind::Model, "z", r#"{"n":"m"}"#),
    ];
    let batches: &[&[ShareRecord]] = &[b1.as_slice(), b2.as_slice()];
    let out1 = merge_share_records(batches, &idle).unwrap();
    let out2 = merge_share_records(batches, &idle).unwrap();
    assert_eq!(out1, out2);
    let keys1: Vec<(RecordKind, &str)> = out1
        .records
        .iter()
        .map(|r| (r.kind, r.key.as_str()))
        .collect();
    let keys2: Vec<(RecordKind, &str)> = out2
        .records
        .iter()
        .map(|r| (r.kind, r.key.as_str()))
        .collect();
    assert_eq!(keys1, keys2);
}

// SHARE-001-T03: unknown id => NotFound, wrong secret => InvalidSecret,
// correct => Ok; errors and Debug output carry zero secret bytes.
#[test]
fn share_merge_t03_secret_validation() {
    const SECRET: &str = "s3cr3t-share-XYZ-987";
    let idle = idle();
    let id = ShareId::new("share-1");
    let other = ShareId::new("share-2");
    let stored = ShareSecret::from_str(SECRET);
    let existing = vec![rec(RecordKind::Session, "s-1", r#"{"title":"a"}"#)];
    let incoming = vec![rec(RecordKind::Message, "m-1", r#"{"v":1}"#)];
    let err = apply_sync(
        None,
        &id,
        &ShareSecret::from_str(SECRET),
        &existing,
        &incoming,
        &idle,
    )
    .unwrap_err();
    assert_eq!(err, ShareError::NotFound);
    let err = apply_sync(
        Some((&other, &stored)),
        &id,
        &ShareSecret::from_str(SECRET),
        &existing,
        &incoming,
        &idle,
    )
    .unwrap_err();
    assert_eq!(err, ShareError::NotFound);
    let err = apply_sync(
        Some((&id, &stored)),
        &id,
        &ShareSecret::from_str("wrong-secret-value"),
        &existing,
        &incoming,
        &idle,
    )
    .unwrap_err();
    assert_eq!(err, ShareError::InvalidSecret);
    let out = apply_sync(
        Some((&id, &stored)),
        &id,
        &ShareSecret::from_str(SECRET),
        &existing,
        &incoming,
        &idle,
    )
    .unwrap();
    assert_eq!(out.records.len(), 2);
    for rendered in [
        format!("{}", ShareError::NotFound),
        format!("{:?}", ShareError::NotFound),
        format!("{}", ShareError::InvalidSecret),
        format!("{:?}", ShareError::InvalidSecret),
        format!("{:?}", stored),
        format!("{:?}", id),
    ] {
        assert!(!rendered.contains(SECRET), "secret leak: {rendered}");
    }
}

// SHARE-001-T04: over caps => TooLarge; 2 invalid records skipped, valid merged.
#[test]
fn share_merge_t04_caps_and_invalid_records() {
    let idle = idle();
    assert!(MAX_BATCHES >= 1 && MAX_RECORDS_PER_BATCH >= 1 && MAX_RECORD_BYTES >= 1);
    let big: Vec<ShareRecord> = (0..(MAX_RECORDS_PER_BATCH + 1))
        .map(|i| rec(RecordKind::Message, &format!("k-{i}"), r#"{"v":1}"#))
        .collect();
    let err = merge_share_records(&[big.as_slice()], &idle).unwrap_err();
    assert_eq!(err, ShareError::TooLarge);
    let many: Vec<Vec<ShareRecord>> = (0..(MAX_BATCHES + 1)).map(|_| Vec::new()).collect();
    let refs: Vec<&[ShareRecord]> = many.iter().map(Vec::as_slice).collect();
    let err = merge_share_records(refs.as_slice(), &idle).unwrap_err();
    assert_eq!(err, ShareError::TooLarge);
    let huge = ShareRecord {
        kind: RecordKind::Session,
        key: "s-huge".to_owned(),
        payload: vec![b'x'; MAX_RECORD_BYTES + 1],
    };
    let err = merge_share_records(&[std::slice::from_ref(&huge)], &idle).unwrap_err();
    assert_eq!(err, ShareError::TooLarge);
    let batch = vec![
        rec(RecordKind::Session, "ok-1", r#"{"a":1}"#),
        ShareRecord {
            kind: RecordKind::Message,
            key: String::new(),
            payload: br#"{"a":1}"#.to_vec(),
        },
        ShareRecord {
            kind: RecordKind::Part,
            key: "bad-json".to_owned(),
            payload: b"not json{{".to_vec(),
        },
        rec(RecordKind::Model, "ok-2", r#"[1,2]"#),
    ];
    let out = merge_share_records(&[batch.as_slice()], &idle).unwrap();
    assert_eq!(out.records.len(), 2);
    assert_eq!(out.skipped, 2);
}

// SHARE-001-T05: disposable fixture dir untouched, no secret/payload bytes in
// errors or Debug, pre-set cancel => Cancelled.
#[test]
fn share_merge_t05_safety_purity_cancel() {
    const SECRET: &str = "s3cr3t-share-XYZ-987";
    const BODY_MARK: &str = "payload-body-ABC-789";
    let dir = tempfile::tempdir().unwrap();
    let sentinel = dir.path().join("sentinel.txt");
    std::fs::write(&sentinel, "untouched").unwrap();
    let idle = idle();
    let stored = ShareSecret::from_str(SECRET);
    let existing = vec![rec(RecordKind::Session, "s-1", r#"{"title":"t"}"#)];
    let incoming = vec![rec(
        RecordKind::Message,
        "m-1",
        &format!(r#"{{"body":"{BODY_MARK}"}}"#),
    )];
    let cancel = AtomicBool::new(true);
    let err =
        merge_share_records(&[existing.as_slice(), incoming.as_slice()], &cancel).unwrap_err();
    assert_eq!(err, ShareError::Cancelled);
    for rendered in [
        format!("{err}"),
        format!("{err:?}"),
        format!("{:?}", stored),
    ] {
        assert!(!rendered.contains(SECRET), "secret leak: {rendered}");
        assert!(!rendered.contains(BODY_MARK), "payload leak: {rendered}");
    }
    let out = merge_share_records(&[existing.as_slice(), incoming.as_slice()], &idle).unwrap();
    assert_eq!(out.records.len(), 2);
    let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(std::fs::read_to_string(&sentinel).unwrap(), "untouched");
}
