use opencode_rk_tools::op_receipts::{
    MAX_DETAIL_CHARS, MAX_RECEIPTS, OpReceipt, ReceiptError, ReceiptLog,
};

fn receipt(op: &str, ok: bool, detail: &str) -> OpReceipt {
    OpReceipt {
        op: op.to_string(),
        ok,
        detail: detail.to_string(),
    }
}

#[test]
fn receipts_t01_push_and_list() {
    let mut log = ReceiptLog::new();
    log.push(receipt("op.a", true, "done")).unwrap();
    log.push(receipt("op.b", false, "broke")).unwrap();
    let list = log.list();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].op, "op.a");
    assert!(list[0].ok);
    assert_eq!(list[0].detail, "done");
    assert_eq!(list[1].op, "op.b");
    assert!(!list[1].ok);
}

#[test]
fn receipts_t02_failures_filtered() {
    let mut log = ReceiptLog::new();
    log.push(receipt("ok.op", true, "fine")).unwrap();
    log.push(receipt("bad.a", false, "e1")).unwrap();
    log.push(receipt("bad.b", false, "e2")).unwrap();
    let fails = log.failures();
    assert_eq!(fails.len(), 2);
    assert_eq!(fails[0].op, "bad.a");
    assert_eq!(fails[1].op, "bad.b");
    assert!(fails.iter().all(|r| !r.ok));
}

#[test]
fn receipts_t03_empty_op_rejected() {
    let mut log = ReceiptLog::new();
    let err = log.push(receipt("", true, "x")).unwrap_err();
    assert!(matches!(err, ReceiptError::EmptyOp));
    assert_eq!(log.list().len(), 0);
}

#[test]
fn receipts_t04_long_detail_rejected() {
    let mut log = ReceiptLog::new();
    // chars, not bytes: multibyte char repeated over limit
    let long = "é".repeat(MAX_DETAIL_CHARS + 1);
    assert_eq!(long.chars().count(), MAX_DETAIL_CHARS + 1);
    let err = log.push(receipt("op.x", true, &long)).unwrap_err();
    match err {
        ReceiptError::DetailTooLong { max, actual } => {
            assert_eq!(max, MAX_DETAIL_CHARS);
            assert_eq!(actual, MAX_DETAIL_CHARS + 1);
        }
        other => panic!("expected DetailTooLong, got {other:?}"),
    }
    assert_eq!(log.list().len(), 0);
    // boundary: exactly MAX chars accepted
    let exact = "a".repeat(MAX_DETAIL_CHARS);
    log.push(receipt("op.y", true, &exact)).unwrap();
    assert_eq!(log.list().len(), 1);
}

#[test]
fn receipts_t05_overflow_rejected() {
    let mut log = ReceiptLog::new();
    for i in 0..MAX_RECEIPTS {
        log.push(receipt(&format!("op.{i}"), true, "d")).unwrap();
    }
    assert_eq!(log.list().len(), MAX_RECEIPTS);
    let err = log.push(receipt("op.over", true, "d")).unwrap_err();
    match err {
        ReceiptError::TooManyReceipts { max, actual } => {
            assert_eq!(max, MAX_RECEIPTS);
            assert_eq!(actual, MAX_RECEIPTS);
        }
        other => panic!("expected TooManyReceipts, got {other:?}"),
    }
    assert_eq!(log.list().len(), MAX_RECEIPTS);
}
