use opencode_rk_foundation::ops_seq::{next_seq, seq_label};

#[test]
fn seq_t01_next() {
    assert_eq!(next_seq(6), 7);
}

#[test]
fn seq_t02_saturates() {
    assert_eq!(next_seq(u64::MAX), u64::MAX);
}

#[test]
fn seq_t03_label() {
    assert_eq!(seq_label(7), "r0007");
}

#[test]
fn seq_t04_label_pad() {
    assert_eq!(seq_label(12345), "r12345");
}

#[test]
fn seq_t05_zero() {
    assert_eq!(next_seq(0), 1);
    assert_eq!(seq_label(0), "r0000");
}
