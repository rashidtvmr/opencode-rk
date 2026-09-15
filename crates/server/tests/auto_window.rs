use opencode_rk_server::auto_window::{in_window, make_window};

#[test]
fn win_t01_day() {
    let w = make_window(9, 17).unwrap();
    assert!(in_window(&w, 9));
    assert!(in_window(&w, 12));
    assert!(!in_window(&w, 8));
    assert!(!in_window(&w, 17));
}

#[test]
fn win_t02_wrap() {
    let w = make_window(22, 3).unwrap();
    assert!(in_window(&w, 22));
    assert!(in_window(&w, 0));
    assert!(in_window(&w, 2));
    assert!(!in_window(&w, 3));
    assert!(!in_window(&w, 12));
}

#[test]
fn win_t03_bad() {
    assert!(make_window(24, 5).is_err());
    assert!(make_window(5, 24).is_err());
}

#[test]
fn win_t04_empty_range() {
    assert!(make_window(5, 5).is_err());
}

#[test]
fn win_t05_edges() {
    let w = make_window(0, 23).unwrap();
    assert!(in_window(&w, 0));
    assert!(in_window(&w, 22));
    assert!(!in_window(&w, 23));
}
