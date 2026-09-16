//! WEB-003: Protocol route-table projection (server-side only).
//!
//! Single authoritative table [`PROTOCOL_ROUTES`]; [`served_routes`] is the
//! projection actually registered on the native router. Invariant:
//! `served_routes() ⊆ PROTOCOL_ROUTES` on (method, path), compared literally.
//! No I/O, no wall-clock, no secrets in the table. Static slice, O(routes).

use std::cell::Cell;
use std::collections::BTreeSet;
use std::fmt;

/// HTTP method for a protocol route. Ordered so derived sorting on
/// `(Method, path)` is deterministic: `GET < POST`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Method {
    Get,
    Post,
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Get => write!(f, "GET"),
            Self::Post => write!(f, "POST"),
        }
    }
}

/// One authoritative Protocol entry: method plus literal path template plus
/// the thin-handler name that serves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolRoute {
    pub method: Method,
    pub path: &'static str,
    pub handler: &'static str,
}

/// Wire kind of a route: JSON body route vs SSE stream route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteKind {
    Json,
    Sse,
}

/// Hard cap on the authoritative table size (no runtime growth).
pub const MAX_ROUTES: usize = 256;

/// Single authoritative Protocol table. Sorted by (method, path).
pub static PROTOCOL_ROUTES: &[ProtocolRoute] = &[
    ProtocolRoute {
        method: Method::Get,
        path: "/events",
        handler: "handle_events_sse",
    },
    ProtocolRoute {
        method: Method::Get,
        path: "/health",
        handler: "handle_health",
    },
    ProtocolRoute {
        method: Method::Post,
        path: "/control/move_session",
        handler: "handle_move_session",
    },
];

/// Routes actually registered on the native router, sorted by
/// (method, path) for determinism. Must stay a subset of
/// [`PROTOCOL_ROUTES`].
#[must_use]
pub fn served_routes() -> Vec<(Method, &'static str)> {
    let mut routes: Vec<(Method, &'static str)> =
        PROTOCOL_ROUTES.iter().map(|r| (r.method, r.path)).collect();
    routes.sort();
    routes
}

/// Wire kind for a served path. `GET /events` is the SSE stream route;
/// every other known route is a JSON route.
#[must_use]
pub fn route_kind(path: &str) -> RouteKind {
    if path == "/events" {
        RouteKind::Sse
    } else {
        RouteKind::Json
    }
}

/// Coverage failure. Carries at most the offending (method, path) keys,
/// never secrets or unrelated table state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverageError {
    /// A served route has no entry in [`PROTOCOL_ROUTES`].
    MissingProtocolEntry { method: Method, path: String },
    /// Two entries share one (method, path) key.
    DuplicateRoute { method: Method, path: String },
    /// Table exceeds [`MAX_ROUTES`].
    TooManyRoutes { max: usize, actual: usize },
}

impl fmt::Display for CoverageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingProtocolEntry { method, path } => {
                write!(f, "route without protocol entry: {method} {path}")
            }
            Self::DuplicateRoute { method, path } => {
                write!(f, "duplicate route: {method} {path}")
            }
            Self::TooManyRoutes { max, actual } => {
                write!(f, "too many routes: max {max}, got {actual}")
            }
        }
    }
}

impl std::error::Error for CoverageError {}

/// Check one served list against the authoritative table: no duplicates in
/// either the table or the served list, and every served (method, path) has
/// a Protocol entry. Pure, synchronous, no I/O.
pub fn check_served(served: &[(Method, &str)]) -> Result<(), CoverageError> {
    if PROTOCOL_ROUTES.len() > MAX_ROUTES {
        return Err(CoverageError::TooManyRoutes {
            max: MAX_ROUTES,
            actual: PROTOCOL_ROUTES.len(),
        });
    }
    let mut seen_protocol = BTreeSet::new();
    for route in PROTOCOL_ROUTES {
        if !seen_protocol.insert((route.method, route.path)) {
            return Err(CoverageError::DuplicateRoute {
                method: route.method,
                path: route.path.to_string(),
            });
        }
    }
    let mut seen_served = BTreeSet::new();
    for (method, path) in served {
        if !seen_served.insert((*method, *path)) {
            return Err(CoverageError::DuplicateRoute {
                method: *method,
                path: (*path).to_string(),
            });
        }
        if !PROTOCOL_ROUTES
            .iter()
            .any(|r| r.method == *method && r.path == *path)
        {
            return Err(CoverageError::MissingProtocolEntry {
                method: *method,
                path: (*path).to_string(),
            });
        }
    }
    Ok(())
}

/// Check the live [`served_routes`] projection against [`PROTOCOL_ROUTES`].
pub fn check_protocol_coverage() -> Result<(), CoverageError> {
    check_served(&served_routes())
}

/// Panicking form for test harnesses: fails naming the offending key.
pub fn assert_protocol_coverage() {
    check_protocol_coverage().expect("protocol coverage must hold");
}

/// Build the native router projection: succeeds exactly when the served
/// routes are covered by Protocol. A rogue injected route surfaces as
/// [`CoverageError::MissingProtocolEntry`] naming (method, path).
pub fn build_router() -> Result<(), CoverageError> {
    check_protocol_coverage()
}

/// Test hook: build the router with extra routes appended, so a rogue
/// `POST /rogue_no_protocol` injection fails the build without mutating
/// the static table.
pub fn build_router_with_extra(extra: &[(Method, &str)]) -> Result<(), CoverageError> {
    let mut served = served_routes();
    served.extend(extra.iter().map(|(m, p)| (*m, *p)));
    check_served(&served)
}

/// Domain service boundary for the thin `POST /control/move_session`
/// handler: exactly one method, no HTTP/wire types cross into it.
pub trait MoveSessionService {
    fn move_session(&self);
}

/// Call spy: records how many times the thin handler reached the domain
/// service for one request. Thin means exactly one call.
pub struct DomainCallSpy {
    calls: Cell<usize>,
}

impl DomainCallSpy {
    #[must_use]
    pub fn new() -> Self {
        Self {
            calls: Cell::new(0),
        }
    }

    #[must_use]
    pub fn calls(&self) -> usize {
        self.calls.get()
    }
}

impl Default for DomainCallSpy {
    fn default() -> Self {
        Self::new()
    }
}

impl MoveSessionService for DomainCallSpy {
    fn move_session(&self) {
        self.calls.set(self.calls.get() + 1);
    }
}

/// Thin-handler dispatch for `POST /control/move_session`: decodes input at
/// the caller (WEB-001), calls exactly one domain service method, translates
/// the outcome at the caller (WEB-002). Performs no I/O itself.
pub fn dispatch_move_session(service: &impl MoveSessionService) {
    service.move_session();
}

/// Substrings that prove a handler module reaches mutating I/O directly
/// instead of going through the domain service.
const FORBIDDEN_IO_TOKENS: &[&str] = &[
    "std::fs",
    "tokio::fs",
    "std::net",
    "tokio::net",
    "std::process",
    "tokio::process",
    "git2",
    "reqwest",
    "hyper",
    "rusqlite",
];

/// Scan handler sources for direct git/fs/net imports. Returns one
/// `"<file>: <token>"` hit per violation, naming file and crate.
/// Pure string scan over caller-provided sources: reads nothing.
#[must_use]
pub fn no_direct_io_imports(handlers: &[(&str, &str)]) -> Vec<String> {
    let mut hits = Vec::new();
    for (file, source) in handlers {
        for token in FORBIDDEN_IO_TOKENS {
            if source.contains(token) {
                hits.push(format!("{file}: {token}"));
            }
        }
    }
    hits
}
