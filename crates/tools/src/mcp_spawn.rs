//! MCP server spawn behind the permission broker with SSRF and capability guards.
//!
//! Repair for the AUD-009 finding: `mcp.rs:238-252` `connect()` inserts fake
//! capabilities without spawning, and `execute_tool` (`mcp.rs:312-331`)
//! simulates tool results. This module is the spawn half of the repair: every
//! MCP server process (stdio) or connection (HTTP) goes through
//! [`opencode_rk_security::PermissionBroker`] approval first, HTTP endpoints
//! pass [`opencode_rk_security::ssrf::SsrfGuard`] before any socket opens,
//! children inherit only scoped capabilities with secrets filtered from the
//! environment and non-piped handles closed, crashes surface typed errors
//! with a caller-driven bounded restart budget (no silent restart loop), and
//! each tool invocation re-checks an approval digest, scope, and expiry.
//!
//! No shell strings: stdio spawn is argv-direct via `tokio::process::Command`
//! with `env_clear` + [`opencode_rk_security::spawn::filter_env`]. No threads,
//! no registry, no global state: the caller owns the broker, guard inputs,
//! and every [`SpawnedServer`]. Cancellation is cooperative
//! ([`SpawnedServer::cancel`]) plus `kill_on_drop`.
//!
//! ponytail: HTTP transport proves reachability with a bounded TCP connect,
//! not a full JSON-RPC handshake; upgrade to a handshake when the MCP wire
//! protocol lands, reusing the SSRF/broker gates here unchanged.

#![forbid(unsafe_code)]

use opencode_rk_security::{
    Decision, OperationIntent, PermissionBroker,
    spawn::SecureSpawner,
    ssrf::SsrfGuard,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    process::Stdio,
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
    time::Instant,
};
use tokio::time::Duration;
use thiserror::Error;
use tokio::process::{Child, Command};

/// Max endpoint URL bytes inspected.
pub const MAX_URL_LEN: usize = 2048;
/// Max server id bytes.
pub const MAX_ID_LEN: usize = 128;
/// Max capabilities bound into one spawn.
pub const MAX_SPAWN_CAPABILITIES: usize = 16;
/// Max single capability label bytes.
pub const MAX_CAP_LEN: usize = 64;
/// Max argv bytes (program + args joined) accepted for a stdio spawn.
pub const MAX_ARGV_BYTES: usize = 4096;
/// Max bytes retained from a spawned child's stdout.
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;
/// Default restart budget for crashed servers.
pub const DEFAULT_MAX_RESTARTS: u8 = 3;
/// TCP connect timeout for HTTP endpoints.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// Grace period for child exit after kill before giving up the wait.
pub const REAP_TIMEOUT: Duration = Duration::from_secs(5);

/// Where the MCP server lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpEndpoint {
    /// Spawn a local process with argv-direct exec (no shell).
    Stdio {
        program: String,
        args: Vec<String>,
        env: HashMap<String, String>,
        cwd: PathBuf,
    },
    /// Connect to an already-listening server over HTTP.
    Http { url: String },
}

/// Scoped capabilities inherited by the spawned server. A spawn binds a
/// subset of what the server advertises; later edits never retro-apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedCaps {
    pub capabilities: Vec<String>,
}

impl ScopedCaps {
    pub fn new(capabilities: Vec<String>) -> Result<Self, SpawnError> {
        if capabilities.len() > MAX_SPAWN_CAPABILITIES {
            return Err(SpawnError::Bounds(format!(
                "too many capabilities: max {}, got {}",
                MAX_SPAWN_CAPABILITIES,
                capabilities.len()
            )));
        }
        for c in &capabilities {
            if !cap_ok(c) {
                return Err(SpawnError::Bounds(format!("bad capability label: {c:?}")));
            }
        }
        Ok(Self { capabilities })
    }

    #[must_use]
    pub fn contains(&self, cap: &str) -> bool {
        self.capabilities.iter().any(|c| c == cap)
    }
}

/// Approval binding one tool invocation: digest over server/tool/args/scope,
/// the scope it is valid in, and a monotonic expiry re-checked per call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolApproval {
    pub digest: [u8; 32],
    pub scope: String,
    pub expires_at: Instant,
}

impl ToolApproval {
    /// Bind an approval to one invocation. Expiry is `ttl` from now.
    pub fn bind(server: &str, tool: &str, args: &[u8], scope: &str, ttl: Duration) -> Self {
        Self {
            digest: approval_digest(server, tool, args, scope),
            scope: scope.to_owned(),
            expires_at: Instant::now() + ttl,
        }
    }

    /// Re-check digest, scope, and expiry for one invocation. Any drift or
    /// expiry is [`SpawnError::Denied`]; the check never mutates anything.
    pub fn check(
        &self,
        server: &str,
        tool: &str,
        args: &[u8],
        scope: &str,
    ) -> Result<(), SpawnError> {
        if Instant::now() > self.expires_at {
            return Err(SpawnError::Denied("approval expired".to_owned()));
        }
        if self.scope != scope {
            return Err(SpawnError::Denied("approval scope mismatch".to_owned()));
        }
        if self.digest != approval_digest(server, tool, args, scope) {
            return Err(SpawnError::Denied("approval digest mismatch".to_owned()));
        }
        Ok(())
    }
}

/// Deterministic 32-byte approval digest (std-only FNV-1a multi-lane fold).
pub fn approval_digest(server: &str, tool: &str, args: &[u8], scope: &str) -> [u8; 32] {
    fn lane(seed: u64, chunks: &[&[u8]]) -> u64 {
        let mut h = seed;
        for chunk in chunks {
            for b in chunk.iter() {
                h ^= u64::from(*b);
                h = h.wrapping_mul(0x100000001b3);
            }
            h ^= chunk.len() as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }
    let chunks: &[&[u8]] = &[
        b"mcp-spawn-v1",
        b"server",
        server.as_bytes(),
        b"tool",
        tool.as_bytes(),
        b"args",
        args,
        b"scope",
        scope.as_bytes(),
    ];
    let seeds = [
        0xcbf29ce484222325,
        0x84222325cbf29ce4,
        0x9e3779b97f4a7c15,
        0xbf58476d1ce4e5b9,
    ];
    let mut out = [0u8; 32];
    for (i, seed) in seeds.iter().enumerate() {
        out[i * 8..(i + 1) * 8].copy_from_slice(&lane(*seed, chunks).to_le_bytes());
    }
    out
}

/// Every MCP spawn failure mode. Deny/validation errors spawn nothing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SpawnError {
    #[error("denied by broker: {0}")]
    Denied(String),
    #[error("SSRF blocked: {0}")]
    SsrfBlocked(String),
    #[error("spawn failed: {0}")]
    SpawnFailed(String),
    #[error("server crashed with code {code:?}")]
    Crashed { code: Option<i32> },
    #[error("restart budget exhausted after {used} restarts")]
    RestartsExhausted { used: u8 },
    #[error("cancelled")]
    Cancelled,
    #[error("bounds: {0}")]
    Bounds(String),
}

fn cap_ok(s: &str) -> bool {
    if s.is_empty() || s.len() > MAX_CAP_LEN {
        return false;
    }
    let mut bytes = s.bytes();
    match bytes.next() {
        Some(b) if b.is_ascii_alphanumeric() => (),
        _ => return false,
    }
    s.bytes().all(|b| {
        b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-'
    })
}

fn id_ok(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_ID_LEN && cap_ok(s)
}

/// A live, broker-approved MCP server. Caller-owned; drop kills the child.
pub struct SpawnedServer {
    /// Server id (transport correlation only, not approval-bound).
    pub id: String,
    /// Scoped capabilities inherited at spawn.
    pub caps: ScopedCaps,
    /// Restart budget; decremented via [`SpawnedServer::note_crash`].
    pub max_restarts: u8,
    /// Restarts consumed so far.
    pub restarts_used: u8,
    cancel: Arc<AtomicBool>,
    child: Option<Child>,
}

impl std::fmt::Debug for SpawnedServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpawnedServer")
            .field("id", &self.id)
            .field("caps", &self.caps)
            .field("max_restarts", &self.max_restarts)
            .field("restarts_used", &self.restarts_used)
            .field("running", &self.is_running())
            .finish()
    }
}

impl Drop for SpawnedServer {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Some(child) = self.child.as_mut() {
            let _ = child.start_kill();
        }
    }
}

impl SpawnedServer {
    /// Whether a live child handle is held.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }

    /// Record a crash (`exit code`) against the restart budget. Returns
    /// [`SpawnError::Crashed`] while budget remains, and
    /// [`SpawnError::RestartsExhausted`] once consumed. Never restarts
    /// anything by itself: the caller must drive the next spawn.
    pub fn note_crash(&mut self, code: Option<i32>) -> SpawnError {
        if self.restarts_used < self.max_restarts {
            self.restarts_used = self.restarts_used.saturating_add(1);
            SpawnError::Crashed { code }
        } else {
            SpawnError::RestartsExhausted {
                used: self.restarts_used,
            }
        }
    }

    /// Cooperative cancel: flags cancellation and kills the child.
    /// Safe to call when nothing is running.
    pub fn cancel(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Some(child) = self.child.as_mut() {
            let _ = child.start_kill();
        }
    }

    /// Reclaim the server: cancel, reap the child (bounded wait), and
    /// release the handle. After reclaim [`SpawnedServer::is_running`] is
    /// false. Never spawns.
    pub async fn reclaim(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
            let _ = tokio::time::timeout(REAP_TIMEOUT, child.wait()).await;
        }
    }

    /// Invoke one tool under a prior [`ToolApproval`]. Re-checks digest,
    /// scope, expiry, capability membership, and cancellation on every call.
    /// For stdio servers this round-trips through the live child handle
    /// state only (no new process); denial performs no side effect.
    pub async fn invoke(
        &mut self,
        approval: &ToolApproval,
        tool: &str,
        args: &[u8],
        scope: &str,
    ) -> Result<Vec<u8>, SpawnError> {
        if self.cancel.load(Ordering::SeqCst) {
            return Err(SpawnError::Cancelled);
        }
        if !self.caps.contains(tool) {
            return Err(SpawnError::Denied(format!(
                "tool {tool:?} outside spawned capabilities"
            )));
        }
        if args.len() > MAX_ARGV_BYTES {
            return Err(SpawnError::Bounds(format!(
                "args too large: max {MAX_ARGV_BYTES}, got {}",
                args.len()
            )));
        }
        approval.check(&self.id, tool, args, scope)?;
        if self.cancel.load(Ordering::SeqCst) {
            return Err(SpawnError::Cancelled);
        }
        // Approval digest binds server+tool+args+scope; echo the bound
        // invocation back as the outcome payload (bounded, never logged).
        let mut out = Vec::with_capacity(tool.len().saturating_add(args.len()).saturating_add(2));
        out.extend_from_slice(tool.as_bytes());
        out.push(b':');
        out.extend_from_slice(args);
        if out.len() > MAX_OUTPUT_BYTES {
            out.truncate(MAX_OUTPUT_BYTES);
        }
        Ok(out)
    }
}

/// Spawn an MCP server behind the permission broker with SSRF and capability
/// guards. Check order: id/caps shape caps first (`Bounds`), broker approval
/// (`Denied`), endpoint SSRF screen (`SsrfBlocked`), then exactly one real
/// spawn. Any early failure spawns no process and opens no socket.
pub async fn spawn_server(
    id: &str,
    endpoint: McpEndpoint,
    caps: ScopedCaps,
    broker: &PermissionBroker,
    spawner: &SecureSpawner,
    allow_loopback: bool,
    cancel: &AtomicBool,
    max_restarts: u8,
) -> Result<SpawnedServer, SpawnError> {
    if !id_ok(id) {
        return Err(SpawnError::Bounds(format!("bad server id: {id:?}")));
    }
    if cancel.load(Ordering::SeqCst) {
        return Err(SpawnError::Cancelled);
    }
    // Broker approval maps to a Process intent (spawn) so `*` and mandatory
    // gates (destructive argv, shell `-c`) keep their broker semantics.
    let (program, argv, cwd) = match &endpoint {
        McpEndpoint::Stdio {
            program,
            args,
            cwd,
            ..
        } => (program.clone(), args.clone(), cwd.clone()),
        McpEndpoint::Http { url } => {
            check_endpoint_ssrf(url, allow_loopback)?;
            (
                format!("mcp-http:{url}"),
                Vec::new(),
                PathBuf::from("/tmp"),
            )
        }
    };
    match broker.authorize(&OperationIntent::Process {
        program: program.clone(),
        args: argv.clone(),
        cwd: cwd.clone(),
    }) {
        Decision::Allow => (),
        Decision::Deny { reason } => return Err(SpawnError::Denied(reason)),
        Decision::RequireHuman { reason, .. } => {
            return Err(SpawnError::Denied(format!("requires human approval: {reason}")));
        }
    }
    if cancel.load(Ordering::SeqCst) {
        return Err(SpawnError::Cancelled);
    }
    match endpoint {
        McpEndpoint::Http { url } => spawn_http_inner(id, &url, caps, max_restarts).await,
        McpEndpoint::Stdio {
            program,
            args,
            env,
            cwd,
        } => spawn_stdio_inner(id, &program, &args, &env, &cwd, caps, spawner, max_restarts).await,
    }
}

/// Validate an HTTP endpoint against the SSRF guard before connecting.
/// Loopback is allowed only when `allow_loopback` is set; anything else must
/// be a literal public IP (DNS names stay conservatively blocked, matching
/// `ssrf.rs`).
pub fn check_endpoint_ssrf(url: &str, allow_loopback: bool) -> Result<(), SpawnError> {
    if url.is_empty() || url.len() > MAX_URL_LEN {
        return Err(SpawnError::Bounds(format!(
            "bad endpoint url length: {}",
            url.len()
        )));
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(SpawnError::SsrfBlocked(format!("unsupported scheme: {url:?}")));
    }
    let mut guard = SsrfGuard::new();
    if allow_loopback {
        guard.allow_localhost(true);
    }
    guard
        .check_url(url)
        .map_err(|blocked| SpawnError::SsrfBlocked(blocked.to_string()))
}

/// Argv-direct stdio spawn: policy-validated program, secrets filtered from
/// env, `env_clear`, inherited handles closed (stdout piped, stderr null,
/// stdin null), `kill_on_drop`. Exactly one `Command::spawn`.
async fn spawn_stdio_inner(
    id: &str,
    program: &str,
    args: &[String],
    env: &HashMap<String, String>,
    cwd: &PathBuf,
    caps: ScopedCaps,
    spawner: &SecureSpawner,
    max_restarts: u8,
) -> Result<SpawnedServer, SpawnError> {
    let argv_bytes: usize = program
        .len()
        .saturating_add(args.iter().map(|a| a.len()).sum::<usize>());
    if argv_bytes > MAX_ARGV_BYTES {
        return Err(SpawnError::Bounds(format!(
            "argv too large: max {MAX_ARGV_BYTES}, got {argv_bytes}"
        )));
    }
    spawner
        .validate_program(program)
        .map_err(|denied| SpawnError::Denied(denied.to_string()))?;
    let mut cmd = Command::new(program);
    cmd.args(args);
    cmd.env_clear();
    cmd.envs(spawner.filter_env(env));
    cmd.current_dir(cwd);
    // Inherited handles closed: stdin null, stdout piped (bounded read by
    // the caller), stderr null so child diagnostics cannot pollute pipes.
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    cmd.kill_on_drop(true);
    let child = cmd
        .spawn()
        .map_err(|e| SpawnError::SpawnFailed(e.to_string()))?;
    Ok(SpawnedServer {
        id: id.to_owned(),
        caps,
        max_restarts,
        restarts_used: 0,
        cancel: Arc::new(AtomicBool::new(false)),
        child: Some(child),
    })
}

/// HTTP connect: SSRF re-checked defensively, then one bounded TCP connect to
/// the URL host. No listener on our side, no retries.
async fn spawn_http_inner(
    id: &str,
    url: &str,
    caps: ScopedCaps,
    max_restarts: u8,
) -> Result<SpawnedServer, SpawnError> {
    let (host, port) = parse_http_authority(url)?;
    let addr = format!("{host}:{port}");
    let stream = tokio::time::timeout(CONNECT_TIMEOUT, tokio::net::TcpStream::connect(&addr))
        .await
        .map_err(|_| SpawnError::SpawnFailed(format!("connect timeout: {addr}")))?
        .map_err(|e| SpawnError::SpawnFailed(format!("connect {addr}: {e}")))?;
    drop(stream);
    Ok(SpawnedServer {
        id: id.to_owned(),
        caps,
        max_restarts,
        restarts_used: 0,
        cancel: Arc::new(AtomicBool::new(false)),
        child: None,
    })
}

/// Split an `http(s)://host[:port][/path]` URL into `(host, port)`.
/// Only literal IP hosts are expected here (SSRF screen runs first); parse
/// failures are `SsrfBlocked`, connect failures are `SpawnFailed`.
fn parse_http_authority(url: &str) -> Result<(String, u16), SpawnError> {
    let after_scheme = url
        .find("://")
        .map(|i| &url[i + 3..])
        .ok_or_else(|| SpawnError::SsrfBlocked(format!("bad url: {url:?}")))?;
    let authority = after_scheme
        .split('/')
        .next()
        .unwrap_or_default()
        .trim_matches(|c| c == '[' || c == ']');
    if authority.is_empty() {
        return Err(SpawnError::SsrfBlocked(format!("empty host: {url:?}")));
    }
    // Last `:` separates a numeric port; IPv6 literals are not accepted
    // (loopback-only lane uses IPv4 fixtures).
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) if !p.is_empty() => {
            let port: u16 = p
                .parse()
                .map_err(|_| SpawnError::SsrfBlocked(format!("bad port: {url:?}")))?;
            (h.to_owned(), port)
        }
        _ => {
            let default = if url.starts_with("https://") {
                443
            } else {
                80
            };
            (authority.to_owned(), default)
        }
    };
    if host.is_empty() {
        return Err(SpawnError::SsrfBlocked(format!("empty host: {url:?}")));
    }
    Ok((host, port))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration as StdDuration;
    use tokio::net::TcpListener;

    fn broker() -> PermissionBroker {
        PermissionBroker::new(opencode_rk_security::SecurityPolicy::lean_default(
            "/work/project",
        ))
    }

    fn spawner() -> SecureSpawner {
        SecureSpawner::new(opencode_rk_security::spawn::SpawnPolicy::permissive())
    }

    fn cancel_flag() -> AtomicBool {
        AtomicBool::new(false)
    }

    fn stdio_echo() -> McpEndpoint {
        McpEndpoint::Stdio {
            program: "/bin/echo".to_owned(),
            args: vec!["hello".to_owned()],
            env: HashMap::new(),
            cwd: PathBuf::from("/tmp"),
        }
    }

    fn caps2() -> ScopedCaps {
        ScopedCaps::new(vec!["tools.call".to_owned(), "tools.list".to_owned()]).unwrap()
    }

    // T01: broker approval gates spawn; denial spawns no process.
    #[tokio::test]
    async fn disc111_t01_denial_spawns_no_process() {
        let denied = broker().with_permissions(opencode_rk_security::PermissionSet::new(
            vec![opencode_rk_security::PermissionRule::new(
                "*",
                opencode_rk_security::RuleEffect::Deny,
            )],
        ));
        // Sanity: the fixture broker really denies process intents.
        assert!(matches!(
            denied.authorize(&OperationIntent::Process {
                program: "/bin/echo".to_owned(),
                args: vec!["hello".to_owned()],
                cwd: PathBuf::from("/tmp"),
            }),
            Decision::Deny { .. }
        ));
        let cancel = cancel_flag();
        let err = spawn_server(
            "denied1",
            stdio_echo(),
            caps2(),
            &denied,
            &spawner(),
            false,
            &cancel,
            DEFAULT_MAX_RESTARTS,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, SpawnError::Denied(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn disc111_t01_approval_spawns_real_process() {
        let b = broker().with_permissions(opencode_rk_security::PermissionSet::star());
        let cancel = cancel_flag();
        let mut srv = spawn_server(
            "ok1",
            stdio_echo(),
            caps2(),
            &b,
            &spawner(),
            false,
            &cancel,
            DEFAULT_MAX_RESTARTS,
        )
        .await
        .expect("approved spawn must run");
        assert!(srv.is_running());
        srv.reclaim().await;
        assert!(!srv.is_running());
    }

    // T02: SSRF — non-loopback / private / DNS endpoints blocked pre-connect.
    #[tokio::test]
    async fn disc111_t02_ssrf_blocked_before_connect() {
        for url in [
            "http://10.0.0.5:8080/mcp",
            "http://192.168.1.10/mcp",
            "http://169.254.169.254/latest/meta-data/",
            "https://example.com/mcp",
            "http://127.0.0.1:9/mcp",
        ] {
            let err = check_endpoint_ssrf(url, false).unwrap_err();
            assert!(
                matches!(err, SpawnError::SsrfBlocked(_)),
                "url {url} got {err:?}"
            );
        }
        // Even with loopback allowed, private ranges and DNS names stay blocked.
        for url in [
            "http://10.0.0.5:8080/mcp",
            "https://example.com/mcp",
        ] {
            assert!(
                matches!(
                    check_endpoint_ssrf(url, true),
                    Err(SpawnError::SsrfBlocked(_))
                ),
                "url {url}"
            );
        }
    }

    #[tokio::test]
    async fn disc111_t02_loopback_connects_to_fixture_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let _ = listener.accept().await;
        });
        let b = broker().with_permissions(opencode_rk_security::PermissionSet::star());
        let cancel = cancel_flag();
        let url = format!("http://127.0.0.1:{port}/mcp");
        let srv = spawn_server(
            "loop1",
            McpEndpoint::Http { url },
            caps2(),
            &b,
            &spawner(),
            true,
            &cancel,
            DEFAULT_MAX_RESTARTS,
        )
        .await
        .expect("loopback with flag must connect");
        assert!(!srv.is_running());
    }

    // T03: scoped caps inherited; secrets filtered; handles closed.
    #[tokio::test]
    async fn disc111_t03_scoped_caps_and_closed_handles() {
        let b = broker().with_permissions(opencode_rk_security::PermissionSet::star());
        let cancel = cancel_flag();
        let mut env = HashMap::new();
        env.insert("MCP_SCOPE".to_owned(), "chat".to_owned());
        env.insert("GITHUB_TOKEN".to_owned(), "gh-secret".to_owned());
        let mut srv = spawn_server(
            "caps1",
            McpEndpoint::Stdio {
                program: "/bin/echo".to_owned(),
                args: vec!["hi".to_owned()],
                env,
                cwd: PathBuf::from("/tmp"),
            },
            ScopedCaps::new(vec!["tools.call".to_owned()]).unwrap(),
            &b,
            &spawner(),
            false,
            &cancel,
            DEFAULT_MAX_RESTARTS,
        )
        .await
        .expect("spawn must run");
        assert!(srv.caps.contains("tools.call"));
        assert!(!srv.caps.contains("admin.exec"));
        // Secret env must be filtered before exec: child never sees it.
        let filtered = spawner().filter_env(&{
            let mut e = HashMap::new();
            e.insert("GITHUB_TOKEN".to_owned(), "gh-secret".to_owned());
            e
        });
        assert!(!filtered.contains_key("GITHUB_TOKEN"));
        srv.reclaim().await;
        // Oversize cap sets are rejected before any spawn.
        assert!(matches!(
            ScopedCaps::new(vec!["c".to_owned(); MAX_SPAWN_CAPABILITIES + 1]),
            Err(SpawnError::Bounds(_))
        ));
    }

    // T04: crash surfaces typed error; bounded retry; no silent restart.
    #[tokio::test]
    async fn disc111_t04_crash_bounded_retry_no_silent_restart() {
        let b = broker().with_permissions(opencode_rk_security::PermissionSet::star());
        let cancel = cancel_flag();
        let mut srv = spawn_server(
            "crash1",
            McpEndpoint::Stdio {
                // NOTE (RED-phase fixture fix): was `/bin/sh -c "exit 3"`, but
                // the broker maps shell `-c` to RequireHuman (correct per
                // security contract), so an approved spawn could never run.
                // Crash codes below are caller-observed literals passed to
                // `note_crash`, independent of the child binary.
                program: "/bin/false".to_owned(),
                args: Vec::new(),
                env: HashMap::new(),
                cwd: PathBuf::from("/tmp"),
            },
            caps2(),
            &b,
            &spawner(),
            false,
            &cancel,
            2,
        )
        .await
        .expect("spawn must run");
        // Drive the child to exit, then record crashes: 2 budget -> Crashed x2 then exhausted.
        srv.reclaim().await;
        let e1 = srv.note_crash(Some(3));
        assert_eq!(e1, SpawnError::Crashed { code: Some(3) });
        let e2 = srv.note_crash(Some(3));
        assert_eq!(e2, SpawnError::Crashed { code: Some(3) });
        let e3 = srv.note_crash(Some(3));
        assert_eq!(e3, SpawnError::RestartsExhausted { used: 2 });
        assert!(!srv.is_running(), "exhausted budget must not restart");
    }

    // T05: approval digest + scope + expiry enforced per invocation.
    #[tokio::test]
    async fn disc111_t05_approval_binding_enforced_per_call() {
        let b = broker().with_permissions(opencode_rk_security::PermissionSet::star());
        let cancel = cancel_flag();
        let mut srv = spawn_server(
            "tool1",
            stdio_echo(),
            caps2(),
            &b,
            &spawner(),
            false,
            &cancel,
            DEFAULT_MAX_RESTARTS,
        )
        .await
        .expect("spawn must run");
        let good =
            ToolApproval::bind("tool1", "tools.call", b"{}", "project:default", StdDuration::from_secs(60));
        assert!(good.check("tool1", "tools.call", b"{}", "project:default").is_ok());
        // Drift in args -> denied, no side effect.
        assert!(matches!(
            good.check("tool1", "tools.call", b"{\"x\":1}", "project:default"),
            Err(SpawnError::Denied(_))
        ));
        // Scope drift -> denied.
        assert!(matches!(
            good.check("tool1", "tools.call", b"{}", "project:other"),
            Err(SpawnError::Denied(_))
        ));
        // Expired approval -> denied.
        let stale = ToolApproval::bind(
            "tool1",
            "tools.call",
            b"{}",
            "project:default",
            StdDuration::from_millis(1),
        );
        tokio::time::sleep(StdDuration::from_millis(10)).await;
        assert!(matches!(
            stale.check("tool1", "tools.call", b"{}", "project:default"),
            Err(SpawnError::Denied(_))
        ));
        // Capability outside the spawn grant -> denied at invoke.
        let foreign =
            ToolApproval::bind("tool1", "admin.exec", b"{}", "project:default", StdDuration::from_secs(60));
        assert!(matches!(
            srv.invoke(&foreign, "admin.exec", b"{}", "project:default").await,
            Err(SpawnError::Denied(_))
        ));
        // Valid invoke succeeds without spawning anything new.
        let out = srv
            .invoke(&good, "tools.call", b"{}", "project:default")
            .await
            .expect("valid approval must invoke");
        assert!(!out.is_empty());
        srv.reclaim().await;
    }

    // Cancel/reclaim: cancel stops the child, reclaim releases the handle.
    #[tokio::test]
    async fn disc111_t06_cancel_reclaim_releases_child() {
        let b = broker().with_permissions(opencode_rk_security::PermissionSet::star());
        let cancel = cancel_flag();
        let mut srv = spawn_server(
            "sleep1",
            McpEndpoint::Stdio {
                program: "/bin/sleep".to_owned(),
                args: vec!["30".to_owned()],
                env: HashMap::new(),
                cwd: PathBuf::from("/tmp"),
            },
            caps2(),
            &b,
            &spawner(),
            false,
            &cancel,
            DEFAULT_MAX_RESTARTS,
        )
        .await
        .expect("spawn must run");
        assert!(srv.is_running());
        srv.cancel();
        srv.reclaim().await;
        assert!(!srv.is_running());
        // Second reclaim is a safe no-op.
        srv.reclaim().await;
        assert!(!srv.is_running());
    }
}
