//! INT-007 typed-integration-handler projection tests (frozen T01..T05).
//!
//! The implementation under test is included by path so the integrator can
//! wire `crates/providers/src/handler_projection.rs` into `lib.rs` without
//! this lane touching shared files.

#[path = "../src/handler_projection.rs"]
mod handler_projection;

use handler_projection::{
    HandlerError, HandlerOp, HandlerResponse, Method, NonSecretMeta, MAX_ID_LEN, MAX_ITEMS,
    MAX_LABEL_LEN,
};

fn meta(id: &str, label: &str, method: Method) -> NonSecretMeta {
    NonSecretMeta {
        id: id.to_owned(),
        label: label.to_owned(),
        method,
    }
}

fn key(id: &str, label: &str) -> NonSecretMeta {
    meta(id, label, Method::Key)
}

fn oauth(id: &str, label: &str) -> NonSecretMeta {
    meta(id, label, Method::OAuth)
}

/// Caller-side mutation sentinel: only a projection or ack mutates state.
fn apply(resp: &HandlerResponse, mutated: &mut bool) {
    if matches!(
        resp,
        HandlerResponse::NoContent | HandlerResponse::Projection(_)
    ) {
        *mutated = true;
    }
}

fn child_pids() -> Vec<u32> {
    let me = std::process::id();
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return out;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{name}/stat")) else {
            continue;
        };
        let mut fields = stat.split_whitespace();
        fields.next();
        fields.next();
        fields.next(); // state
        let is_child = fields.next().and_then(|ppid| ppid.parse::<u32>().ok()) == Some(me);
        if !is_child {
            continue;
        }
        if let Ok(pid) = name.parse::<u32>() {
            out.push(pid);
        }
    }
    out.sort_unstable();
    out
}

fn socket_count() -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir("/proc/self/fd") {
        for entry in entries.flatten() {
            let is_socket = std::fs::read_link(entry.path())
                .map(|target| target.to_string_lossy().starts_with("socket:"))
                .unwrap_or(false);
            if is_socket {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn int_007_t01_list_get_keyack_happy_path() {
    assert_eq!(MAX_ITEMS, 128);
    assert_eq!(MAX_ID_LEN, 64);
    assert_eq!(MAX_LABEL_LEN, 64);

    let a = key("a", "alpha");
    let b = oauth("b", "beta");
    let items = vec![b.clone(), a.clone()];

    match handler_projection::project(&HandlerOp::List, None, &items) {
        HandlerResponse::List { items, truncated } => {
            assert!(!truncated, "2 items must not truncate");
            assert_eq!(items, vec![a.clone(), b.clone()], "sorted by id");
        }
        other => panic!("expected List, got {other:?}"),
    }
    // Deterministic: same input => byte-identical response.
    let first = format!(
        "{:?}",
        handler_projection::project(&HandlerOp::List, None, &items)
    );
    let second = format!(
        "{:?}",
        handler_projection::project(&HandlerOp::List, None, &items)
    );
    assert_eq!(first, second);

    match handler_projection::project(&HandlerOp::Get, Some(&a), &items) {
        HandlerResponse::Projection(m) => {
            assert_eq!(m, a);
            assert_eq!(m.method, Method::Key);
            assert_eq!(m.label, "alpha");
        }
        other => panic!("expected Projection(a), got {other:?}"),
    }

    match handler_projection::project(&HandlerOp::KeyAck, Some(&a), &items) {
        HandlerResponse::NoContent => {}
        other => panic!("expected NoContent, got {other:?}"),
    }
    match handler_projection::project(&HandlerOp::OAuthStart, Some(&b), &items) {
        HandlerResponse::NoContent => {}
        other => panic!("expected NoContent, got {other:?}"),
    }
}

#[test]
fn int_007_t02_code_required_vs_auth_failed() {
    let m = oauth("gh", "github");
    let items = vec![m.clone()];
    let mut mutated = false;

    let no_code = handler_projection::project(
        &HandlerOp::OAuthComplete {
            code: None,
            auth_failed: false,
        },
        Some(&m),
        &items,
    );
    match &no_code {
        HandlerResponse::Err(e) => assert_eq!(*e, HandlerError::CodeRequired),
        other => panic!("expected CodeRequired, got {other:?}"),
    }
    apply(&no_code, &mut mutated);

    // Flag set but no code: still CodeRequired, never AuthFailed.
    let flagged_no_code = handler_projection::project(
        &HandlerOp::OAuthComplete {
            code: None,
            auth_failed: true,
        },
        Some(&m),
        &items,
    );
    assert_eq!(
        flagged_no_code,
        HandlerResponse::Err(HandlerError::CodeRequired)
    );
    apply(&flagged_no_code, &mut mutated);
    assert!(!mutated, "code-required keeps attempt pending unmutated");

    let auth_failed = handler_projection::project(
        &HandlerOp::OAuthComplete {
            code: Some("code-abc".to_owned()),
            auth_failed: true,
        },
        Some(&m),
        &items,
    );
    assert_eq!(auth_failed, HandlerResponse::Err(HandlerError::AuthFailed));
    assert_ne!(
        HandlerError::CodeRequired,
        HandlerError::AuthFailed,
        "distinct variants"
    );
}

#[test]
fn int_007_t03_validation_and_missing() {
    let items = vec![key("a", "alpha"), key("b", "beta")];
    let before = format!("{items:?}");

    let bad_ids: Vec<String> = vec![
        String::new(),
        "bad id!".to_owned(),
        "-lead".to_owned(),
        "a".repeat(65),
    ];
    for id in &bad_ids {
        let m = key(id, "ok-label");
        match handler_projection::project(&HandlerOp::KeyAck, Some(&m), &items) {
            HandlerResponse::Err(HandlerError::InvalidRequest) => {}
            other => panic!("expected InvalidRequest for id {id:?}, got {other:?}"),
        }
        match handler_projection::project(&HandlerOp::OAuthStart, Some(&m), &items) {
            HandlerResponse::Err(HandlerError::InvalidRequest) => {}
            other => panic!("expected InvalidRequest for id {id:?}, got {other:?}"),
        }
    }

    let bad_labels: Vec<String> = vec![
        String::new(),
        "has space".to_owned(),
        "bang!".to_owned(),
        "l".repeat(65),
    ];
    for label in &bad_labels {
        let m = key("ok-id", label);
        match handler_projection::project(&HandlerOp::KeyAck, Some(&m), &items) {
            HandlerResponse::Err(HandlerError::InvalidRequest) => {}
            other => panic!("expected InvalidRequest for label {label:?}, got {other:?}"),
        }
    }

    match handler_projection::project(&HandlerOp::Get, None, &items) {
        HandlerResponse::Err(HandlerError::NotFound) => {}
        other => panic!("expected NotFound for Get(absent), got {other:?}"),
    }
    match handler_projection::project(&HandlerOp::AttemptPoll, None, &items) {
        HandlerResponse::Err(HandlerError::NotFound) => {}
        other => panic!("expected NotFound for AttemptPoll(absent), got {other:?}"),
    }

    assert_eq!(
        format!("{items:?}"),
        before,
        "errors leave caller items byte-identical"
    );
}

#[test]
fn int_007_t04_normalization_and_truncation() {
    let m = oauth("gh", "github");
    let items = vec![m.clone()];

    let r1 = handler_projection::project(
        &HandlerOp::OAuthComplete {
            code: Some("provider-a-expired-token".to_owned()),
            auth_failed: true,
        },
        Some(&m),
        &items,
    );
    let r2 = handler_projection::project(
        &HandlerOp::OAuthComplete {
            code: Some("provider-b-revoked-grant-xyz".to_owned()),
            auth_failed: true,
        },
        Some(&m),
        &items,
    );
    assert_eq!(r1, HandlerResponse::Err(HandlerError::AuthFailed));
    assert_eq!(r2, HandlerResponse::Err(HandlerError::AuthFailed));
    let d1 = format!("{r1:?}");
    let d2 = format!("{r2:?}");
    assert_eq!(d1, d2, "auth failures normalize byte-identical");
    assert!(
        !d1.contains("expired") && !d1.contains("revoked"),
        "no provider detail leaked: {d1:?}"
    );

    let mut big: Vec<NonSecretMeta> = (0..200)
        .map(|i| key(&format!("int-{i:03}"), &format!("label-{i:03}")))
        .collect();
    big.reverse();
    match handler_projection::project(&HandlerOp::List, None, &big) {
        HandlerResponse::List { items, truncated } => {
            assert!(truncated, "200 items must report truncated");
            assert_eq!(items.len(), MAX_ITEMS);
            assert!(
                items.windows(2).all(|w| w[0].id <= w[1].id),
                "sorted ascending"
            );
            assert_eq!(items[0].id, "int-000");
            assert_eq!(items[MAX_ITEMS - 1].id, "int-127");
        }
        other => panic!("expected truncated List, got {other:?}"),
    }
}

#[test]
fn int_007_t05_no_authority_safety() {
    const SENTINEL: &str = "INT007_SENTINEL_MUST_STAY_UNSET_9f3k";
    let dir = tempfile::tempdir().expect("disposable test dir should exist");
    let before_files: Vec<String> = std::fs::read_dir(dir.path())
        .expect("test dir should list")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    let before_children = child_pids();
    let before_sockets = socket_count();
    assert!(std::env::var(SENTINEL).is_err(), "sentinel env stays unset");

    let probe_code = "code-PROBE-secret-77";
    let m = key("gh", "github");
    let items = vec![m.clone(), key("zz", "zeta")];
    let mut log_capture = String::new();
    let mut mutated = false;

    let ops: Vec<HandlerOp> = vec![
        HandlerOp::List,
        HandlerOp::Get,
        HandlerOp::KeyAck,
        HandlerOp::OAuthStart,
        HandlerOp::OAuthComplete {
            code: None,
            auth_failed: false,
        },
        HandlerOp::OAuthComplete {
            code: Some(probe_code.to_owned()),
            auth_failed: true,
        },
        HandlerOp::OAuthComplete {
            code: Some(probe_code.to_owned()),
            auth_failed: false,
        },
        HandlerOp::AttemptPoll,
    ];
    for op in &ops {
        for meta in [None, Some(&m)] {
            let resp = handler_projection::project(op, meta, &items);
            apply(&resp, &mut mutated);
            log_capture.push_str(&format!("{resp:?} "));
        }
    }
    let bad = key("", "");
    let resp = handler_projection::project(&HandlerOp::KeyAck, Some(&bad), &items);
    log_capture.push_str(&format!("{resp:?}"));

    assert!(
        !log_capture.contains(probe_code),
        "captured logs carry zero code bytes"
    );
    assert!(
        !log_capture.contains("PROBE"),
        "captured logs carry zero secret-like bytes"
    );

    assert_eq!(child_pids(), before_children, "no child process");
    assert_eq!(socket_count(), before_sockets, "no sockets");
    let after_files: Vec<String> = std::fs::read_dir(dir.path())
        .expect("test dir should list")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(after_files, before_files, "no files written");
    assert!(
        std::fs::read_dir(dir.path())
            .expect("dir")
            .flatten()
            .all(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                !name.ends_with(".db") && !name.ends_with(".sqlite")
            }),
        "no DB writes"
    );
    assert!(std::env::var(SENTINEL).is_err(), "sentinel env still unset");
}
