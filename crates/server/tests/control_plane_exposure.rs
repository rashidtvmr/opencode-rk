//! WEB-004 FROZEN: server control-plane remote-exposure boundary.
//! T01 local allowed, T02 remote default-deny, T03 grant + header spoof,
//! T04 health exempt + socket peer, T05 no side effect + safety.
//! Includes the boundary module directly so the integrator never needs to
//! touch shared `lib.rs` for this lane.
#[path = "../src/control_plane_exposure.rs"]
mod control_plane_exposure;

use control_plane_exposure::{
    check_exposure, check_socket_peer, check_with_headers, deny_body, deny_code, deny_log_line,
    deny_status, is_gated_route, peer_family_label, BindAddr, Exposure, ExposureDeny, PeerAddr,
};
use std::cell::{Cell, RefCell};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

fn loopback_v4() -> IpAddr {
    IpAddr::V4(Ipv4Addr::LOCALHOST)
}

fn remote_v4() -> IpAddr {
    // TEST-NET-1 documentation address: non-loopback, never routed.
    IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1))
}

/// Ordered spy: exposure gate runs before auth parsing and before the domain
/// service call. Records each stage it enters.
struct OrderSpy {
    order: RefCell<Vec<&'static str>>,
    auth_entered: Cell<bool>,
    service_calls: Cell<usize>,
}

impl OrderSpy {
    fn new() -> Self {
        Self {
            order: RefCell::new(Vec::new()),
            auth_entered: Cell::new(false),
            service_calls: Cell::new(0),
        }
    }

    /// Route-layer dispatch for POST /control/move_session.
    fn dispatch(&self, peer: IpAddr, cfg: &Exposure) -> Result<(), ExposureDeny> {
        self.order.borrow_mut().push("exposure");
        check_exposure(peer, cfg)?;
        self.order.borrow_mut().push("auth");
        self.auth_entered.set(true);
        // Auth parsing itself stays with the auth owner; the spy stands in.
        self.order.borrow_mut().push("service");
        self.service_calls.set(self.service_calls.get() + 1);
        Ok(())
    }

    fn order(&self) -> Vec<&'static str> {
        self.order.borrow().clone()
    }
}

// WEB-004-T01 (local always allowed): loopback + default cfg passes and the
// ordered spy shows exposure ran before auth and before the service.
#[test]
fn web004_t01_loopback_allowed_with_default_cfg() {
    let spy = OrderSpy::new();
    let cfg = Exposure::default();
    assert!(!cfg.allow_remote, "remote must be opt-in, never default");
    assert_eq!(cfg.bind, BindAddr::Loopback);
    assert!(check_exposure(loopback_v4(), &cfg).is_ok());
    assert!(check_exposure(IpAddr::V6(Ipv6Addr::LOCALHOST), &cfg).is_ok());
    spy.dispatch(loopback_v4(), &cfg)
        .expect("local dispatch reaches the service");
    assert_eq!(spy.order(), ["exposure", "auth", "service"]);
    assert!(spy.auth_entered.get());
    assert_eq!(spy.service_calls.get(), 1);
}

// WEB-004-T02 (remote default-deny): non-loopback + default cfg is refused
// before auth parsing and before any domain call; wire shape is 403.
#[test]
fn web004_t02_remote_denied_by_default_before_auth_and_service() {
    let spy = OrderSpy::new();
    let cfg = Exposure::default();
    let err = check_exposure(remote_v4(), &cfg).unwrap_err();
    assert_eq!(err, ExposureDeny::RemoteDenied);
    assert_eq!(format!("{err}"), "remote access denied");
    spy.dispatch(remote_v4(), &cfg)
        .expect_err("remote dispatch must not reach the service");
    assert_eq!(spy.order(), ["exposure"]);
    assert!(!spy.auth_entered.get(), "auth parser never runs on deny");
    assert_eq!(spy.service_calls.get(), 0);
    assert_eq!(deny_status(), 403);
    assert_eq!(deny_code(), "remote_denied");
    let body = deny_body();
    assert!(body.len() <= 512, "deny body bounded");
    assert!(body.contains("\"code\":\"remote_denied\""));
    let value: serde_json::Value = serde_json::from_str(&body).expect("valid JSON deny body");
    assert_eq!(value["error"]["code"], "remote_denied");
}

// WEB-004-T03 (explicit grant + header spoof): non-loopback + allow_remote
// passes; spoofed loopback headers over a non-loopback transport peer are
// ignored and the verdict stays denied.
#[test]
fn web004_t03_grant_allows_remote_but_spoofed_headers_stay_denied() {
    let grant = Exposure {
        allow_remote: true,
        bind: BindAddr::Loopback,
    };
    assert!(check_exposure(remote_v4(), &grant).is_ok());
    let spy = OrderSpy::new();
    spy.dispatch(remote_v4(), &grant)
        .expect("granted remote reaches the service");
    assert_eq!(spy.order(), ["exposure", "auth", "service"]);

    // Spoofed loopback headers must not flip a transport-peer deny.
    let denied =
        check_with_headers(remote_v4(), &Exposure::default(), Some("127.0.0.1"), None).unwrap_err();
    assert_eq!(denied, ExposureDeny::RemoteDenied);
    let denied = check_with_headers(
        remote_v4(),
        &Exposure::default(),
        None,
        Some("for=127.0.0.1"),
    )
    .unwrap_err();
    assert_eq!(denied, ExposureDeny::RemoteDenied);
    // Grant verdict is header-independent too.
    assert!(check_with_headers(remote_v4(), &grant, Some("10.0.0.9"), None).is_ok());
}

// WEB-004-T04 (health exempt + socket peer): GET /health is outside the gated
// set for any peer; Unix-socket peers pass the gated route with default cfg.
#[test]
fn web004_t04_health_exempt_and_socket_peer_local() {
    assert!(
        !is_gated_route("GET", "/health"),
        "health stays reachable regardless of flag"
    );
    assert!(is_gated_route("POST", "/control/move_session"));
    // Non-gated route from a remote peer needs no grant.
    let health_allowed = |peer: IpAddr, cfg: &Exposure| {
        if is_gated_route("GET", "/health") {
            check_exposure(peer, cfg).is_ok()
        } else {
            true
        }
    };
    assert!(health_allowed(remote_v4(), &Exposure::default()));
    // Unix-socket peer (no IP) is local.
    assert!(check_socket_peer(&Exposure::default()).is_ok());
    assert_eq!(peer_family_label(&PeerAddr::Socket), "socket");
    assert_eq!(peer_family_label(&PeerAddr::Ip(loopback_v4())), "v4");
    assert_eq!(
        peer_family_label(&PeerAddr::Ip(IpAddr::V6(Ipv6Addr::LOCALHOST))),
        "v6"
    );
}

// WEB-004-T05 (no side effect + safety): a denied remote attempt performs zero
// filesystem I/O outside the disposable fixture and zero domain calls; logs
// carry at most the peer family label, never secret bytes.
#[test]
fn web004_t05_deny_is_side_effect_free_and_redacted() {
    let src = include_str!("../src/control_plane_exposure.rs");
    for banned in [
        "std::fs",
        "TcpStream",
        "UdpSocket",
        "tokio",
        "axum",
        "gethostbyname",
        "SystemTime",
        "std::env",
    ] {
        assert!(!src.contains(banned), "gate must not use {banned}");
    }

    let dir = tempfile::tempdir().expect("disposable fixture");
    let sentinel = dir.path().join("sentinel.txt");
    std::fs::write(&sentinel, b"untouched").expect("sentinel fixture");
    let secret = "ses_SUPERSECRET_fixture_token";

    let spy = OrderSpy::new();
    let peer = remote_v4();
    for _ in 0..3 {
        let verdict = check_with_headers(
            peer,
            &Exposure::default(),
            Some("127.0.0.1"),
            Some("for=127.0.0.1"),
        );
        assert_eq!(verdict.unwrap_err(), ExposureDeny::RemoteDenied);
        let _ = spy.dispatch(peer, &Exposure::default());
    }
    // One spy dispatch per loop iteration entered exposure only.
    assert_eq!(spy.service_calls.get(), 0);
    assert!(!spy.auth_entered.get());

    // Deterministic: same (peer, cfg) always yields the same verdict.
    assert_eq!(
        check_exposure(peer, &Exposure::default()),
        check_exposure(peer, &Exposure::default())
    );

    let log_line = deny_log_line(&PeerAddr::Ip(peer));
    assert!(
        log_line.contains("v4"),
        "family label only, got: {log_line}"
    );
    for rendered in [
        log_line.as_str(),
        deny_body().as_str(),
        format!("{:?}", ExposureDeny::RemoteDenied).as_str(),
    ] {
        assert!(!rendered.contains(secret), "zero secret bytes");
        assert!(!rendered.contains("SUPERSECRET"), "zero secret bytes");
    }
    assert_eq!(
        std::fs::read(&sentinel).expect("reread sentinel"),
        b"untouched"
    );
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .expect("list fixture")
        .collect();
    assert_eq!(entries.len(), 1, "nothing written beside the sentinel");
}
