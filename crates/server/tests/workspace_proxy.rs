// WSX-001 contract tests: workspace HTTP/WebSocket proxy bridge.
// Maps to obligations WSX-001-T01..T05 in tasks/WSX-001.md.
// Module included by path; shared lib.rs wiring left to integrator.
#[path = "../src/workspace_proxy.rs"]
mod workspace_proxy;

use workspace_proxy::{
    route, strip_headers, ProxyError, ProxyItem, ProxyItemKind, ProxyQueue, ProxyTarget,
    RouteHint, MAX_PROXY_QUEUE_BYTES, MAX_PROXY_QUEUE_ITEMS,
};

fn local_hint(path: &str) -> RouteHint {
    RouteHint {
        path: path.to_string(),
        has_directory: true,
        remote_endpoint: None,
    }
}

fn remote_hint(path: &str, endpoint: &str) -> RouteHint {
    RouteHint {
        path: path.to_string(),
        has_directory: false,
        remote_endpoint: Some(endpoint.to_string()),
    }
}

fn item(kind: ProxyItemKind, bytes: &[u8]) -> ProxyItem {
    ProxyItem {
        kind,
        bytes: bytes.to_vec(),
    }
}

#[test]
fn wsx001_t01_happy_path_route_strip_fifo() {
    assert_eq!(
        route(&local_hint("/api/sessions/abc")),
        Ok(ProxyTarget::Local)
    );
    assert_eq!(
        route(&remote_hint("/__workspace_ws", "wss://remote.example/s")),
        Ok(ProxyTarget::Remote {
            endpoint: "wss://remote.example/s".to_string()
        })
    );

    let headers = vec![
        ("Content-Type".to_string(), "application/json".to_string()),
        ("Connection".to_string(), "keep-alive".to_string()),
        ("X-Request-Id".to_string(), "r1".to_string()),
        ("Upgrade".to_string(), "websocket".to_string()),
        ("X-Workspace-Directory".to_string(), "/tmp/w".to_string()),
        ("Authorization".to_string(), "Bearer t".to_string()),
        ("TE".to_string(), "trailers".to_string()),
        ("X-Workspace-Remote".to_string(), "wss://r/s".to_string()),
        ("Transfer-Encoding".to_string(), "chunked".to_string()),
        ("Keep-Alive".to_string(), "timeout=5".to_string()),
        ("Trailer".to_string(), "x".to_string()),
        ("Proxy-Authenticate".to_string(), "Basic".to_string()),
        (
            "Proxy-Authorization".to_string(),
            "Basic c2VjcmV0".to_string(),
        ),
        ("Accept".to_string(), "*/*".to_string()),
    ];
    let stripped = strip_headers(&headers).unwrap();
    assert_eq!(
        stripped,
        vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("X-Request-Id".to_string(), "r1".to_string()),
            ("Authorization".to_string(), "Bearer t".to_string()),
            ("Accept".to_string(), "*/*".to_string()),
        ]
    );

    let mut q = ProxyQueue::new();
    q.push(item(ProxyItemKind::HttpBody, b"hello")).unwrap();
    q.push(item(ProxyItemKind::WsFrame, &[0x81, 0x05])).unwrap();
    q.push(item(ProxyItemKind::HttpBody, b"world")).unwrap();
    let out = q.drain();
    assert_eq!(out.len(), 3);
    assert_eq!(out[0].kind, ProxyItemKind::HttpBody);
    assert_eq!(out[0].bytes, b"hello");
    assert_eq!(out[1].kind, ProxyItemKind::WsFrame);
    assert_eq!(out[1].bytes, vec![0x81, 0x05]);
    assert_eq!(out[2].kind, ProxyItemKind::HttpBody);
    assert_eq!(out[2].bytes, b"world");
    assert!(q.is_empty());
    assert_eq!(q.queue_len(), 0);
    assert_eq!(q.queued_bytes(), 0);
}

#[test]
fn wsx001_t02_routing_table_and_bad_endpoints() {
    // Explicit remote hint wins, even on a local-looking path.
    assert_eq!(
        route(&remote_hint("/api/sessions/x", "wss://remote.example/w")),
        Ok(ProxyTarget::Remote {
            endpoint: "wss://remote.example/w".to_string()
        })
    );
    assert_eq!(
        route(&remote_hint("/api/status", "https://remote.example/api")),
        Ok(ProxyTarget::Remote {
            endpoint: "https://remote.example/api".to_string()
        })
    );
    // Unsupported scheme.
    assert_eq!(
        route(&remote_hint("/__workspace_ws", "ftp://remote.example/s")),
        Err(ProxyError::BadEndpoint)
    );
    // Empty endpoint is never valid.
    assert_eq!(
        route(&RouteHint {
            path: "/__workspace_ws".to_string(),
            has_directory: false,
            remote_endpoint: Some(String::new()),
        }),
        Err(ProxyError::BadEndpoint)
    );
    // Oversize endpoint (> 2048 chars).
    let big = format!("wss://{}", "a".repeat(2048));
    assert!(big.chars().count() > 2048);
    assert_eq!(
        route(&remote_hint("/__workspace_ws", &big)),
        Err(ProxyError::BadEndpoint)
    );
    // Exact 2048-char endpoint is accepted.
    let max_ok = format!("wss://{}", "a".repeat(2042));
    assert_eq!(max_ok.chars().count(), 2048);
    assert_eq!(
        route(&remote_hint("/__workspace_ws", &max_ok)),
        Ok(ProxyTarget::Remote {
            endpoint: max_ok.clone()
        })
    );
    // Bridge path with no endpoint at all cannot route anywhere.
    assert_eq!(
        route(&RouteHint {
            path: "/__workspace_ws".to_string(),
            has_directory: false,
            remote_endpoint: None,
        }),
        Err(ProxyError::BadEndpoint)
    );
    // Local status path without directory: documented loopback default.
    assert_eq!(
        route(&RouteHint {
            path: "/api/status".to_string(),
            has_directory: false,
            remote_endpoint: None,
        }),
        Ok(ProxyTarget::Local)
    );
    // Unknown path without endpoint: loopback default.
    assert_eq!(
        route(&RouteHint {
            path: "/api/other".to_string(),
            has_directory: false,
            remote_endpoint: None,
        }),
        Ok(ProxyTarget::Local)
    );
}

#[test]
fn wsx001_t03_queue_caps_atomic() {
    let mut q = ProxyQueue::new();
    for _ in 0..MAX_PROXY_QUEUE_ITEMS {
        q.push(item(ProxyItemKind::HttpBody, b"q")).unwrap();
    }
    assert_eq!(q.queue_len(), 256);
    let bytes = q.queued_bytes();
    assert_eq!(
        q.push(item(ProxyItemKind::WsFrame, b"q")),
        Err(ProxyError::QueueFull)
    );
    assert_eq!(q.queue_len(), 256);
    assert_eq!(q.queued_bytes(), bytes);

    let mut q2 = ProxyQueue::new();
    let big = vec![7u8; 262_144];
    for _ in 0..16 {
        q2.push(ProxyItem {
            kind: ProxyItemKind::HttpBody,
            bytes: big.clone(),
        })
        .unwrap();
    }
    assert_eq!(q2.queued_bytes(), MAX_PROXY_QUEUE_BYTES);
    assert_eq!(q2.queue_len(), 16);
    assert_eq!(
        q2.push(item(ProxyItemKind::HttpBody, b"tiny")),
        Err(ProxyError::QueueFull)
    );
    assert_eq!(q2.queued_bytes(), MAX_PROXY_QUEUE_BYTES);
    assert_eq!(q2.queue_len(), 16);
    assert_eq!(
        q2.push(ProxyItem {
            kind: ProxyItemKind::WsFrame,
            bytes: vec![0u8; 262_145],
        }),
        Err(ProxyError::TooLarge)
    );
    assert_eq!(q2.queued_bytes(), MAX_PROXY_QUEUE_BYTES);
    assert_eq!(q2.queue_len(), 16);
    // Exact single-item cap is accepted on a fresh queue.
    let mut q3 = ProxyQueue::new();
    assert!(
        q3.push(ProxyItem {
            kind: ProxyItemKind::WsFrame,
            bytes: vec![1u8; 262_144],
        })
        .is_ok()
    );
}

#[test]
fn wsx001_t04_strip_safety_and_bad_headers() {
    let secret = "s3cr3t-proxy-token-zzz";
    let headers = vec![
        ("Proxy-Authorization".to_string(), secret.to_string()),
        ("X-Custom".to_string(), "keep".to_string()),
    ];
    let before = headers.clone();
    let stripped = strip_headers(&headers).unwrap();
    assert_eq!(
        stripped,
        vec![("X-Custom".to_string(), "keep".to_string())]
    );
    let rendered = format!("{stripped:?}");
    assert!(
        !rendered.contains(secret),
        "stripped value must not leak"
    );
    assert_eq!(headers, before);

    let long_name = "n".repeat(257);
    let bad = vec![(long_name, "v".to_string())];
    assert_eq!(strip_headers(&bad), Err(ProxyError::BadHeader));
    assert_eq!(bad.len(), 1);

    let big_value = "v".repeat(8193);
    assert_eq!(
        strip_headers(&[("X-Ok".to_string(), big_value)]),
        Err(ProxyError::BadHeader)
    );

    // Boundary caps accepted.
    let edge = vec![
        ("n".repeat(256), "v".to_string()),
        ("X-Ok".to_string(), "v".repeat(8192)),
    ];
    assert_eq!(strip_headers(&edge).unwrap().len(), 2);

    // Errors carry variant names only, never header bytes.
    assert!(!format!("{}", ProxyError::BadHeader).contains(secret));
    assert!(!format!("{:?}", ProxyError::BadHeader).contains(secret));
}

#[test]
fn wsx001_t05_isolation_and_determinism() {
    let dir = tempfile::tempdir().unwrap();
    let run = || {
        let t1 = route(&local_hint("/api/sessions/abc")).unwrap();
        let t2 = route(&remote_hint("/__workspace_ws", "wss://remote.example/s")).unwrap();
        let s = strip_headers(&[
            ("Connection".to_string(), "close".to_string()),
            ("X-Keep".to_string(), "1".to_string()),
        ])
        .unwrap();
        let mut q = ProxyQueue::new();
        q.push(item(ProxyItemKind::HttpBody, b"one")).unwrap();
        q.push(item(ProxyItemKind::WsFrame, b"two")).unwrap();
        let d = q.drain();
        (t1, t2, s, d, q.queue_len(), q.queued_bytes())
    };
    assert_eq!(run(), run());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/workspace_proxy.rs"
    ))
    .unwrap();
    for banned in [
        "std::fs",
        "std::net",
        "std::thread",
        "std::process",
        "tokio",
        "Command",
        "TcpStream",
        "UdpSocket",
    ] {
        assert!(!src.contains(banned), "banned api in module: {banned}");
    }
}
