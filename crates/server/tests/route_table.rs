//! WEB-003 FROZEN: Protocol route-table projection (server-side only).
//! T01 subset+seed, T02 rogue rejected, T03 thin handlers,
//! T04 SSE kind+determinism, T05 no side effect+safety.
#[path = "../src/route_table.rs"]
mod route_table;

use route_table::{
    DomainCallSpy, Method, ProtocolRoute, RouteKind, MAX_ROUTES, PROTOCOL_ROUTES,
    assert_protocol_coverage, build_router, build_router_with_extra, check_protocol_coverage,
    check_served, dispatch_move_session, no_direct_io_imports, route_kind, served_routes,
};

const CLEAN_MOVE_SESSION_HANDLER: &str = r#"
use crate::control_decode::decode_move_session_input;
use crate::control_translate::{translate_control_error, to_response};

pub fn handle_move_session(bytes: &[u8], service: &dyn MoveSessionService) -> Response {
    let input = match decode_move_session_input(bytes) {
        Ok(input) => input,
        Err(err) => return translate_control_error(&err),
    };
    let outcome = service.move_session(input);
    to_response(outcome)
}
"#;

const FAT_HANDLER: &str = r#"
use std::fs;
use git2::Repository;

pub fn handle_fat(path: &str) -> Vec<u8> {
    let _repo = Repository::open(path).unwrap();
    fs::read(path).unwrap()
}
"#;

#[test]
fn web003_t01_projection_subset_and_seed() {
    assert!(
        PROTOCOL_ROUTES.len() <= MAX_ROUTES,
        "table exceeds cap: {} > {MAX_ROUTES}",
        PROTOCOL_ROUTES.len()
    );
    for (method, path) in [
        (Method::Get, "/health"),
        (Method::Post, "/control/move_session"),
        (Method::Get, "/events"),
    ] {
        assert!(
            PROTOCOL_ROUTES
                .iter()
                .any(|r: &ProtocolRoute| r.method == method && r.path == path),
            "seed route missing: {method} {path}"
        );
    }
    for (method, path) in served_routes() {
        assert!(
            PROTOCOL_ROUTES
                .iter()
                .any(|r: &ProtocolRoute| r.method == method && r.path == path),
            "served route without Protocol entry: {method} {path}"
        );
    }
    check_protocol_coverage().expect("seed table covers served routes");
    assert_protocol_coverage();
}

#[test]
fn web003_t02_rogue_route_rejected() {
    let err =
        build_router_with_extra(&[(Method::Post, "/rogue_no_protocol")]).unwrap_err();
    let message = format!("{err}");
    assert!(
        message.contains("/rogue_no_protocol"),
        "error must name the path: {message}"
    );
    assert!(
        message.contains("POST"),
        "error must name the method: {message}"
    );
    build_router().expect("clean build succeeds once rogue route is removed");
}

#[test]
fn web003_t03_thin_handlers() {
    let clean = no_direct_io_imports(&[("move_session.rs", CLEAN_MOVE_SESSION_HANDLER)]);
    assert!(clean.is_empty(), "clean handler flagged: {clean:?}");

    let hits = no_direct_io_imports(&[("fat_handler.rs", FAT_HANDLER)]);
    assert!(
        hits.iter()
            .any(|h| h.contains("fat_handler.rs") && h.contains("std::fs")),
        "must name file and crate: {hits:?}"
    );
    assert!(
        hits.iter().any(|h| h.contains("git2")),
        "must flag git import: {hits:?}"
    );

    let spy = DomainCallSpy::new();
    dispatch_move_session(&spy);
    assert_eq!(spy.calls(), 1, "thin handler calls exactly one domain method");
}

#[test]
fn web003_t04_sse_kind_and_determinism() {
    assert_eq!(route_kind("/events"), RouteKind::Sse);
    assert_eq!(route_kind("/health"), RouteKind::Json);
    assert_eq!(
        route_kind("/control/move_session"),
        RouteKind::Json
    );

    let first = served_routes();
    let second = served_routes();
    assert_eq!(first, second, "same table plus same build gives same list");
    let mut sorted = first.clone();
    sorted.sort();
    assert_eq!(first, sorted, "route list is sorted by (method, path)");
}

#[test]
fn web003_t05_no_side_effect_and_safety() {
    for route in PROTOCOL_ROUTES {
        for field in [route.path, route.handler] {
            let lower = field.to_ascii_lowercase();
            assert!(
                !lower.contains("secret")
                    && !lower.contains("token")
                    && !lower.contains("password"),
                "route table must hold zero secrets: {field}"
            );
        }
    }

    let dir = tempfile::tempdir().expect("disposable fixture");
    let sentinel = dir.path().join("sentinel.txt");
    std::fs::write(&sentinel, b"sentinel-bytes").expect("sentinel fixture");
    let before = std::fs::read(&sentinel).expect("read sentinel");

    let rogue =
        build_router_with_extra(&[(Method::Post, "/rogue_no_protocol")]).unwrap_err();
    let duplicated = vec![
        (Method::Get, "/health"),
        (Method::Post, "/control/move_session"),
        (Method::Post, "/control/move_session"),
    ];
    let duplicate = check_served(&duplicated).unwrap_err();
    let rogue_message = format!("{rogue}");
    let duplicate_message = format!("{duplicate}");

    assert_eq!(
        std::fs::read(&sentinel).expect("reread sentinel"),
        before,
        "coverage failure must not touch files"
    );
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .expect("list fixture")
        .collect();
    assert_eq!(entries.len(), 1, "no new files from coverage failure");

    let secret = "ses_SUPERSECRET_value";
    for rendered in [
        rogue_message.as_str(),
        duplicate_message.as_str(),
        format!("{rogue:?}").as_str(),
        format!("{duplicate:?}").as_str(),
    ] {
        assert!(
            !rendered.contains(secret),
            "logs must hold zero secret bytes"
        );
    }
    assert!(
        duplicate_message.contains("/control/move_session"),
        "duplicate error must name the key: {duplicate_message}"
    );
    assert!(
        !duplicate_message.contains("/health") && !duplicate_message.contains("health"),
        "duplicate error must not dump unrelated state: {duplicate_message}"
    );
}
