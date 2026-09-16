// TOOL-018 contract tests: per-MCP power toggle plus lifecycle machine.
// Maps to obligations TOOL-018-T01..T05 in tasks/TOOL-018.md.
use opencode_rk_tools::mcp_lifecycle::{
    LifecycleError, LifecycleRegistry, LifecycleState, MAX_PERSISTED_BYTES, is_runnable,
};

#[test]
fn tool018_t01_toggle_happy_path() {
    let mut reg = LifecycleRegistry::new();
    reg.add("srv-a").unwrap();
    assert_eq!(
        reg.set_enabled("srv-a", true).unwrap(),
        LifecycleState::Enabled
    );
    assert_eq!(
        reg.mark_starting("srv-a").unwrap(),
        LifecycleState::Starting
    );
    assert!(!is_runnable(&reg, "srv-a"));
    assert_eq!(reg.mark_ready("srv-a").unwrap(), LifecycleState::Ready);
    assert!(is_runnable(&reg, "srv-a"));
    assert_eq!(reg.runnable_ids(), vec!["srv-a".to_string()]);
    assert_eq!(
        reg.set_enabled("srv-a", false).unwrap(),
        LifecycleState::Disabled
    );
    assert!(!is_runnable(&reg, "srv-a"));
    assert!(reg.runnable_ids().is_empty());
    assert!(reg.registered_server_ids().is_empty());
}

#[test]
fn tool018_t02_error_and_reconnect() {
    let mut reg = LifecycleRegistry::new();
    reg.add("srv-a").unwrap();
    reg.set_enabled("srv-a", true).unwrap();
    reg.mark_starting("srv-a").unwrap();
    let state = reg.mark_error("srv-a", "conn-refused").unwrap();
    assert_eq!(
        state,
        LifecycleState::Error {
            code: "conn-refused".to_string()
        }
    );
    assert!(!is_runnable(&reg, "srv-a"));
    assert_eq!(reg.mark_starting("srv-a"), Err(LifecycleError::NotEnabled));
    assert_eq!(
        reg.state("srv-a").unwrap(),
        LifecycleState::Error {
            code: "conn-refused".to_string()
        }
    );
    assert_eq!(reg.reconnect("srv-a").unwrap(), LifecycleState::Enabled);
    assert!(!is_runnable(&reg, "srv-a"));
}

#[test]
fn tool018_t03_transition_guards() {
    let mut reg = LifecycleRegistry::new();
    reg.add("srv-a").unwrap();
    assert_eq!(reg.mark_ready("srv-a"), Err(LifecycleError::NotEnabled));
    assert_eq!(reg.mark_ready("nope"), Err(LifecycleError::Unknown));
    assert_eq!(reg.set_enabled("", true), Err(LifecycleError::EmptyId));
    assert_eq!(reg.add("srv-a"), Err(LifecycleError::Duplicate));
    for i in 1..64 {
        reg.add(format!("srv-{i:02}")).unwrap();
    }
    assert_eq!(reg.len(), 64);
    assert_eq!(reg.add("one-too-many"), Err(LifecycleError::Overflow));
    assert_eq!(reg.len(), 64);
}

#[test]
fn tool018_t04_persistence_round_trip() {
    let mut reg = LifecycleRegistry::new();
    reg.add("srv-a").unwrap();
    reg.add("srv-b").unwrap();
    reg.set_enabled("srv-a", true).unwrap();
    reg.mark_starting("srv-a").unwrap();
    reg.set_enabled("srv-b", true).unwrap();
    reg.mark_starting("srv-b").unwrap();
    reg.mark_error("srv-b", "timeout").unwrap();
    let shape = reg.persist();
    let bytes = shape.to_bytes().unwrap();
    assert!(bytes.len() <= MAX_PERSISTED_BYTES);
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(!text.contains("sk-"));
    let restored = LifecycleRegistry::restore_bytes(&bytes).unwrap();
    assert_eq!(restored.state("srv-a").unwrap(), LifecycleState::Enabled);
    assert!(!restored.is_runnable("srv-a"));
    assert_eq!(
        restored.state("srv-b").unwrap(),
        LifecycleState::Error {
            code: "timeout".to_string()
        }
    );
    assert_eq!(restored.len(), 2);
}

#[test]
fn tool018_t05_determinism_and_isolation() {
    let build = || {
        let mut reg = LifecycleRegistry::new();
        reg.add("srv-b").unwrap();
        reg.add("srv-a").unwrap();
        reg.set_enabled("srv-a", true).unwrap();
        reg
    };
    let a = serde_json::to_vec(&build().list()).unwrap();
    let b = serde_json::to_vec(&build().list()).unwrap();
    assert_eq!(a, b);
    let text = String::from_utf8(a).unwrap();
    assert!(text.contains("srv-a"));
    assert!(!text.contains("secret"));
    let dbg = format!("{:?}", build().list());
    assert!(!dbg.contains("token"));
}
