use opencode_rk_foundation::ops_ping::is_alive;

#[test]
fn png_t01_alive() {
    assert!(is_alive(0, 3));
}

#[test]
fn png_t02_boundary() {
    assert!(is_alive(3, 3));
}

#[test]
fn png_t03_dead() {
    assert!(!is_alive(4, 3));
}

#[test]
fn png_t04_zero_max() {
    assert!(is_alive(0, 0));
    assert!(!is_alive(1, 0));
}

#[test]
fn png_t05_max() {
    assert!(is_alive(0, u64::MAX));
    assert!(is_alive(u64::MAX, u64::MAX));
    assert!(!is_alive(u64::MAX, u64::MAX - 1));
}
