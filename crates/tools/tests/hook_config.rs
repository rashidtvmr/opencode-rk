use opencode_rk_tools::hook_config::{HookConfigError, MAX_HOOK_CONFIGS, qualify_hooks};

#[test]
fn hkc_t01_valid() {
    let got = qualify_hooks(&[("a", "echo a"), ("b", "echo b")]).unwrap();
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].id, "a");
    assert_eq!(got[0].command, "echo a");
    assert_eq!(got[1].id, "b");
    assert_eq!(got[1].command, "echo b");
}

#[test]
fn hkc_t02_empty_id() {
    assert!(matches!(
        qualify_hooks(&[("", "echo a")]),
        Err(HookConfigError::EmptyId)
    ));
}

#[test]
fn hkc_t03_empty_cmd() {
    assert!(matches!(
        qualify_hooks(&[("a", "")]),
        Err(HookConfigError::EmptyCommand)
    ));
}

#[test]
fn hkc_t04_overflow() {
    let items: Vec<(String, String)> = (0..MAX_HOOK_CONFIGS + 1)
        .map(|i| (format!("h{i}"), format!("cmd {i}")))
        .collect();
    let refs: Vec<(&str, &str)> = items
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    match qualify_hooks(&refs) {
        Err(HookConfigError::TooMany { max, actual }) => {
            assert_eq!(max, MAX_HOOK_CONFIGS);
            assert_eq!(actual, MAX_HOOK_CONFIGS + 1);
        }
        other => panic!("expected TooMany, got {other:?}"),
    }
}

#[test]
fn hkc_t05_order() {
    let ids = ["z", "m", "a"];
    let items: Vec<(&str, &str)> = ids.iter().map(|id| (*id, "run")).collect();
    let got = qualify_hooks(&items).unwrap();
    let order: Vec<&str> = got.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(order, vec!["z", "m", "a"]);
}
