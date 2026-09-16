//! INT-009 project-location context resolution tests (frozen T01..T05).
//!
//! Pure caller-supplied resolution: no filesystem, git, DB, env, or child
//! process access. Module wired via `#[path]` so shared `lib.rs` is untouched.
//! Distinct lane file from `location_ctx.rs` (same slice fallback, no collision).

#[path = "../src/int_location_lane.rs"]
mod int_location_lane;

use int_location_lane::{
    resolve_request, resolve_session, CtxError, DirHint, IdentHints, Identity, RequestCtx,
    SessionPin, VcsInfo, WsHint,
};

fn req(dir: Option<&str>, ws: Option<&str>) -> RequestCtx {
    RequestCtx {
        dir: dir.map(|p| DirHint { path: p.to_owned() }),
        ws: ws.map(|id| WsHint { id: id.to_owned() }),
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
        fields.next();
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
fn int009_t01_request_happy_path_and_precedence() {
    let ctx = req(Some("/a/b"), Some("wrk1"));
    let ids = IdentHints {
        common_dir: Some("common-1".to_owned()),
        root_commit: Some("root-1".to_owned()),
    };
    let vcs = Some(VcsInfo {
        remote: Some("https://example/r".to_owned()),
        branch: None,
    });
    let out = resolve_request(&ctx, "/fallback", vcs, &ids).expect("request resolves");
    assert_eq!(out.directory, "/a/b");
    assert_eq!(out.workspace_id.as_deref(), Some("wrk1"));
    assert_eq!(
        out.identity,
        Identity::Remote("https://example/r".to_owned())
    );
    let vcs_file = Some(VcsInfo {
        remote: Some("file:///x".to_owned()),
        branch: None,
    });
    let out2 = resolve_request(&ctx, "/fallback", vcs_file, &ids).expect("request resolves");
    assert_ne!(
        out2.identity,
        Identity::Remote("file:///x".to_owned()),
        "file remote must not become Remote identity"
    );
    assert_eq!(out2.identity, Identity::CommonDir("common-1".to_owned()));
}

#[test]
fn int009_t02_session_pinning_ignores_request() {
    let pin = SessionPin {
        directory: "/s".to_owned(),
        workspace_id: Some("wrk9".to_owned()),
    };
    let evil = req(Some("/evil"), Some("wrkE"));
    let out = resolve_session(&pin, &evil, None, &IdentHints::default()).expect("session resolves");
    assert_eq!(out.directory, "/s");
    assert_eq!(out.workspace_id.as_deref(), Some("wrk9"));
    let bad = SessionPin {
        directory: "relative/bad".to_owned(),
        workspace_id: None,
    };
    match resolve_session(&bad, &evil, None, &IdentHints::default()) {
        Err(CtxError::InvalidPin) => {}
        other => panic!("expected InvalidPin, got {other:?}"),
    }
}

#[test]
fn int009_t03_fallback_rules_and_input_immutability() {
    let ctx = req(Some(""), Some("bad ws!"));
    let before = ctx.clone();
    let out =
        resolve_request(&ctx, "/dflt", None, &IdentHints::default()).expect("fallback resolves");
    assert_eq!(out.directory, "/dflt");
    assert_eq!(out.workspace_id, None);
    assert_eq!(
        ctx, before,
        "caller inputs must be byte-identical after call"
    );
    let ctx2 = req(Some("/ok"), Some("nope!"));
    let out2 = resolve_request(&ctx2, "/dflt", None, &IdentHints::default()).expect("resolves");
    assert_eq!(out2.directory, "/ok");
    assert_eq!(out2.workspace_id, None);
    let long = format!("/{}", "a".repeat(300));
    let ctx3 = req(Some(long.as_str()), None);
    let out3 = resolve_request(&ctx3, "/dflt", None, &IdentHints::default()).expect("resolves");
    assert_eq!(out3.directory, "/dflt");
    match resolve_request(&ctx, "relative", None, &IdentHints::default()) {
        Err(CtxError::InvalidDefault) => {}
        other => panic!("expected InvalidDefault, got {other:?}"),
    }
}

#[test]
fn int009_t04_identity_precedence_order() {
    let ctx = RequestCtx {
        dir: None,
        ws: None,
    };
    let all = IdentHints {
        common_dir: Some("c1".to_owned()),
        root_commit: Some("r1".to_owned()),
    };
    let vcs = Some(VcsInfo {
        remote: Some("https://example/r".to_owned()),
        branch: Some("main".to_owned()),
    });
    let out = resolve_request(&ctx, "/d", vcs, &all).expect("resolves");
    assert_eq!(
        out.identity,
        Identity::Remote("https://example/r".to_owned())
    );
    let file_vcs = Some(VcsInfo {
        remote: Some("file:///x".to_owned()),
        branch: None,
    });
    let out = resolve_request(&ctx, "/d", file_vcs, &all).expect("resolves");
    assert_eq!(out.identity, Identity::CommonDir("c1".to_owned()));
    let only_root = IdentHints {
        common_dir: None,
        root_commit: Some("r1".to_owned()),
    };
    let out = resolve_request(&ctx, "/d", None, &only_root).expect("resolves");
    assert_eq!(out.identity, Identity::RootCommit("r1".to_owned()));
    let out = resolve_request(&ctx, "/d", None, &IdentHints::default()).expect("resolves");
    assert_eq!(out.identity, Identity::Global);
    let a = resolve_request(&ctx, "/d", None, &only_root).expect("resolves");
    let b = resolve_request(&ctx, "/d", None, &only_root).expect("resolves");
    assert_eq!(a, b);
}

#[test]
fn int009_t05_no_side_effects() {
    let tmp = tempfile::tempdir().expect("disposable dir");
    let snapshot = || {
        let mut names: Vec<String> = std::fs::read_dir(tmp.path())
            .expect("read disposable dir")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    let before = snapshot();
    let before_children = child_pids();
    let before_sockets = socket_count();
    std::env::set_var("INT009_SENTINEL_DIR", "/sentinel");
    let ctx = req(Some("/a/b"), Some("wrk1"));
    let vcs = Some(VcsInfo {
        remote: Some("https://example/r".to_owned()),
        branch: None,
    });
    let ids = IdentHints {
        common_dir: Some("c1".to_owned()),
        root_commit: Some("r1".to_owned()),
    };
    let with_env = resolve_request(&ctx, "/dflt", vcs.clone(), &ids).expect("resolves");
    std::env::remove_var("INT009_SENTINEL_DIR");
    let without_env = resolve_request(&ctx, "/dflt", vcs, &ids).expect("resolves");
    assert_eq!(with_env, without_env, "resolution must not depend on env");
    assert_eq!(
        snapshot(),
        before,
        "no files written outside disposable dir"
    );
    assert_eq!(child_pids(), before_children, "no child process");
    assert_eq!(socket_count(), before_sockets, "no sockets");
    let bad = SessionPin {
        directory: "rel".to_owned(),
        workspace_id: None,
    };
    let err = resolve_session(&bad, &ctx, None, &ids).unwrap_err();
    let msg = format!("{err}");
    assert!(!msg.contains("/a/b"), "logs must not carry directory bytes");
    assert!(!msg.contains("https"), "logs must not carry remote bytes");
    assert!(!msg.contains("wrk1"), "logs must not carry workspace bytes");
}
