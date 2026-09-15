use opencode_rk_tools::plugin_manifest::{ManifestError, PluginManifest, validate_manifest};

fn valid() -> PluginManifest {
    PluginManifest {
        name: "my-plugin".to_string(),
        version: "1.2.3".to_string(),
        permissions: vec!["read".to_string(), "write".to_string()],
    }
}

#[test]
fn manifest_t01_valid_accepted() {
    assert!(validate_manifest(&valid()).is_ok());
}

#[test]
fn manifest_t02_empty_name_rejected() {
    let mut m = valid();
    m.name.clear();
    assert!(matches!(
        validate_manifest(&m),
        Err(ManifestError::EmptyName)
    ));
}

#[test]
fn manifest_t03_bad_version_rejected() {
    for bad in [
        "", "1", "1.2", "1.2.3.4", "a.b.c", "1..3", "1.2.x", "v1.2.3",
    ] {
        let mut m = valid();
        m.version = bad.to_string();
        assert!(
            matches!(validate_manifest(&m), Err(ManifestError::BadVersion)),
            "version {bad:?} must be rejected"
        );
    }
}

#[test]
fn manifest_t04_empty_permission_rejected() {
    let mut m = valid();
    m.permissions.push(String::new());
    assert!(matches!(
        validate_manifest(&m),
        Err(ManifestError::EmptyPermission)
    ));
}

#[test]
fn manifest_t05_overflow_rejected() {
    let mut m = valid();
    m.permissions = (0..33).map(|i| format!("perm-{i}")).collect();
    match validate_manifest(&m) {
        Err(ManifestError::TooManyPermissions { max, actual }) => {
            assert_eq!(max, 32);
            assert_eq!(actual, 33);
        }
        other => panic!("expected TooManyPermissions, got {other:?}"),
    }
}
