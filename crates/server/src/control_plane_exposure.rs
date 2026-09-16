//! WEB-004: server control-plane remote-exposure boundary (server-side only).
//!
//! Pure synchronous gate: decides reachability from the transport peer
//! address before auth parsing and before any domain service runs. No DNS,
//! no I/O, no clock, no network dial. `X-Forwarded-For` / `Forwarded`
//! headers are never consulted; the verdict uses only the transport peer.
//! Unix-socket peers (no IP) are local. Deny logs carry the peer family
//! label only (v4/v6/socket), never secrets or full addresses.

use std::net::IpAddr;

/// Gated route prefixes: control-plane mutations only. `GET /health` is not
/// gated and stays reachable for local supervision regardless of the flag.
pub const GATED_PREFIXES: [&str; 1] = ["/control/"];

/// Fixed deny template body (<= 512 bytes). No secrets, paths, or I/O text.
pub const DENY_BODY: &str =
    "{\"error\":{\"code\":\"remote_denied\",\"message\":\"remote access denied\"}}";

/// Bind address class. This slice asserts gate verdicts only; bind
/// enforcement itself is daemon-config validation owned elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindAddr {
    Loopback,
    Unspecified,
}

/// Remote-exposure grant. Default is default-deny: loopback bind, no remote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exposure {
    pub allow_remote: bool,
    pub bind: BindAddr,
}

impl Default for Exposure {
    fn default() -> Self {
        Self {
            allow_remote: false,
            bind: BindAddr::Loopback,
        }
    }
}

/// Refusal of a non-local peer without an explicit grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExposureDeny {
    RemoteDenied,
}

impl std::fmt::Display for ExposureDeny {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RemoteDenied => write!(f, "remote access denied"),
        }
    }
}

impl std::error::Error for ExposureDeny {}

/// Transport peer: either an IP from the accepted connection or a
/// Unix-socket peer with no IP (always local).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerAddr {
    Ip(IpAddr),
    Socket,
}

/// Pure exposure verdict over (peer, cfg). Loopback peers always pass;
/// non-loopback peers need `allow_remote: true`. Total and deterministic:
/// same (peer, cfg) always yields the same verdict.
pub fn check_exposure(peer: IpAddr, cfg: &Exposure) -> Result<(), ExposureDeny> {
    check_peer(&PeerAddr::Ip(peer), cfg)
}

/// Socket-peer verdict: Unix-socket peers have no remote surface and pass
/// under any config, including the default.
pub fn check_socket_peer(cfg: &Exposure) -> Result<(), ExposureDeny> {
    check_peer(&PeerAddr::Socket, cfg)
}

/// Header-agnostic entrypoint: forwarded-header hints are accepted only so
/// callers can prove they are ignored. The verdict uses the transport peer
/// alone; any `X-Forwarded-For` / `Forwarded` value is discarded.
pub fn check_with_headers(
    peer: IpAddr,
    cfg: &Exposure,
    _x_forwarded_for: Option<&str>,
    _forwarded: Option<&str>,
) -> Result<(), ExposureDeny> {
    check_exposure(peer, cfg)
}

fn check_peer(peer: &PeerAddr, cfg: &Exposure) -> Result<(), ExposureDeny> {
    match peer {
        PeerAddr::Socket => Ok(()),
        PeerAddr::Ip(addr) if addr.is_loopback() => Ok(()),
        PeerAddr::Ip(_) if cfg.allow_remote => Ok(()),
        PeerAddr::Ip(_) => Err(ExposureDeny::RemoteDenied),
    }
}

/// True iff the (method, path) pair is in the gated control-plane set.
/// Only `POST /control/*` mutation routes are gated; `GET /health` and any
/// route outside `GATED_PREFIXES` is exempt regardless of the flag.
pub fn is_gated_route(method: &str, path: &str) -> bool {
    method == "POST" && GATED_PREFIXES.iter().any(|prefix| path.starts_with(prefix))
}

/// Wire status for a deny.
pub fn deny_status() -> u16 {
    403
}

/// Stable machine-readable deny code.
pub fn deny_code() -> &'static str {
    "remote_denied"
}

/// Exact wire envelope for a deny. Fixed template, bounded well under
/// 512 bytes, carries no peer bytes or secrets.
pub fn deny_body() -> String {
    debug_assert!(DENY_BODY.len() <= 512);
    DENY_BODY.to_string()
}

/// Peer family label for redacted logging: v4, v6, or socket. Never the
/// full address, never credentials.
pub fn peer_family_label(peer: &PeerAddr) -> &'static str {
    match peer {
        PeerAddr::Socket => "socket",
        PeerAddr::Ip(IpAddr::V4(_)) => "v4",
        PeerAddr::Ip(IpAddr::V6(_)) => "v6",
    }
}

/// Redacted deny log line: family label only.
pub fn deny_log_line(peer: &PeerAddr) -> String {
    format!(
        "control_plane_exposure denied peer_family={}",
        peer_family_label(peer)
    )
}
