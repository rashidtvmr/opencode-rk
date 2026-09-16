use opencode_rk_providers::int_backoff::{backoff_ms, MAX_BACKOFF_MS};

#[test]
fn bof_t01_linear() {
    assert_eq!(MAX_BACKOFF_MS, 60_000);
    assert_eq!(backoff_ms(1, 1_000), 1_000);
    assert_eq!(backoff_ms(3, 1_000), 3_000);
}

#[test]
fn bof_t02_zero_attempt() {
    assert_eq!(backoff_ms(0, 1_000), 0);
    assert_eq!(backoff_ms(0, u64::MAX), 0);
}

#[test]
fn bof_t03_overflow_caps() {
    assert_eq!(backoff_ms(u32::MAX, u64::MAX), MAX_BACKOFF_MS);
    assert_eq!(backoff_ms(u32::MAX, 1), MAX_BACKOFF_MS);
}

#[test]
fn bof_t04_zero_base() {
    assert_eq!(backoff_ms(5, 0), 0);
    assert_eq!(backoff_ms(1, 0), 0);
}

#[test]
fn bof_t05_cap() {
    assert_eq!(backoff_ms(10, 10_000), MAX_BACKOFF_MS);
    assert_eq!(backoff_ms(2, 30_000), MAX_BACKOFF_MS);
    assert_eq!(backoff_ms(1, 60_000), 60_000);
}
