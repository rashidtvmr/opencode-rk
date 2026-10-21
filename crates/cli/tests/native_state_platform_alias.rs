#![cfg(target_os = "macos")]

#[path = "../src/native_box_style.rs"]
mod native_box_style;
#[path = "../src/native_ui_budget.rs"]
mod native_ui_budget;
#[path = "../src/native_ui_preferences.rs"]
mod native_ui_preferences;

use native_ui_preferences::UiPreferences;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
};

struct Fixture {
    root: PathBuf,
    alias: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let canonical = std::env::temp_dir().canonicalize().unwrap();
        assert!(
            canonical.starts_with("/private/var"),
            "macOS test temp directory must use the platform var alias"
        );
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = canonical.join(format!(
            "state-alias-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let alias = PathBuf::from("/").join(root.strip_prefix("/private").unwrap());
        Self { root, alias }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn platform_var_alias_loads_existing_preferences_without_diagnostic() {
    let fixture = Fixture::new();
    fs::write(
        fixture.root.join("kv.json"),
        br#"{"theme":"nord","sidebar":"hide","unrelated":7}"#,
    )
    .unwrap();
    let preferences = UiPreferences::load(&[], &fixture.alias.join("kv.json")).unwrap();
    assert!(
        preferences.diagnostics.is_empty(),
        "{:?}",
        preferences.diagnostics
    );
    assert_eq!(preferences.theme.as_deref(), Some("nord"));
    assert!(preferences.sidebar_hidden);
}

#[test]
fn platform_var_alias_persists_without_replacing_unrelated_preferences() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("kv.json"), br#"{"unrelated":7}"#).unwrap();
    UiPreferences::persist_sidebar(&fixture.alias.join("kv.json"), true).unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.root.join("kv.json")).unwrap()).unwrap();
    assert_eq!(value["unrelated"], 7);
    assert_eq!(value["sidebar"], "hide");
}

#[test]
fn platform_var_alias_creates_missing_owned_state_parent() {
    let fixture = Fixture::new();
    UiPreferences::persist_theme(&fixture.alias.join("state/kv.json"), Some("nord")).unwrap();
    assert_eq!(
        fs::metadata(fixture.root.join("state"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        UiPreferences::load(&[], &fixture.alias.join("state/kv.json"))
            .unwrap()
            .theme
            .as_deref(),
        Some("nord")
    );
}

#[test]
fn platform_alias_does_not_allow_nested_symlink_redirection() {
    let fixture = Fixture::new();
    let outside = fixture.root.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("sentinel"), b"unchanged").unwrap();
    symlink(&outside, fixture.root.join("state")).unwrap();
    assert!(UiPreferences::persist_sidebar(&fixture.alias.join("state/kv.json"), true).is_err());
    assert!(!outside.join("kv.json").exists());
    assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"unchanged");
}

#[test]
fn platform_alias_does_not_allow_hardlinked_state_destination() {
    let fixture = Fixture::new();
    let target = fixture.root.join("target.json");
    let bytes = br#"{"unrelated":7}"#;
    fs::write(&target, bytes).unwrap();
    fs::hard_link(&target, fixture.root.join("kv.json")).unwrap();
    assert!(UiPreferences::persist_sidebar(&fixture.alias.join("kv.json"), true).is_err());
    assert_eq!(fs::read(target).unwrap(), bytes);
}
