use opencode_rk_tools::ext_hooks::{
    ExtHook, ExtHookError, ExtHookRegistry, MAX_EXT_HOOKS, valid_events,
};

fn hook(id: &str, event: &str) -> ExtHook {
    ExtHook {
        id: id.to_string(),
        event: event.to_string(),
    }
}

#[test]
fn exth_t01_register_get() {
    assert_eq!(valid_events(), &["pre", "post", "error"]);
    let mut reg = ExtHookRegistry::new();
    reg.register(hook("h1", "pre")).expect("register pre");
    reg.register(hook("h2", "error")).expect("register error");
    assert_eq!(reg.get("h1").unwrap().event, "pre");
    assert_eq!(reg.get("h2").unwrap().event, "error");
    assert!(reg.get("missing").is_none());
    let ids: Vec<&str> = reg.list().iter().map(|h| h.id.as_str()).collect();
    assert_eq!(ids, vec!["h1", "h2"]);
}

#[test]
fn exth_t02_bad_event() {
    let mut reg = ExtHookRegistry::new();
    let err = reg
        .register(hook("h1", "bogus"))
        .expect_err("bad event rejected");
    assert!(matches!(err, ExtHookError::EmptyEvent));
}

#[test]
fn exth_t03_dup_rejected() {
    let mut reg = ExtHookRegistry::new();
    reg.register(hook("h1", "pre")).expect("first ok");
    let err = reg.register(hook("h1", "post")).expect_err("dup rejected");
    assert!(matches!(err, ExtHookError::DuplicateId { .. }));
    if let ExtHookError::DuplicateId { id } = err {
        assert_eq!(id, "h1");
    }
}

#[test]
fn exth_t04_empty_rejected() {
    let mut reg = ExtHookRegistry::new();
    assert!(matches!(
        reg.register(hook("", "pre"))
            .expect_err("empty id rejected"),
        ExtHookError::EmptyId
    ));
    assert!(matches!(
        reg.register(hook("h1", ""))
            .expect_err("empty event rejected"),
        ExtHookError::EmptyEvent
    ));
}

#[test]
fn exth_t05_overflow() {
    let mut reg = ExtHookRegistry::new();
    for i in 0..MAX_EXT_HOOKS {
        reg.register(hook(&format!("h{i}"), "pre"))
            .expect("fill ok");
    }
    match reg.register(hook("overflow", "pre")) {
        Err(ExtHookError::TooManyHooks { max, actual }) => {
            assert_eq!(max, MAX_EXT_HOOKS);
            assert_eq!(actual, MAX_EXT_HOOKS + 1);
        }
        other => panic!("expected TooManyHooks, got {other:?}"),
    }
}
