use opencode_rk_sessions::ui_013::HelpOverlay;

#[test]
fn ui013_t01_default_hidden() {
    let o = HelpOverlay::new();
    assert!(!o.visible);
    assert!(!o.is_visible());
}

#[test]
fn ui013_t02_show() {
    let mut o = HelpOverlay::new();
    o.show();
    assert!(o.visible);
    assert!(o.is_visible());
}

#[test]
fn ui013_t03_hide() {
    let mut o = HelpOverlay::new();
    o.show();
    o.hide();
    assert!(!o.visible);
    assert!(!o.is_visible());
}

#[test]
fn ui013_t04_toggle_on() {
    let mut o = HelpOverlay::new();
    assert!(o.toggle());
    assert!(o.is_visible());
}

#[test]
fn ui013_t05_toggle_off() {
    let mut o = HelpOverlay::new();
    o.show();
    assert!(!o.toggle());
    assert!(!o.is_visible());
}
