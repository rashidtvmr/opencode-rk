//! Workspace HTTP/WebSocket proxy forwarding bridge (WSX-001).
//!
//! Caller-owned routing plus header strip plus bounded bridge queue.
//! All functions are synchronous pure logic: no I/O, no clock, no threads,
//! no globals. Caller owns the queue and any socket handles.
#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::fmt;

/// Max total queued bridge bytes (4 MiB).
pub const MAX_PROXY_QUEUE_BYTES: usize = 4_194_304;
/// Max queued bridge items.
pub const MAX_PROXY_QUEUE_ITEMS: usize = 256;
/// Max bytes in a single bridge item (256 KiB).
pub const MAX_PROXY_ITEM_BYTES: usize = 262_144;
/// Max endpoint length in chars.
pub const MAX_PROXY_ENDPOINT_CHARS: usize = 2048;
/// Max header name length in bytes.
pub const MAX_PROXY_HEADER_NAME_LEN: usize = 256;
/// Max header value length in bytes (8 KiB).
pub const MAX_PROXY_HEADER_VALUE_LEN: usize = 8192;

/// Where workspace traffic goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyTarget {
    Local,
    Remote { endpoint: String },
}

/// Caller-supplied routing input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteHint {
    pub path: String,
    pub has_directory: bool,
    pub remote_endpoint: Option<String>,
}

/// Bridge payload kind. Preserved end to end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyItemKind {
    HttpBody,
    WsFrame,
}

/// One queued bridge payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyItem {
    pub kind: ProxyItemKind,
    pub bytes: Vec<u8>,
}

/// Typed failures. Variant names only; never carry header/item bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyError {
    QueueFull,
    TooLarge,
    BadEndpoint,
    BadHeader,
}

impl fmt::Display for ProxyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QueueFull => write!(f, "queue full"),
            Self::TooLarge => write!(f, "too large"),
            Self::BadEndpoint => write!(f, "bad endpoint"),
            Self::BadHeader => write!(f, "bad header"),
        }
    }
}

impl std::error::Error for ProxyError {}

fn valid_endpoint(endpoint: &str) -> bool {
    if endpoint.is_empty() || endpoint.chars().count() > MAX_PROXY_ENDPOINT_CHARS {
        return false;
    }
    let Some((scheme, rest)) = endpoint.split_once("://") else {
        return false;
    };
    if rest.is_empty() {
        return false;
    }
    matches!(
        scheme.to_ascii_lowercase().as_str(),
        "http" | "https" | "ws" | "wss"
    )
}

fn is_bridge_path(path: &str) -> bool {
    const BRIDGE: &str = "/__workspace_ws";
    path == BRIDGE
        || path.strip_prefix(BRIDGE).is_some_and(|rest| {
            rest.starts_with('/') || rest.starts_with('?') || rest.starts_with('#')
        })
}

/// Pure routing decision over caller-supplied input.
///
/// An explicit valid `remote_endpoint` always wins. The `__workspace_ws`
/// bridge path without an endpoint is [`ProxyError::BadEndpoint`].
/// Everything else stays on the loopback path ([`ProxyTarget::Local`]).
pub fn route(hint: &RouteHint) -> Result<ProxyTarget, ProxyError> {
    if let Some(endpoint) = hint.remote_endpoint.as_deref() {
        if !valid_endpoint(endpoint) {
            return Err(ProxyError::BadEndpoint);
        }
        return Ok(ProxyTarget::Remote {
            endpoint: endpoint.to_string(),
        });
    }
    if is_bridge_path(&hint.path) {
        return Err(ProxyError::BadEndpoint);
    }
    Ok(ProxyTarget::Local)
}

fn stripped_name(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "transfer-encoding"
            | "upgrade"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "x-workspace-directory"
            | "x-workspace-remote"
    )
}

/// Strip hop-by-hop plus routing headers; survivors keep input order.
///
/// Input slice is never mutated. Over-cap names/values fail the whole
/// call with [`ProxyError::BadHeader`]; stripped values are dropped.
pub fn strip_headers(
    headers: &[(String, String)],
) -> Result<Vec<(String, String)>, ProxyError> {
    for (name, value) in headers {
        if name.len() > MAX_PROXY_HEADER_NAME_LEN
            || value.len() > MAX_PROXY_HEADER_VALUE_LEN
        {
            return Err(ProxyError::BadHeader);
        }
    }
    Ok(headers
        .iter()
        .filter(|(name, _)| !stripped_name(name))
        .cloned()
        .collect())
}

/// Caller-owned bounded FIFO bridge queue.
#[derive(Debug, Default)]
pub struct ProxyQueue {
    items: VecDeque<ProxyItem>,
    queued_bytes: usize,
}

impl ProxyQueue {
    /// Empty queue; allocates only the empty deque.
    pub fn new() -> Self {
        Self {
            items: VecDeque::new(),
            queued_bytes: 0,
        }
    }

    /// Buffer one item. Rejections leave the queue byte-identical.
    pub fn push(&mut self, item: ProxyItem) -> Result<(), ProxyError> {
        if item.bytes.len() > MAX_PROXY_ITEM_BYTES {
            return Err(ProxyError::TooLarge);
        }
        if self.items.len() >= MAX_PROXY_QUEUE_ITEMS
            || self
                .queued_bytes
                .saturating_add(item.bytes.len())
                > MAX_PROXY_QUEUE_BYTES
        {
            return Err(ProxyError::QueueFull);
        }
        self.queued_bytes = self.queued_bytes.saturating_add(item.bytes.len());
        self.items.push_back(item);
        Ok(())
    }

    /// Remove all items in FIFO order; queue is empty afterwards.
    pub fn drain(&mut self) -> Vec<ProxyItem> {
        self.queued_bytes = 0;
        self.items.drain(..).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn queue_len(&self) -> usize {
        self.items.len()
    }

    pub fn queued_bytes(&self) -> usize {
        self.queued_bytes
    }
}
