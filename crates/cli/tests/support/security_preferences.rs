#![cfg(unix)]
use crate::native_ui_preferences::UiPreferences;
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "oc-ui-pref-isolation-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn symlinked_parent_cannot_redirect_preference_writes() {
    let fixture = Fixture::new();
    let outside = fixture.0.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("sentinel"), b"unchanged").unwrap();
    let state = fixture.0.join("state");
    symlink(&outside, &state).unwrap();
    assert!(UiPreferences::persist_sidebar(&state.join("kv.json"), true).is_err());
    assert!(!outside.join("kv.json").exists());
    assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"unchanged");
}

#[test]
fn symlinked_destination_is_rejected_without_touching_target() {
    let fixture = Fixture::new();
    let target = fixture.0.join("unrelated.json");
    let bytes = br#"{"unrelated": "preserved"}"#;
    fs::write(&target, bytes).unwrap();
    let state = fixture.0.join("kv.json");
    symlink(&target, &state).unwrap();
    assert!(UiPreferences::persist_theme_selection(&state, "opencode", "dark").is_err());
    assert_eq!(fs::read(&target).unwrap(), bytes);
    assert!(fs::symlink_metadata(&state)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn hardlinked_destination_is_rejected_without_replacing_either_link() {
    let fixture = Fixture::new();
    let target = fixture.0.join("unrelated.json");
    let bytes = br#"{"unrelated": true}"#;
    fs::write(&target, bytes).unwrap();
    let state = fixture.0.join("kv.json");
    fs::hard_link(&target, &state).unwrap();
    assert!(UiPreferences::persist_sidebar(&state, true).is_err());
    assert_eq!(fs::read(&target).unwrap(), bytes);
    assert_eq!(fs::read(&state).unwrap(), bytes);
}

#[test]
fn ordinary_updates_preserve_unrelated_state_and_private_mode() {
    let fixture = Fixture::new();
    let state = fixture.0.join("state/kv.json");
    UiPreferences::persist_sidebar(&state, true).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&state).unwrap()).unwrap();
    value["unrelated"] = serde_json::json!({"keep": [1, 2, 3]});
    fs::write(&state, serde_json::to_vec(&value).unwrap()).unwrap();
    UiPreferences::persist_theme_selection(&state, "opencode", "light").unwrap();
    let value: serde_json::Value = serde_json::from_slice(&fs::read(&state).unwrap()).unwrap();
    assert_eq!(value["unrelated"], serde_json::json!({"keep": [1, 2, 3]}));
    assert_eq!(value["sidebar"], "hide");
    assert_eq!(value["theme_mode"], "light");
    assert_eq!(
        fs::metadata(&state).unwrap().permissions().mode() & 0o777,
        0o600
    );
}
