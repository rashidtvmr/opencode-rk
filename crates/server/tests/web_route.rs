use opencode_rk_server::web_route::{MAX_WEB_ROUTES, WebRouteError, add_route};

#[test]
fn wrt_t01_add() {
    let mut routes = Vec::new();
    add_route(&mut routes, "/a", "http://localhost:3000").unwrap();
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].path, "/a");
    assert_eq!(routes[0].target, "http://localhost:3000");
}

#[test]
fn wrt_t02_empty_path() {
    let mut routes = Vec::new();
    assert!(matches!(
        add_route(&mut routes, "", "http://localhost:3000"),
        Err(WebRouteError::EmptyPath)
    ));
}

#[test]
fn wrt_t03_empty_target() {
    let mut routes = Vec::new();
    assert!(matches!(
        add_route(&mut routes, "/a", ""),
        Err(WebRouteError::EmptyTarget)
    ));
}

#[test]
fn wrt_t04_dup_updates() {
    let mut routes = Vec::new();
    add_route(&mut routes, "/a", "http://one").unwrap();
    add_route(&mut routes, "/a", "http://two").unwrap();
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].target, "http://two");
}

#[test]
fn wrt_t05_overflow() {
    let mut routes = Vec::new();
    for i in 0..MAX_WEB_ROUTES {
        add_route(&mut routes, &format!("/r{i}"), "http://x").unwrap();
    }
    let err = add_route(&mut routes, "/overflow", "http://x").unwrap_err();
    match err {
        WebRouteError::TooMany { max, actual } => {
            assert_eq!(max, MAX_WEB_ROUTES);
            assert_eq!(actual, MAX_WEB_ROUTES);
        }
        e => panic!("wrong error: {e:?}"),
    }
}
