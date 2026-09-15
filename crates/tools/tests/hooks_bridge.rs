use opencode_rk_tools::hooks_bridge::{HookBridgeError, HookEntry, HookTiming, hooks_for};

fn entry(pattern: &str, timing: HookTiming, id: &str) -> HookEntry {
    HookEntry {
        tool_pattern: pattern.to_string(),
        timing,
        hook_id: id.to_string(),
    }
}

#[test]
fn hooks_t01_exact_match() {
    let hooks = vec![
        entry("read", HookTiming::Pre, "h1"),
        entry("write", HookTiming::Pre, "h2"),
    ];
    let got = hooks_for(&hooks, "read", &HookTiming::Pre).unwrap();
    assert_eq!(got, vec!["h1".to_string()]);
}

#[test]
fn hooks_t02_wildcard_match() {
    let hooks = vec![entry("*", HookTiming::Pre, "any")];
    assert_eq!(
        hooks_for(&hooks, "anything", &HookTiming::Pre).unwrap(),
        vec!["any".to_string()]
    );
}

#[test]
fn hooks_t03_prefix_match() {
    let hooks = vec![entry("fs.*", HookTiming::Post, "p1")];
    assert_eq!(
        hooks_for(&hooks, "fs.read", &HookTiming::Post).unwrap(),
        vec!["p1".to_string()]
    );
    assert!(hooks_for(&hooks, "other", &HookTiming::Post)
        .unwrap()
        .is_empty());
}

#[test]
fn hooks_t04_timing_filtered() {
    let hooks = vec![
        entry("read", HookTiming::Pre, "pre1"),
        entry("read", HookTiming::Post, "post1"),
    ];
    assert_eq!(
        hooks_for(&hooks, "read", &HookTiming::Pre).unwrap(),
        vec!["pre1".to_string()]
    );
    assert_eq!(
        hooks_for(&hooks, "read", &HookTiming::Post).unwrap(),
        vec!["post1".to_string()]
    );
}

#[test]
fn hooks_t05_empty_and_overflow_rejected() {
    let bad_pattern = vec![entry("", HookTiming::Pre, "h1")];
    assert!(matches!(
        hooks_for(&bad_pattern, "read", &HookTiming::Pre),
        Err(HookBridgeError::EmptyPattern)
    ));
    let bad_id = vec![entry("read", HookTiming::Pre, "")];
    assert!(matches!(
        hooks_for(&bad_id, "read", &HookTiming::Pre),
        Err(HookBridgeError::EmptyHookId)
    ));
    let many: Vec<HookEntry> = (0..65)
        .map(|i| entry("read", HookTiming::Pre, &format!("h{i}")))
        .collect();
    match hooks_for(&many, "read", &HookTiming::Pre) {
        Err(HookBridgeError::TooManyHooks { max, actual }) => {
            assert_eq!(max, 64);
            assert_eq!(actual, 65);
        }
        other => panic!("expected TooManyHooks, got {other:?}"),
    }
}
