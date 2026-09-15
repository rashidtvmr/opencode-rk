//! INT-006 remote MCP transport/auth boundary (frozen T01..T05).
//!
//! TDD RED: fails to compile until `crates/providers/src/mcp_transport.rs`
//! provides the boundary module. Included via path so the integrator can
//! wire `lib.rs` exports without lane races.

#[path = "../src/mcp_transport.rs"]
mod mcp_transport;

use mcp_transport::{
    AuthMode, FakeStep, LoopbackTransport, McpAttachment, McpAuth, McpError, McpTransport,
    TransportKind, HANDSHAKE_TIMEOUT_MS, MAX_HANDSHAKE_BYTES,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

fn sse_none() -> McpAttachment {
    McpAttachment {
        transport: McpTransport::Sse {
            url: "https://mcp.example.com/sse".to_owned(),
        },
        auth: McpAuth::None,
    }
}

fn open_cancel() -> AtomicBool {
    AtomicBool::new(false)
}

#[test]
fn int006_t01_validate_and_handshake_happy_path() {
    let att = sse_none();
    assert!(att.validate().is_ok(), "valid SSE attachment validates Ok");
    let mut t = LoopbackTransport::ok();
    let cancel = open_cancel();
    let sess = att.connect(&mut t, &[], &cancel).expect("handshake ok");
    assert_eq!(sess.endpoint_label, "https://mcp.example.com/sse");
    assert_eq!(sess.auth_mode, AuthMode::None);
    assert_eq!(sess.transport, TransportKind::Sse);
    assert!(!sess.is_closed());
    assert_eq!(t.open_handles(), 1, "session owns exactly one handle");
    sess.close();
    assert_eq!(t.open_handles(), 0, "close reclaims the handle");
    // Drop without explicit close also reclaims.
    let sess2 = att.connect(&mut t, &[], &cancel).expect("second ok");
    assert_eq!(t.open_handles(), 1);
    drop(sess2);
    assert_eq!(t.open_handles(), 0);
}

#[test]
fn int006_t02_transport_auth_matrix_and_scheme_reject() {
    let bearer = "h1-bearer".to_owned();
    let oauth = "h2-oauth".to_owned();
    let known = ["h1-bearer", "h2-oauth"];
    let transports = [
        (
            TransportKind::Sse,
            McpTransport::Sse {
                url: "https://mcp.example.com/sse".to_owned(),
            },
        ),
        (
            TransportKind::WebSocket,
            McpTransport::WebSocket {
                url: "wss://mcp.example.com/ws".to_owned(),
            },
        ),
        (
            TransportKind::Stdio,
            McpTransport::Stdio {
                bin: "/usr/bin/mcp-server".to_owned(),
                argv: vec!["--flag".to_owned()],
            },
        ),
    ];
    let auths = [
        (AuthMode::None, McpAuth::None),
        (
            AuthMode::Bearer,
            McpAuth::BearerHandle {
                handle: bearer.clone(),
            },
        ),
        (
            AuthMode::OAuth,
            McpAuth::OAuthHandle {
                handle: oauth.clone(),
            },
        ),
    ];
    for (want_transport, transport) in &transports {
        for (want_auth, auth) in &auths {
            let att = McpAttachment {
                transport: transport.clone(),
                auth: auth.clone(),
            };
            assert!(
                att.validate().is_ok(),
                "combo {want_transport:?}/{want_auth:?} validates Ok"
            );
            let mut t = LoopbackTransport::ok();
            let cancel = open_cancel();
            let sess = att
                .connect(&mut t, &known, &cancel)
                .unwrap_or_else(|e| panic!("combo {want_transport:?}/{want_auth:?} connects: {e:?}"));
            assert_eq!(sess.transport, *want_transport);
            assert_eq!(sess.auth_mode, *want_auth);
            sess.close();
            assert_eq!(t.open_handles(), 0);
        }
    }
    // Plain http:/ws: (and cross-scheme) rejected before connect.
    let bad = [
        McpTransport::Sse {
            url: "http://mcp.example.com/sse".to_owned(),
        },
        McpTransport::WebSocket {
            url: "ws://mcp.example.com/ws".to_owned(),
        },
        McpTransport::Sse {
            url: "wss://mcp.example.com/sse".to_owned(),
        },
        McpTransport::WebSocket {
            url: "https://mcp.example.com/ws".to_owned(),
        },
        McpTransport::Stdio {
            bin: "relative/mcp-server".to_owned(),
            argv: vec![],
        },
    ];
    for transport in &bad {
        let att = McpAttachment {
            transport: transport.clone(),
            auth: McpAuth::None,
        };
        assert_eq!(att.validate().unwrap_err(), McpError::InvalidEndpoint);
        let mut t = LoopbackTransport::ok();
        let cancel = open_cancel();
        assert_eq!(
            att.connect(&mut t, &[], &cancel).unwrap_err(),
            McpError::InvalidEndpoint
        );
        assert_eq!(t.connect_attempts(), 0, "zero connect attempts");
        assert_eq!(t.open_handles(), 0);
    }
}

#[test]
fn int006_t03_bounds_hold() {
    let att = sse_none();
    let cancel = open_cancel();
    // 65 KiB fake handshake exceeds the 64 KiB cap.
    let big = vec![0x41u8; MAX_HANDSHAKE_BYTES + 1024];
    let mut t = LoopbackTransport::with_script(vec![FakeStep::bytes(big, 1)]);
    assert_eq!(
        att.connect(&mut t, &[], &cancel).unwrap_err(),
        McpError::HandshakeTooLarge
    );
    assert_eq!(t.open_handles(), 0);
    // 33 argv entries exceed the 32 cap.
    let att_argv = McpAttachment {
        transport: McpTransport::Stdio {
            bin: "/usr/bin/mcp-server".to_owned(),
            argv: (0..33).map(|i| format!("arg{i}")).collect(),
        },
        auth: McpAuth::None,
    };
    assert_eq!(
        att_argv.validate().unwrap_err(),
        McpError::InvalidEndpoint
    );
    let mut t = LoopbackTransport::ok();
    assert_eq!(
        att_argv.connect(&mut t, &[], &cancel).unwrap_err(),
        McpError::InvalidEndpoint
    );
    assert_eq!(t.connect_attempts(), 0);
    assert_eq!(t.open_handles(), 0);
    // Oversize single arg (>1 KiB) and oversize url (>2 KiB) rejected.
    let att_bigarg = McpAttachment {
        transport: McpTransport::Stdio {
            bin: "/usr/bin/mcp-server".to_owned(),
            argv: vec!["x".repeat(1025)],
        },
        auth: McpAuth::None,
    };
    assert_eq!(
        att_bigarg.validate().unwrap_err(),
        McpError::InvalidEndpoint
    );
    let long_url = format!("https://{}", "a".repeat(2049 - "https://".len()));
    assert_eq!(long_url.len(), 2049);
    let att_long = McpAttachment {
        transport: McpTransport::Sse { url: long_url },
        auth: McpAuth::None,
    };
    assert_eq!(att_long.validate().unwrap_err(), McpError::InvalidEndpoint);
    // Slow fake exceeding 10 s on the virtual clock, no wall sleep.
    assert!(HANDSHAKE_TIMEOUT_MS == 10_000);
    let mut t = LoopbackTransport::with_script(vec![
        FakeStep::bytes(vec![0x42u8; 16], 6_000),
        FakeStep::bytes(vec![0x43u8; 16], 6_000),
    ]);
    let start = Instant::now();
    assert_eq!(
        att.connect(&mut t, &[], &cancel).unwrap_err(),
        McpError::HandshakeTimeout
    );
    assert!(start.elapsed() < Duration::from_secs(1), "no wall sleep");
    assert_eq!(t.open_handles(), 0);
}

#[test]
fn int006_t04_failure_and_cancel_reclaim() {
    let att = sse_none();
    // Pre-set cancel wins promptly even on a large fixture.
    let big = vec![0x41u8; MAX_HANDSHAKE_BYTES + 1024];
    let mut t = LoopbackTransport::with_script(vec![FakeStep::bytes(big, 1)]);
    let cancelled = AtomicBool::new(true);
    let start = Instant::now();
    assert_eq!(
        att.connect(&mut t, &[], &cancelled).unwrap_err(),
        McpError::Cancelled
    );
    assert!(start.elapsed() < Duration::from_secs(1));
    assert_eq!(t.connect_attempts(), 0);
    assert_eq!(t.open_handles(), 0);
    // Mid-handshake remote close.
    let mut t = LoopbackTransport::with_script(vec![
        FakeStep::bytes(vec![0x42u8; 16], 1),
        FakeStep::CLOSED,
    ]);
    let cancel = open_cancel();
    assert_eq!(
        att.connect(&mut t, &[], &cancel).unwrap_err(),
        McpError::TransportClosed
    );
    assert_eq!(t.open_handles(), 0);
    // Unknown auth handle rejected without opening anything.
    let att_bearer = McpAttachment {
        transport: McpTransport::Sse {
            url: "https://mcp.example.com/sse".to_owned(),
        },
        auth: McpAuth::BearerHandle {
            handle: "unknown-handle".to_owned(),
        },
    };
    let mut t = LoopbackTransport::ok();
    assert_eq!(
        att_bearer
            .connect(&mut t, &["known-handle"], &cancel)
            .unwrap_err(),
        McpError::AuthRejected
    );
    assert_eq!(t.connect_attempts(), 0);
    assert_eq!(t.open_handles(), 0);
    // Cancel flag visibility helper is exercised (no unused-import drift).
    assert!(!cancel.load(Ordering::SeqCst));
}

#[test]
fn int006_t05_no_side_effects_and_redaction() {
    // Validation performs zero I/O: disposable fixture dir stays empty.
    let dir = tempfile::tempdir().expect("fixture dir");
    let count = || {
        std::fs::read_dir(dir.path())
            .expect("read fixture dir")
            .count()
    };
    assert_eq!(count(), 0);
    let handle = "h1-secret-handle-bytes";
    let att = McpAttachment {
        transport: McpTransport::Sse {
            url: "https://mcp.example.com/sse?token=abc123".to_owned(),
        },
        auth: McpAuth::BearerHandle {
            handle: handle.to_owned(),
        },
    };
    assert!(att.validate().is_ok());
    assert_eq!(count(), 0, "validate wrote nothing to the fixture dir");
    // Endpoint label carries no query secrets.
    assert_eq!(att.endpoint_label(), "https://mcp.example.com/sse");
    // Captured logs carry the fixed redaction, never handle/url secrets.
    let log = att.safe_describe();
    assert!(log.contains("hdl-***"), "redaction marker present: {log}");
    assert!(!log.contains(handle), "no handle bytes in logs");
    assert!(!log.contains("abc123"), "no url query secret in logs");
    assert!(!log.contains("token="), "no query key in logs");
    // Fixture transport marker: no live socket, no real process.
    let t = LoopbackTransport::ok();
    assert!(t.is_loopback());
    assert_eq!(LoopbackTransport::FAKE_MARKER, "loopback-fake-v1");
}
