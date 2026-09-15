use opencode_rk_sessions::ui_002::{
    append_msg, latest_n, ThreadError, ThreadMsg, MAX_MSG_CHARS, MAX_THREAD_MSGS,
};

fn msg(id: &str, role: &str, text: &str) -> ThreadMsg {
    ThreadMsg {
        id: id.to_owned(),
        role: role.to_owned(),
        text: text.to_owned(),
    }
}

#[test]
fn ui002_t01_append_and_latest() {
    let mut buf = Vec::new();
    append_msg(&mut buf, msg("a", "user", "hello")).unwrap();
    append_msg(&mut buf, msg("b", "assistant", "world")).unwrap();
    append_msg(&mut buf, msg("c", "system", "sys")).unwrap();
    append_msg(&mut buf, msg("d", "tool", "out")).unwrap();
    assert_eq!(buf.len(), 4);
    let last2 = latest_n(&buf, 2);
    assert_eq!(last2.len(), 2);
    assert_eq!(last2[0].id, "c");
    assert_eq!(last2[1].id, "d");
    assert!(latest_n(&buf, 0).is_empty());
    assert_eq!(latest_n(&buf, 99).len(), 4);
}

#[test]
fn ui002_t02_bad_role_rejected() {
    let mut buf = Vec::new();
    let err = append_msg(&mut buf, msg("a", "human", "hi")).unwrap_err();
    assert!(matches!(err, ThreadError::EmptyRole));
    assert!(buf.is_empty());
}

#[test]
fn ui002_t03_long_text_rejected() {
    let mut buf = Vec::new();
    let long: String = "x".repeat(MAX_MSG_CHARS + 1);
    let err = append_msg(
        &mut buf,
        ThreadMsg {
            id: "a".into(),
            role: "user".into(),
            text: long,
        },
    )
    .unwrap_err();
    match err {
        ThreadError::TextTooLong { max, actual } => {
            assert_eq!(max, MAX_MSG_CHARS);
            assert_eq!(actual, MAX_MSG_CHARS + 1);
        }
        other => panic!("wrong error: {other:?}"),
    }
    // exactly at max is accepted
    let ok: String = "y".repeat(MAX_MSG_CHARS);
    append_msg(
        &mut buf,
        ThreadMsg {
            id: "b".into(),
            role: "user".into(),
            text: ok,
        },
    )
    .unwrap();
    assert_eq!(buf.len(), 1);
}

#[test]
fn ui002_t04_empty_rejected() {
    let mut buf = Vec::new();
    assert!(matches!(
        append_msg(&mut buf, msg("", "user", "hi")).unwrap_err(),
        ThreadError::EmptyId
    ));
    assert!(matches!(
        append_msg(&mut buf, msg("a", "", "hi")).unwrap_err(),
        ThreadError::EmptyRole
    ));
    assert!(buf.is_empty());
}

#[test]
fn ui002_t05_overflow_rejected() {
    let mut buf: Vec<ThreadMsg> = (0..MAX_THREAD_MSGS)
        .map(|i| msg(&format!("m{i}"), "user", "t"))
        .collect();
    assert_eq!(buf.len(), MAX_THREAD_MSGS);
    let err = append_msg(&mut buf, msg("over", "user", "t")).unwrap_err();
    match err {
        ThreadError::TooManyMessages { max, actual } => {
            assert_eq!(max, MAX_THREAD_MSGS);
            assert_eq!(actual, MAX_THREAD_MSGS);
        }
        other => panic!("wrong error: {other:?}"),
    }
    assert_eq!(buf.len(), MAX_THREAD_MSGS);
}
