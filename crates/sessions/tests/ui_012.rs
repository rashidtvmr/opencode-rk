use opencode_rk_sessions::ui_012::{toggle_plugin, PluginPanelError, MAX_PANEL_PLUGINS};

#[test]
fn ui012_t01_add_enabled() {
    let mut rows = Vec::new();
    assert_eq!(toggle_plugin(&mut rows, "foo").unwrap(), true);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "foo");
    assert!(rows[0].enabled);
}

#[test]
fn ui012_t02_toggle_off() {
    let mut rows = Vec::new();
    toggle_plugin(&mut rows, "foo").unwrap();
    assert_eq!(toggle_plugin(&mut rows, "foo").unwrap(), false);
    assert!(!rows[0].enabled);
    assert_eq!(toggle_plugin(&mut rows, "foo").unwrap(), true);
    assert!(rows[0].enabled);
}

#[test]
fn ui012_t03_empty_rejected() {
    let mut rows = Vec::new();
    assert!(matches!(
        toggle_plugin(&mut rows, ""),
        Err(PluginPanelError::EmptyName)
    ));
    assert!(rows.is_empty());
}

#[test]
fn ui012_t04_toggle_missing_adds() {
    let mut rows = Vec::new();
    toggle_plugin(&mut rows, "foo").unwrap();
    assert_eq!(toggle_plugin(&mut rows, "bar").unwrap(), true);
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|r| r.name == "bar" && r.enabled));
}

#[test]
fn ui012_t05_overflow_rejected() {
    let mut rows = Vec::new();
    for i in 0..MAX_PANEL_PLUGINS {
        toggle_plugin(&mut rows, &format!("p{i}")).unwrap();
    }
    assert_eq!(rows.len(), MAX_PANEL_PLUGINS);
    match toggle_plugin(&mut rows, "one-too-many") {
        Err(PluginPanelError::TooManyPlugins { max, actual }) => {
            assert_eq!(max, MAX_PANEL_PLUGINS);
            assert_eq!(actual, MAX_PANEL_PLUGINS);
        }
        other => panic!("expected TooManyPlugins, got {other:?}"),
    }
    assert_eq!(rows.len(), MAX_PANEL_PLUGINS);
}
