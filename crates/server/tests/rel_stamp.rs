use opencode_rk_server::rel_stamp::{make_stamp, stamp_label, RelStampError};

#[test]
fn rst_t01_valid() {
    let s = make_stamp("v1", 1).unwrap();
    assert_eq!(s.tag, "v1");
    assert_eq!(s.seq, 1);
}

#[test]
fn rst_t02_empty() {
    assert!(matches!(make_stamp("", 1), Err(RelStampError::EmptyTag)));
    assert!(matches!(make_stamp("   ", 1), Err(RelStampError::EmptyTag)));
}

#[test]
fn rst_t03_trims() {
    let s = make_stamp("  v1  ", 2).unwrap();
    assert_eq!(s.tag, "v1");
    assert_eq!(s.seq, 2);
}

#[test]
fn rst_t04_label() {
    let s = make_stamp("v1", 7).unwrap();
    assert_eq!(stamp_label(&s), "v1-0007");
}

#[test]
fn rst_t05_zero() {
    let s = make_stamp("v1", 0).unwrap();
    assert_eq!(stamp_label(&s), "v1-0000");
}
