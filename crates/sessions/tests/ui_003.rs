use opencode_rk_sessions::ui_003::{format_batch, format_ts, TsError, MAX_STAMPS};

#[test]
fn ui003_t01_formats_seconds() {
    assert_eq!(format_ts(1_500_000).unwrap(), "1.500s");
    assert_eq!(format_ts(1_005_000).unwrap(), "1.005s");
    assert_eq!(format_ts(1_000_000).unwrap(), "1.000s");
}

#[test]
fn ui003_t02_zero() {
    assert_eq!(format_ts(0).unwrap(), "0.000s");
}

#[test]
fn ui003_t03_negative_rejected() {
    assert_eq!(format_ts(-1), Err(TsError::NegativeTs));
    assert_eq!(format_batch(&[-1]), Err(TsError::NegativeTs));
}

#[test]
fn ui003_t04_batch_in_order() {
    let out = format_batch(&[0, 1_000_000, 1_500_000]).unwrap();
    assert_eq!(out, vec!["0.000s", "1.000s", "1.500s"]);
}

#[test]
fn ui003_t05_overflow_rejected() {
    let many = vec![0i64; MAX_STAMPS + 1];
    assert_eq!(
        format_batch(&many),
        Err(TsError::TooManyStamps {
            max: MAX_STAMPS,
            actual: MAX_STAMPS + 1
        })
    );
}
