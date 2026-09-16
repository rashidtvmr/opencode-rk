use opencode_rk_tools::ext_enable::{EnableError, ExtEntry, MAX_EXT_ENTRIES, set_enabled};

#[test]
fn ene_t01_add() {
    let mut entries = Vec::new();
    assert_eq!(set_enabled(&mut entries, "a", true), Ok(true));
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0],
        ExtEntry {
            name: "a".to_string(),
            enabled: true
        }
    );
}

#[test]
fn ene_t02_toggle() {
    let mut entries = vec![ExtEntry {
        name: "a".to_string(),
        enabled: true,
    }];
    assert_eq!(set_enabled(&mut entries, "a", false), Ok(false));
    assert_eq!(entries.len(), 1);
    assert!(!entries[0].enabled);
}

#[test]
fn ene_t03_empty() {
    let mut entries = Vec::new();
    assert_eq!(
        set_enabled(&mut entries, "", true),
        Err(EnableError::EmptyName)
    );
    assert!(entries.is_empty());
}

#[test]
fn ene_t04_missing_adds() {
    let mut entries = Vec::new();
    assert_eq!(set_enabled(&mut entries, "b", false), Ok(false));
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0],
        ExtEntry {
            name: "b".to_string(),
            enabled: false
        }
    );
}

#[test]
fn ene_t05_overflow() {
    let mut entries: Vec<ExtEntry> = (0..MAX_EXT_ENTRIES)
        .map(|i| ExtEntry {
            name: format!("ext-{i}"),
            enabled: true,
        })
        .collect();
    assert_eq!(
        set_enabled(&mut entries, "one-more", true),
        Err(EnableError::TooMany {
            max: MAX_EXT_ENTRIES,
            actual: MAX_EXT_ENTRIES
        })
    );
    assert_eq!(entries.len(), MAX_EXT_ENTRIES);
}
