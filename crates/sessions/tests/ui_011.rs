use opencode_rk_sessions::ui_011::TrustToggle;

#[test]
fn ui011_t01_default_untrusted() {
    let t = TrustToggle::new();
    assert!(!t.trusted);
    assert!(!t.is_trusted());
}

#[test]
fn ui011_t02_enable() {
    let mut t = TrustToggle::new();
    let msg = t.set(true);
    assert_eq!(msg, "trusted:on");
    assert!(t.is_trusted());
    assert!(t.trusted);
}

#[test]
fn ui011_t03_disable() {
    let mut t = TrustToggle::new();
    t.set(true);
    let msg = t.set(false);
    assert_eq!(msg, "trusted:off");
    assert!(!t.is_trusted());
}

#[test]
fn ui011_t04_message_format() {
    let mut t = TrustToggle::new();
    assert_eq!(t.set(true), "trusted:on");
    assert_eq!(t.set(false), "trusted:off");
}

#[test]
fn ui011_t05_toggle_twice() {
    let mut t = TrustToggle::new();
    t.set(true);
    t.set(false);
    let msg = t.set(true);
    assert_eq!(msg, "trusted:on");
    assert!(t.is_trusted());
}
