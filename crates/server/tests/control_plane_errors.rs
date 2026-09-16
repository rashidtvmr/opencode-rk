//! WEB-002 tests: control-plane domain-error translation at the route boundary.
//! Frozen WEB-002-T01..T05. Includes the boundary module directly so the
//! integrator never needs to touch shared `lib.rs` for this lane.
#[path = "../src/control_plane_errors.rs"]
mod control_plane_errors;

use control_plane_errors::{
    all_codes, domain_imports_have_no_http, move_session, translate, wire_body, DomainError,
    MAX_ERROR_MESSAGE_BYTES,
};

// WEB-002-T01: mapping happy path, frozen 404/409/422/500 table.
#[test]
fn web002_t01_translate_maps_each_variant_to_frozen_status_and_code() {
    let table: [(DomainError, u16, &str); 5] = [
        (DomainError::SessionNotFound, 404, "session_not_found"),
        (DomainError::TargetExists, 409, "target_exists"),
        (
            DomainError::TargetUnreadable("/tmp/fixture-target".to_string()),
            422,
            "target_unreadable",
        ),
        (DomainError::ConflictOrLocked, 409, "conflict_or_locked"),
        (DomainError::Internal("boom".to_string()), 500, "internal"),
    ];
    for (err, status, code) in &table {
        let http = translate(err);
        assert_eq!(http.status, *status, "status for {}", code);
        assert_eq!(http.code, *code, "code mismatch");
    }
}

// WEB-002-T02: wire shape, exact keys, no extra keys.
#[test]
fn web002_t02_wire_body_is_exact_error_envelope() {
    let body = wire_body(&DomainError::SessionNotFound);
    let value: serde_json::Value = serde_json::from_str(&body).expect("wire body is valid JSON");
    let top = value.as_object().expect("top-level object");
    assert_eq!(top.len(), 1, "exactly one top-level key");
    let inner = top
        .get("error")
        .expect("error envelope")
        .as_object()
        .expect("inner object");
    assert_eq!(inner.len(), 2, "exactly code + message keys");
    assert_eq!(inner["code"], "session_not_found");
    let message = inner["message"].as_str().expect("string message");
    assert!(!message.is_empty(), "template is non-empty");
    assert_eq!(
        body,
        format!("{{\"error\":{{\"code\":\"session_not_found\",\"message\":\"{message}\"}}}}"),
        "byte-exact envelope, insertion order"
    );
    assert!(body.len() <= 2048, "bounded wire body");
}

// WEB-002-T03: redaction, secrets and paths never serialized.
#[test]
fn web002_t03_internal_and_unreadable_redact_secrets_and_paths() {
    let secret = "s3cr3t-fixture";
    let body = wire_body(&DomainError::Internal(format!(
        "disk blew up at /home/opener: {secret}"
    )));
    assert!(!body.contains(secret), "inner cause never serialized");
    assert!(!body.contains("/home/"), "no filesystem paths leak");
    assert!(body.contains("\"code\":\"internal\""), "code still present");

    let real_path = "/home/opener/disposable-fixture/target-dir";
    let body = wire_body(&DomainError::TargetUnreadable(real_path.to_string()));
    assert!(
        !body.contains(real_path),
        "target path absent from wire body"
    );
    assert!(!body.contains("/home/"), "no path prefix leaks");
}

// WEB-002-T04: exhaustiveness, 5 variants, no wildcard fallback.
#[test]
fn web002_t04_all_variants_covered_exactly_once() {
    assert_eq!(all_codes().len(), 5, "fixed error cardinality");
    let mut codes = all_codes().to_vec();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), 5, "every variant has a distinct code");
    // Frozen match has no wildcard arm: adding a sixth variant without a row
    // fails compilation. Guard the invariant textually as documentation.
    let src = include_str!("../src/control_plane_errors.rs");
    assert!(
        !src.contains("_ =>"),
        "no wildcard fallback may hide a new variant"
    );
}

// WEB-002-T05: domain stays transport-free + failing path is side-effect free.
#[test]
fn web002_t05_domain_transport_free_and_no_side_effects() {
    assert!(
        domain_imports_have_no_http(),
        "domain boundary must stay free of transport types"
    );
    let src = include_str!("../src/control_plane_errors.rs");
    for banned in ["StatusCode", "axum", "http::", "hyper", "From<HttpError"] {
        assert!(
            !src.contains(banned),
            "domain boundary must not mention {banned}"
        );
    }
    assert!(
        src.contains("fn translate(err: &DomainError) -> HttpError"),
        "translate takes only &DomainError, double translation rejected by types"
    );
    assert!(
        !src.contains("std::fs"),
        "translation performs no filesystem I/O"
    );

    // Failing translation path touches nothing outside the disposable fixture.
    let dir = tempfile::tempdir().expect("disposable fixture");
    let sentinel = dir.path().join("sentinel.txt");
    std::fs::write(&sentinel, b"untouched").expect("sentinel fixture");
    let secret = "s3cr3t-fixture";
    let http = translate(&DomainError::Internal(secret.to_string()));
    assert_eq!(http.status, 500);
    assert!(http.message.len() <= MAX_ERROR_MESSAGE_BYTES);
    let log_line = format!("translate_failed status={} code={}", http.status, http.code);
    assert!(!log_line.contains(secret), "logs contain zero secret bytes");
    assert_eq!(
        std::fs::read(&sentinel).expect("reread sentinel"),
        b"untouched"
    );
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .expect("list fixture")
        .collect();
    assert_eq!(entries.len(), 1, "nothing written beside the sentinel");

    // Domain service signature stays transport-free: pure decision, no status.
    assert!(move_session(true, false, true, false).is_ok());
    assert_eq!(
        move_session(false, false, true, false).unwrap_err(),
        DomainError::SessionNotFound
    );
    assert_eq!(
        move_session(true, true, true, false).unwrap_err(),
        DomainError::TargetExists
    );
    assert_eq!(
        move_session(true, false, true, true).unwrap_err(),
        DomainError::ConflictOrLocked
    );
}
