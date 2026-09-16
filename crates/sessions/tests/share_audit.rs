use opencode_rk_sessions::share_audit::{record_audit, AuditError, ShareAudit, MAX_SHARE_AUDIT};

#[test]
fn aud_t01_record() {
    let mut buf: Vec<ShareAudit> = Vec::new();
    record_audit(&mut buf, "alice", "share").unwrap();
    assert_eq!(buf.len(), 1);
    assert_eq!(buf[0].actor, "alice");
    assert_eq!(buf[0].action, "share");
    assert_eq!(buf[0].seq, 1);
}

#[test]
fn aud_t02_empty_actor() {
    let mut buf: Vec<ShareAudit> = Vec::new();
    assert!(matches!(
        record_audit(&mut buf, "", "share"),
        Err(AuditError::EmptyActor)
    ));
    assert!(buf.is_empty());
}

#[test]
fn aud_t03_empty_action() {
    let mut buf: Vec<ShareAudit> = Vec::new();
    assert!(matches!(
        record_audit(&mut buf, "alice", ""),
        Err(AuditError::EmptyAction)
    ));
    assert!(buf.is_empty());
}

#[test]
fn aud_t04_overflow() {
    let mut buf: Vec<ShareAudit> = Vec::with_capacity(MAX_SHARE_AUDIT);
    for i in 0..MAX_SHARE_AUDIT {
        record_audit(&mut buf, "a", &format!("act-{i}")).unwrap();
    }
    let err = record_audit(&mut buf, "a", "one-more").unwrap_err();
    assert!(matches!(
        err,
        AuditError::TooMany { max, actual }
        if max == MAX_SHARE_AUDIT && actual == MAX_SHARE_AUDIT
    ));
    assert_eq!(buf.len(), MAX_SHARE_AUDIT);
}

#[test]
fn aud_t05_seq() {
    let mut buf: Vec<ShareAudit> = Vec::new();
    record_audit(&mut buf, "alice", "one").unwrap();
    record_audit(&mut buf, "bob", "two").unwrap();
    record_audit(&mut buf, "alice", "three").unwrap();
    let seqs: Vec<u64> = buf.iter().map(|e| e.seq).collect();
    assert_eq!(seqs, vec![1, 2, 3]);
    assert_eq!(buf[1].actor, "bob");
    assert_eq!(buf[2].action, "three");
}
