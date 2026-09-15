//! WEB-006 T03 fragment FROZEN: production web entry probe (pure, side-effect-free).
//! T01-equiv resolve ok, T02-equiv stale recovery explicit, T03 launch contract,
//! T04 lifecycle bounds, T05 no second listener / no side effects.
//! `#[path]` include: lib.rs untouched, no listener spawn, no network from impl.
#[path = "../src/web_entry_probe.rs"]
mod web_entry_probe;

use web_entry_probe::{
    MAX_HTTP_ORIGIN_LEN, WEB_ENTRY_PATH, DaemonEndpoint, WebEntry, WebEntryError,
    resolve_web_entry,
};

fn live_endpoint() -> DaemonEndpoint {
    DaemonEndpoint {
        pid: std::process::id(),
        http_origin: "http://127.0.0.1:41001".to_string(),
        schema_version: 1,
    }
}

#[test]
fn probe_t01_resolve_ok_and_deterministic() {
    let endpoint = live_endpoint();
    let first = resolve_web_entry(Some(&endpoint), 1).expect("healthy daemon resolves entry");
    let second = resolve_web_entry(Some(&endpoint), 1).expect("resolve repeats exactly");
    assert_eq!(first, second, "probe must be deterministic");
    assert_eq!(first.url, "http://127.0.0.1:41001/");
    assert_eq!(WebEntry::entry_path(), WEB_ENTRY_PATH);
    assert_eq!(WEB_ENTRY_PATH, "/");
}

#[test]
fn probe_t02_missing_entry_explicit_launch_error() {
    let err = resolve_web_entry(None, 1).expect_err("missing daemon must fail explicitly");
    assert!(matches!(err, WebEntryError::MissingDescriptor));
    let message = format!("{err}");
    assert!(
        message.contains("opencode-rk web"),
        "must name the launch command: {message}"
    );
    assert!(
        !message.contains("press any key") && !message.contains("prompt"),
        "failure must be keyboard-independent, never interactive: {message}"
    );
}

#[test]
fn probe_t03_invalid_origin_rejected_without_vite_or_second_server() {
    for bad in [
        "http://localhost:41001",
        "https://127.0.0.1:41001",
        "http://127.0.0.1:41001/",
        "http://127.0.0.1:notaport",
        "http://127.0.0.1:0",
        "http://127.0.0.1:99999",
        "http://127.0.0.1:41001/api",
        "http://127.0.0.1:+80",
        "",
    ] {
        let endpoint = DaemonEndpoint {
            pid: 1234,
            http_origin: bad.to_string(),
            schema_version: 1,
        };
        let err = resolve_web_entry(Some(&endpoint), 1)
            .expect_err(&format!("origin must be rejected: {bad:?}"));
        assert!(
            matches!(err, WebEntryError::InvalidOrigin),
            "origin {bad:?} must be InvalidOrigin, got {err:?}"
        );
        let message = format!("{err}");
        assert!(
            message.contains("127.0.0.1"),
            "error must name the expected origin form: {message}"
        );
    }
}

#[test]
fn probe_t04_stale_and_schema_mismatch_recoverable_explicit() {
    let stale = DaemonEndpoint {
        pid: 0,
        http_origin: "http://127.0.0.1:41001".to_string(),
        schema_version: 1,
    };
    let err =
        resolve_web_entry(Some(&stale), 1).expect_err("ownerless endpoint must fail explicitly");
    assert!(matches!(err, WebEntryError::StaleEndpoint));
    assert!(
        format!("{err}").contains("stale"),
        "stale error must say stale: {err}"
    );

    let mismatched = DaemonEndpoint {
        pid: 1234,
        http_origin: "http://127.0.0.1:41001".to_string(),
        schema_version: 2,
    };
    let err = resolve_web_entry(Some(&mismatched), 1)
        .expect_err("schema mismatch must fail explicitly");
    assert!(
        matches!(
            err,
            WebEntryError::SchemaMismatch {
                expected: 1,
                actual: 2
            }
        ),
        "must carry expected/actual schema: {err:?}"
    );

    let healthy = live_endpoint();
    resolve_web_entry(Some(&healthy), 1).expect("healthy daemon is never replaced");
}

#[test]
fn probe_t05_no_side_effects_and_bounds() {
    let dir = tempfile::tempdir().expect("disposable fixture");
    let sentinel = dir.path().join("sentinel.txt");
    std::fs::write(&sentinel, b"sentinel-bytes").expect("sentinel fixture");
    let before = std::fs::read(&sentinel).expect("read sentinel");

    let endpoint = live_endpoint();
    let _ = resolve_web_entry(Some(&endpoint), 1);
    let _ = resolve_web_entry(None, 1);

    assert_eq!(
        std::fs::read(&sentinel).expect("reread sentinel"),
        before,
        "probe must not touch files"
    );
    assert_eq!(
        std::fs::read_dir(dir.path())
            .expect("list fixture")
            .count(),
        1,
        "probe must create no files"
    );

    let long = format!("http://127.0.0.1:{}", "1".repeat(64));
    assert!(long.len() > MAX_HTTP_ORIGIN_LEN, "fixture exceeds bound");
    let oversized = DaemonEndpoint {
        pid: 1234,
        http_origin: long,
        schema_version: 1,
    };
    let err = resolve_web_entry(Some(&oversized), 1).expect_err("oversized origin must fail");
    assert!(
        matches!(err, WebEntryError::OriginTooLong { .. }),
        "must be OriginTooLong: {err:?}"
    );

    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/web_entry_probe.rs"
    ))
    .expect("read probe source");
    for banned in [
        "TcpListener",
        "UnixListener",
        "UdpSocket",
        "tokio::spawn",
        "std::process",
        "Command",
        "listen(",
        "connect(",
        "std::fs",
        "std::net",
    ] {
        assert!(
            !source.contains(banned),
            "probe source must not spawn/bind/dial: found {banned:?}"
        );
    }
    for secret_word in ["secret", "token", "password"] {
        assert!(
            !source.to_lowercase().contains(secret_word),
            "probe source must hold zero secrets: found {secret_word:?}"
        );
    }
}
