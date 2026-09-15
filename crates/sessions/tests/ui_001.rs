use opencode_rk_sessions::ui_001::{
    SessionListError, SessionListItem, SessionListState, MAX_UI_SESSIONS,
};

fn item(id: &str, title: &str, active: bool) -> SessionListItem {
    SessionListItem {
        id: id.to_owned(),
        title: title.to_owned(),
        active,
    }
}

#[test]
fn ui001_t01_upsert_and_list() {
    let mut s = SessionListState::new();
    assert!(s.list().is_empty());
    s.upsert(item("a", "Alpha", false)).unwrap();
    s.upsert(item("b", "Beta", true)).unwrap();
    let list = s.list();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].id, "a");
    assert_eq!(list[1].id, "b");
}

#[test]
fn ui001_t02_select() {
    let mut s = SessionListState::new();
    s.upsert(item("a", "Alpha", true)).unwrap();
    let found = s.select("a").expect("must find a");
    assert_eq!(found.title, "Alpha");
    assert!(found.active);
    assert!(s.select("missing").is_none());
}

#[test]
fn ui001_t03_replace_same_id() {
    let mut s = SessionListState::new();
    s.upsert(item("a", "Alpha", false)).unwrap();
    s.upsert(item("b", "Beta", false)).unwrap();
    s.upsert(item("a", "Alpha2", true)).unwrap();
    let list = s.list();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].id, "a");
    assert_eq!(list[0].title, "Alpha2");
    assert!(list[0].active);
}

#[test]
fn ui001_t04_empty_rejected() {
    let mut s = SessionListState::new();
    let err = s.upsert(item("", "Nope", false)).unwrap_err();
    assert_eq!(err, SessionListError::EmptyId);
    assert!(s.list().is_empty());
}

#[test]
fn ui001_t05_overflow_rejected() {
    let mut s = SessionListState::new();
    for i in 0..MAX_UI_SESSIONS {
        s.upsert(item(&format!("s{i}"), "t", false)).unwrap();
    }
    let err = s.upsert(item("overflow", "t", false)).unwrap_err();
    assert_eq!(
        err,
        SessionListError::TooManySessions {
            max: MAX_UI_SESSIONS,
            actual: MAX_UI_SESSIONS,
        }
    );
    // replace at capacity still succeeds
    s.upsert(item("s0", "replaced", true)).unwrap();
    assert_eq!(s.list().len(), MAX_UI_SESSIONS);
    assert_eq!(s.select("s0").unwrap().title, "replaced");
}
