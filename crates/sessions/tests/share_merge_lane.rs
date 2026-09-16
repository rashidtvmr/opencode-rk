//! SHARE-001 fallback lane frozen tests T01..T05. Deterministic
//! share-snapshot merge + secret validation. `#[path]` include:
//! `share_merge_lane.rs` is owned by this lane and is NOT wired into
//! `lib.rs` (integrator assembles shared files).
#[path = "../src/share_merge_lane.rs"]
mod share_merge_lane;

use share_merge_lane::{
    apply_lane_sync, merge_lane_records, LaneMergeOutput, LaneRecordKind, LaneShareError,
    LaneShareId, LaneShareRecord, LaneShareSecret, LANE_MAX_BATCHES, LANE_MAX_RECORDS_PER_BATCH,
    LANE_MAX_RECORD_BYTES,
};
use std::sync::atomic::AtomicBool;

fn rec(kind: LaneRecordKind, key: &str, payload: &str) -> LaneShareRecord {
    LaneShareRecord {
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
fn share001_lane_t01_last_write_wins_sorted() {
    let idle = idle();
    let b1 = vec![
        rec(LaneRecordKind::LaneMessage, "m-2", r#"{"v":1}"#),
        rec(LaneRecordKind::LaneSession, "s-1", r#"{"title":"old"}"#),
        rec(LaneRecordKind::LaneModel, "m-1", r#"{"n":"a"}"#),
    ];
    let b2 = vec![
        rec(LaneRecordKind::LaneSession, "s-1", r#"{"title":"new"}"#),
        rec(LaneRecordKind::LanePart, "p-1", r#"{"t":"x"}"#),
    ];
    let out: LaneMergeOutput = merge_lane_records(&[b1.as_slice(), b2.as_slice()], &idle).unwrap();
    assert_eq!(out.records.len(), 4);
    assert_eq!(out.skipped, 0);
    let s1 = out
        .records
        .iter()
        .find(|r| r.kind == LaneRecordKind::LaneSession && r.key == "s-1")
        .unwrap();
    assert_eq!(payload_json(&s1.payload)["title"], "new");
    let keys: Vec<(LaneRecordKind, &str)> = out
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
fn share001_lane_t02_deterministic() {
    let idle = idle();
    let b1 = vec![
        rec(LaneRecordKind::LaneMessage, "b", r#"{"v":1}"#),
        rec(LaneRecordKind::LaneSession, "a", r#"{"title":"old"}"#),
    ];
    let b2 = vec![
        rec(LaneRecordKind::LaneSession, "a", r#"{"title":"new"}"#),
        rec(LaneRecordKind::LaneModel, "z", r#"{"n":"m"}"#),
    ];
    let batches: &[&[LaneShareRecord]] = &[b1.as_slice(), b2.as_slice()];
    let out1 = merge_lane_records(batches, &idle).unwrap();
    let out2 = merge_lane_records(batches, &idle).unwrap();
    assert_eq!(out1, out2);
    let keys1: Vec<(LaneRecordKind, &str)> = out1
        .records
        .iter()
        .map(|r| (r.kind, r.key.as_str()))
        .collect();
    let keys2: Vec<(LaneRecordKind, &str)> = out2
        .records
        .iter()
        .map(|r| (r.kind, r.key.as_str()))
        .collect();
    assert_eq!(keys1, keys2);
}

// SHARE-001-T03: unknown id => NotFound, wrong secret => InvalidSecret,
// correct => Ok; errors and Debug output carry zero secret bytes.
#[test]
fn share001_lane_t03_secret_validation() {
    const SECRET: &str = "s3cr3t-share-XYZ-987";
    let idle = idle();
    let id = LaneShareId::new("share-1");
    let other = LaneShareId::new("share-2");
    let stored = LaneShareSecret::from_str(SECRET);
    let existing = vec![rec(LaneRecordKind::LaneSession, "s-1", r#"{"title":"a"}"#)];
    let incoming = vec![rec(LaneRecordKind::LaneMessage, "m-1", r#"{"v":1}"#)];
    let err = apply_lane_sync(
        None,
        &id,
        &LaneShareSecret::from_str(SECRET),
        &existing,
        &incoming,
        &idle,
    )
    .unwrap_err();
    assert_eq!(err, LaneShareError::NotFound);
    let err = apply_lane_sync(
        Some((&other, &stored)),
        &id,
        &LaneShareSecret::from_str(SECRET),
        &existing,
        &incoming,
        &idle,
    )
    .unwrap_err();
    assert_eq!(err, LaneShareError::NotFound);
    let err = apply_lane_sync(
        Some((&id, &stored)),
        &id,
        &LaneShareSecret::from_str("wrong-secret-value"),
        &existing,
        &incoming,
        &idle,
    )
    .unwrap_err();
    assert_eq!(err, LaneShareError::InvalidSecret);
    let out = apply_lane_sync(
        Some((&id, &stored)),
        &id,
        &LaneShareSecret::from_str(SECRET),
        &existing,
        &incoming,
        &idle,
    )
    .unwrap();
    assert_eq!(out.records.len(), 2);
    for rendered in [
        format!("{}", LaneShareError::NotFound),
        format!("{:?}", LaneShareError::NotFound),
        format!("{}", LaneShareError::InvalidSecret),
        format!("{:?}", LaneShareError::InvalidSecret),
        format!("{:?}", stored),
        format!("{:?}", id),
    ] {
        assert!(!rendered.contains(SECRET), "secret leak: {rendered}");
    }
}

// SHARE-001-T04: over caps => TooLarge; 2 invalid records skipped, valid merged.
#[test]
fn share001_lane_t04_caps_and_invalid_records() {
    let idle = idle();
    assert!(LANE_MAX_BATCHES >= 1 && LANE_MAX_RECORDS_PER_BATCH >= 1 && LANE_MAX_RECORD_BYTES >= 1);
    let big: Vec<LaneShareRecord> = (0..(LANE_MAX_RECORDS_PER_BATCH + 1))
        .map(|i| rec(LaneRecordKind::LaneMessage, &format!("k-{i}"), r#"{"v":1}"#))
        .collect();
    let err = merge_lane_records(&[big.as_slice()], &idle).unwrap_err();
    assert_eq!(err, LaneShareError::TooLarge);
    let many: Vec<Vec<LaneShareRecord>> = (0..(LANE_MAX_BATCHES + 1)).map(|_| Vec::new()).collect();
    let refs: Vec<&[LaneShareRecord]> = many.iter().map(Vec::as_slice).collect();
    let err = merge_lane_records(refs.as_slice(), &idle).unwrap_err();
    assert_eq!(err, LaneShareError::TooLarge);
    let huge = LaneShareRecord {
        kind: LaneRecordKind::LaneSession,
        key: "s-huge".to_owned(),
        payload: vec![b'x'; LANE_MAX_RECORD_BYTES + 1],
    };
    let err = merge_lane_records(&[std::slice::from_ref(&huge)], &idle).unwrap_err();
    assert_eq!(err, LaneShareError::TooLarge);
    let batch = vec![
        rec(LaneRecordKind::LaneSession, "ok-1", r#"{"a":1}"#),
        LaneShareRecord {
            kind: LaneRecordKind::LaneMessage,
            key: String::new(),
            payload: br#"{"a":1}"#.to_vec(),
        },
        LaneShareRecord {
            kind: LaneRecordKind::LanePart,
            key: "bad-json".to_owned(),
            payload: b"not json{{".to_vec(),
        },
        rec(LaneRecordKind::LaneModel, "ok-2", r#"[1,2]"#),
    ];
    let out = merge_lane_records(&[batch.as_slice()], &idle).unwrap();
    assert_eq!(out.records.len(), 2);
    assert_eq!(out.skipped, 2);
}

// SHARE-001-T05: disposable fixture dir untouched, no secret/payload bytes in
// errors or Debug, pre-set cancel => Cancelled.
#[test]
fn share001_lane_t05_safety_purity_cancel() {
    const SECRET: &str = "s3cr3t-share-XYZ-987";
    const BODY_MARK: &str = "payload-body-ABC-789";
    let dir = tempfile::tempdir().unwrap();
    let sentinel = dir.path().join("sentinel.txt");
    std::fs::write(&sentinel, "untouched").unwrap();
    let idle = idle();
    let stored = LaneShareSecret::from_str(SECRET);
    let existing = vec![rec(LaneRecordKind::LaneSession, "s-1", r#"{"title":"t"}"#)];
    let incoming = vec![rec(
        LaneRecordKind::LaneMessage,
        "m-1",
        &format!(r#"{{"body":"{BODY_MARK}"}}"#),
    )];
    let cancel = AtomicBool::new(true);
    let err = merge_lane_records(&[existing.as_slice(), incoming.as_slice()], &cancel).unwrap_err();
    assert_eq!(err, LaneShareError::Cancelled);
    for rendered in [
        format!("{err}"),
        format!("{err:?}"),
        format!("{:?}", stored),
    ] {
        assert!(!rendered.contains(SECRET), "secret leak: {rendered}");
        assert!(!rendered.contains(BODY_MARK), "payload leak: {rendered}");
    }
    // Record Debug renders key + payload length only, never payload bytes.
    let probe = rec(
        LaneRecordKind::LaneMessage,
        "m-1",
        &format!(r#"{{"body":"{BODY_MARK}"}}"#),
    );
    let rendered = format!("{:?}", probe);
    assert!(
        rendered.contains("m-1"),
        "key must stay visible: {rendered}"
    );
    assert!(!rendered.contains(BODY_MARK), "payload leak: {rendered}");
    let out = merge_lane_records(&[existing.as_slice(), incoming.as_slice()], &idle).unwrap();
    assert_eq!(out.records.len(), 2);
    let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(std::fs::read_to_string(&sentinel).unwrap(), "untouched");
}
