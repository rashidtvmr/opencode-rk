use opencode_rk_sessions::remote_share::{
    select_target, RemoteShareError, RemoteTarget, MAX_REMOTE_TARGETS,
};

fn target(name: &str, url: &str) -> RemoteTarget {
    RemoteTarget {
        name: name.to_owned(),
        url: url.to_owned(),
    }
}

#[test]
fn remotesh_t01_selects_url() {
    let ts = vec![
        target("a", "https://a.example"),
        target("b", "https://b.example/x"),
    ];
    assert_eq!(select_target(&ts, "b").unwrap(), "https://b.example/x");
}

#[test]
fn remotesh_t02_empty_name_rejected() {
    // empty entry name
    let ts = vec![target("", "https://a.example")];
    assert!(matches!(
        select_target(&ts, "a"),
        Err(RemoteShareError::EmptyName)
    ));
    // empty selector arg
    let ts = vec![target("a", "https://a.example")];
    assert!(matches!(
        select_target(&ts, ""),
        Err(RemoteShareError::EmptyName)
    ));
}

#[test]
fn remotesh_t03_bad_url_rejected() {
    // empty url -> EmptyUrl
    let ts = vec![target("a", "")];
    assert!(matches!(
        select_target(&ts, "a"),
        Err(RemoteShareError::EmptyUrl)
    ));
    // non-https -> BadUrl
    let ts = vec![target("a", "http://a.example")];
    assert!(matches!(
        select_target(&ts, "a"),
        Err(RemoteShareError::BadUrl)
    ));
}

#[test]
fn remotesh_t04_unknown_is_badurl() {
    // unknown name maps to BadUrl (frozen enum has no Unknown variant)
    let ts = vec![target("a", "https://a.example")];
    assert!(matches!(
        select_target(&ts, "zzz"),
        Err(RemoteShareError::BadUrl)
    ));
}

#[test]
fn remotesh_t05_overflow_rejected() {
    let ts: Vec<RemoteTarget> = (0..MAX_REMOTE_TARGETS + 1)
        .map(|i| target(&format!("n{i}"), "https://a.example"))
        .collect();
    match select_target(&ts, "n0") {
        Err(RemoteShareError::TooManyTargets { max, actual }) => {
            assert_eq!(max, MAX_REMOTE_TARGETS);
            assert_eq!(actual, MAX_REMOTE_TARGETS + 1);
        }
        other => panic!("expected TooManyTargets, got {other:?}"),
    }
}
