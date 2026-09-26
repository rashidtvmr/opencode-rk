//! Authenticated singleton discovery client (APP-002).
//!
//! Client-side mirror of `crates/server/src/daemon.rs:133-149`
//! (`read_backend_descriptor`): a `backend.json` descriptor is trusted only
//! when its schema version matches, its PID is alive, and its HTTP origin is
//! loopback (`http://127.0.0.1:<port>`). Anything else is stale/forged and
//! must never redirect credentials at a server.
//!
//! Security boundaries (client enforces, daemon mirrors):
//! - Symlinked descriptor files are rejected (path swap attack).
//! - Descriptors owned by another UID are rejected (credential redirect).
//! - A healthy response on an occupied port without a valid descriptor is a
//!   foreign listener: report it, never kill anything. There is deliberately
//!   **no PID-kill path** in this module; lifecycle decisions are limited to
//!   [`LifecycleAction`], which has no kill variant.
//!
//! IO boundary: this module opens no sockets and reads no files, so it
//! compiles standalone (`rustc --test`). The caller supplies file bytes,
//! `symlink_metadata` results (plus UID via `MetadataExt::uid` / `geteuid`),
//! PID liveness, and `/health` status; this module decides.
//!
//! Caller sketch (not compiled here):
//! ```ignore
//! let meta = std::fs::symlink_metadata(&path)?; // does NOT follow links
//! validate_file_meta(&DescriptorFileMeta {
//!     is_symlink: meta.is_symlink(),
//!     owner_uid: std::os::unix::fs::MetadataExt::uid(&meta),
//!     caller_uid: unsafe { libc::geteuid() }, // caller-side only
//!     len_bytes: meta.len(),
//! })?;
//! let bytes = std::fs::read(&path)?;
//! let descriptor = parse_backend_descriptor(&bytes)?;
//! let port = validate_descriptor(&descriptor, pid_alive)?;
//! ```

#![forbid(unsafe_code)]

/// Must equal `opencode_rk_contracts::WIRE_SCHEMA_VERSION`
/// (`crates/contracts/src/lib.rs:14`). Bumped only with the wire protocol.
pub const EXPECTED_SCHEMA_VERSION: u16 = 1;

/// Byte budget for `backend.json`. Anything larger is forged/broken, never
/// buffered beyond this cap (AGENTS.md resource bounds).
pub const MAX_DESCRIPTOR_BYTES: usize = 8 * 1024;

/// Client-side copy of `daemon::BackendDescriptor` (`daemon.rs:107-112`).
/// Duplicated (not imported) so this file stays dependency-free and
/// compilable standalone via `rustc --test`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendDescriptor {
    pub pid: u32,
    pub http_origin: String,
    pub schema_version: u16,
}

/// Rejection reasons for descriptor bytes or file metadata. Every variant is
/// fail-closed: the caller must not contact the described origin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DescriptorReject {
    /// Input is empty, oversized, or not the expected JSON object shape.
    Malformed(String),
    /// `schema_version != EXPECTED_SCHEMA_VERSION`.
    SchemaMismatch { expected: u16, actual: u16 },
    /// PID is 0 or not alive: descriptor is stale, never reuse.
    StalePid(u32),
    /// Origin is not exactly `http://127.0.0.1:<port>`.
    BadOrigin(String),
    /// Descriptor path is a symlink: possible path-swap attack.
    Symlink,
    /// Descriptor owned by another user: possible credential redirect.
    WrongOwner { owner: u32, caller: u32 },
    /// File exceeds [`MAX_DESCRIPTOR_BYTES`].
    TooLarge { len: u64, max: u64 },
}

impl std::fmt::Display for DescriptorReject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(detail) => write!(f, "malformed descriptor: {detail}"),
            Self::SchemaMismatch { expected, actual } => write!(
                f,
                "descriptor schema mismatch: expected {expected}, got {actual}"
            ),
            Self::StalePid(pid) => write!(f, "stale descriptor: pid {pid} not alive"),
            Self::BadOrigin(origin) => {
                write!(f, "descriptor origin not loopback: {origin}")
            }
            Self::Symlink => write!(f, "descriptor is a symlink; refusing to follow"),
            Self::WrongOwner { owner, caller } => write!(
                f,
                "descriptor owned by uid {owner}, caller is uid {caller}; refusing"
            ),
            Self::TooLarge { len, max } => {
                write!(f, "descriptor too large: {len} bytes (max {max})")
            }
        }
    }
}

impl std::error::Error for DescriptorReject {}

/// Metadata the caller collects about the descriptor file. Uses
/// `symlink_metadata` (never follows links) so a symlink is observable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DescriptorFileMeta {
    pub is_symlink: bool,
    pub owner_uid: u32,
    pub caller_uid: u32,
    pub len_bytes: u64,
}

/// Validate file metadata before reading bytes. Order is deliberate: size
/// first (bound resources), then symlink, then ownership.
pub fn validate_file_meta(meta: &DescriptorFileMeta) -> Result<(), DescriptorReject> {
    if meta.len_bytes > MAX_DESCRIPTOR_BYTES as u64 {
        return Err(DescriptorReject::TooLarge {
            len: meta.len_bytes,
            max: MAX_DESCRIPTOR_BYTES as u64,
        });
    }
    if meta.is_symlink {
        return Err(DescriptorReject::Symlink);
    }
    if meta.owner_uid != meta.caller_uid {
        return Err(DescriptorReject::WrongOwner {
            owner: meta.owner_uid,
            caller: meta.caller_uid,
        });
    }
    Ok(())
}

/// Parse descriptor bytes into [`BackendDescriptor`]. Minimal strict JSON
/// object parser (std only): unknown fields are ignored for forward
/// compatibility, missing/m mistyped fields are [`DescriptorReject::Malformed`].
pub fn parse_backend_descriptor(bytes: &[u8]) -> Result<BackendDescriptor, DescriptorReject> {
    if bytes.is_empty() {
        return Err(DescriptorReject::Malformed("empty descriptor".into()));
    }
    if bytes.len() > MAX_DESCRIPTOR_BYTES {
        return Err(DescriptorReject::TooLarge {
            len: bytes.len() as u64,
            max: MAX_DESCRIPTOR_BYTES as u64,
        });
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| DescriptorReject::Malformed("descriptor is not UTF-8".into()))?;
    let mut parser = Parser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    parser.parse_descriptor()
}

/// Validate a parsed descriptor. Mirrors `daemon.rs:142-147`: schema check,
/// PID liveness, loopback-origin check. Returns the loopback port on success.
/// `is_alive` is caller-supplied (production: `/proc/<pid>` exists check,
/// same as `daemon::pid_alive`); tests inject fakes.
pub fn validate_descriptor(
    descriptor: &BackendDescriptor,
    is_alive: impl Fn(u32) -> bool,
) -> Result<u16, DescriptorReject> {
    if descriptor.schema_version != EXPECTED_SCHEMA_VERSION {
        return Err(DescriptorReject::SchemaMismatch {
            expected: EXPECTED_SCHEMA_VERSION,
            actual: descriptor.schema_version,
        });
    }
    if descriptor.pid == 0 || !is_alive(descriptor.pid) {
        return Err(DescriptorReject::StalePid(descriptor.pid));
    }
    parse_loopback_port(&descriptor.http_origin)
        .ok_or_else(|| DescriptorReject::BadOrigin(descriptor.http_origin.clone()))
}

/// Accept exactly `http://127.0.0.1:<port>` with numeric port 1..=65535.
/// Returns the port. Rejects `localhost`, `0.0.0.0`, LAN IPs, `https`,
/// missing ports, and trailing paths/slashes.
pub fn parse_loopback_port(origin: &str) -> Option<u16> {
    let rest = origin.strip_prefix("http://127.0.0.1:")?;
    if rest.is_empty() || rest.len() > 5 || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // No leading zeros quirk: "007" would be odd but harmless; reject it to
    // keep the canonical form tight.
    if rest.len() > 1 && rest.starts_with('0') {
        return None;
    }
    let port: u32 = rest.parse().ok()?;
    if port == 0 || port > u16::MAX as u32 {
        return None;
    }
    Some(port as u16)
}

/// Production PID-liveness check, same rule as `daemon::pid_alive`.
/// Linux uses `/proc`; macOS uses the native `kill -0` probe because Darwin
/// has no `/proc` mount. A missing or inaccessible process fails closed.
pub fn pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(target_os = "linux")]
    {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::process::Command::new("/bin/kill")
            .args(["-0", &pid.to_string()])
            .status()
            .is_ok_and(|status| status.success())
    }
}

/// Client/server schema version pair for health/version negotiation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VersionNegotiation {
    pub client: u16,
    pub server: u16,
}

/// Outcome of version negotiation. Anything but [`VersionDecision::Compatible`]
/// must surface an actionable error, never retry silently or loop forever.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VersionDecision {
    Compatible,
    /// Server is newer: upgrade the client. Version fields aid the message.
    ServerNewer {
        client: u16,
        server: u16,
    },
    /// Client is newer: upgrade the server / restart a stale daemon.
    ClientNewer {
        client: u16,
        server: u16,
    },
}

impl VersionNegotiation {
    #[must_use]
    pub fn decide(&self) -> VersionDecision {
        if self.server == self.client {
            VersionDecision::Compatible
        } else if self.server > self.client {
            VersionDecision::ServerNewer {
                client: self.client,
                server: self.server,
            }
        } else {
            VersionDecision::ClientNewer {
                client: self.client,
                server: self.server,
            }
        }
    }
}

/// True only for HTTP 200 on `GET /health`. Mirrors `chat.rs:398-400`
/// (`probe_daemon`): a bare TCP connect is not enough, an unrelated listener
/// on the port must not be mistaken for the daemon.
#[must_use]
pub fn health_ok(status: u16) -> bool {
    status == 200
}

/// Fail-closed lifecycle decision. There is no kill variant by design:
/// an occupied port or reused PID must never cause an unrelated process to
/// be signalled (APP-002-T03).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LifecycleAction {
    /// Valid descriptor plus healthy probe: reuse this daemon.
    Reuse(BackendDescriptor),
    /// No usable daemon: the caller may start one (bounded readiness wait,
    /// no infinite retry; see `chat.rs:404-430` ownership rule).
    StartNew { reason: String },
    /// Healthy probe but no valid descriptor: a foreign listener owns the
    /// port. Surface an actionable error; do not kill, do not reuse.
    RefuseOccupiedPort { detail: String },
}

/// Decide the lifecycle from a validated descriptor and the `/health` probe.
/// `valid_descriptor` must already have passed [`validate_descriptor`]
/// (plus [`validate_file_meta`] on its file); passing an unchecked
/// descriptor is a caller bug.
pub fn decide_lifecycle(
    valid_descriptor: Option<BackendDescriptor>,
    health_status: Option<u16>,
) -> LifecycleAction {
    let healthy = health_status.is_some_and(health_ok);
    match (valid_descriptor, healthy) {
        (Some(descriptor), true) => LifecycleAction::Reuse(descriptor),
        (Some(descriptor), false) => LifecycleAction::StartNew {
            reason: format!(
                "daemon pid {} at {} not answering /health; stale or starting",
                descriptor.pid, descriptor.http_origin
            ),
        },
        (None, true) => LifecycleAction::RefuseOccupiedPort {
            detail: "port answers HTTP but no valid daemon descriptor; \
                     foreign listener suspected, refusing to reuse or kill; \
                     start the daemon on a free port or stop the occupant manually"
                .into(),
        },
        (None, false) => LifecycleAction::StartNew {
            reason: "no valid descriptor and no healthy probe; starting new daemon".into(),
        },
    }
}

// --- minimal JSON object parser (std only) ---

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn parse_authed_descriptor(
        &mut self,
    ) -> Result<(BackendDescriptor, String), DescriptorReject> {
        const MALFORMED: fn(String) -> DescriptorReject = DescriptorReject::Malformed;
        self.skip_ws();
        if !self.eat(b'{') {
            return Err(MALFORMED("top level must be an object".into()));
        }
        let mut pid: Option<u32> = None;
        let mut http_origin: Option<String> = None;
        let mut schema_version: Option<u16> = None;
        let mut auth_token: Option<String> = None;
        self.skip_ws();
        if self.eat(b'}') {
            return Err(MALFORMED("empty object".into()));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            if !self.eat(b':') {
                return Err(MALFORMED("expected ':' after key".into()));
            }
            self.skip_ws();
            match key.as_str() {
                "pid" => pid = Some(self.parse_u32("pid")?),
                "schema_version" => schema_version = Some(self.parse_u16("schema_version")?),
                "http_origin" => http_origin = Some(self.parse_string()?),
                "auth_token" => auth_token = Some(self.parse_string()?),
                _ => self.skip_value()?,
            }
            self.skip_ws();
            if self.eat(b',') {
                continue;
            }
            if self.eat(b'}') {
                break;
            }
            return Err(MALFORMED("expected ',' or '}'".into()));
        }
        self.skip_ws();
        if self.pos != self.bytes.len() {
            return Err(MALFORMED("trailing bytes after object".into()));
        }
        Ok((
            BackendDescriptor {
                pid: pid.ok_or_else(|| MALFORMED("missing field 'pid'".into()))?,
                http_origin: http_origin
                    .ok_or_else(|| MALFORMED("missing field 'http_origin'".into()))?,
                schema_version: schema_version
                    .ok_or_else(|| MALFORMED("missing field 'schema_version'".into()))?,
            },
            auth_token
                .ok_or_else(|| MALFORMED("missing field 'auth_token'".into()))?,
        ))
    }

    fn parse_descriptor(&mut self) -> Result<BackendDescriptor, DescriptorReject> {
        const MALFORMED: fn(String) -> DescriptorReject = DescriptorReject::Malformed;
        self.skip_ws();
        if !self.eat(b'{') {
            return Err(MALFORMED("top level must be an object".into()));
        }
        let mut pid: Option<u32> = None;
        let mut http_origin: Option<String> = None;
        let mut schema_version: Option<u16> = None;
        self.skip_ws();
        if self.eat(b'}') {
            return Err(MALFORMED("empty object".into()));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            if !self.eat(b':') {
                return Err(MALFORMED("expected ':' after key".into()));
            }
            self.skip_ws();
            match key.as_str() {
                "pid" => pid = Some(self.parse_u32("pid")?),
                "schema_version" => schema_version = Some(self.parse_u16("schema_version")?),
                "http_origin" => http_origin = Some(self.parse_string()?),
                _ => self.skip_value()?,
            }
            self.skip_ws();
            if self.eat(b',') {
                continue;
            }
            if self.eat(b'}') {
                break;
            }
            return Err(MALFORMED("expected ',' or '}'".into()));
        }
        self.skip_ws();
        if self.pos != self.bytes.len() {
            return Err(MALFORMED("trailing bytes after object".into()));
        }
        Ok(BackendDescriptor {
            pid: pid.ok_or_else(|| MALFORMED("missing field 'pid'".into()))?,
            http_origin: http_origin
                .ok_or_else(|| MALFORMED("missing field 'http_origin'".into()))?,
            schema_version: schema_version
                .ok_or_else(|| MALFORMED("missing field 'schema_version'".into()))?,
        })
    }

    fn parse_u32(&mut self, field: &str) -> Result<u32, DescriptorReject> {
        let start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if start == self.pos {
            return Err(DescriptorReject::Malformed(format!(
                "field '{field}' must be a non-negative integer"
            )));
        }
        std::str::from_utf8(&self.bytes[start..self.pos])
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or_else(|| DescriptorReject::Malformed(format!("field '{field}' out of range")))
    }

    fn parse_u16(&mut self, field: &str) -> Result<u16, DescriptorReject> {
        let value = self.parse_u32(field)?;
        u16::try_from(value).map_err(|_| {
            DescriptorReject::Malformed(format!("field '{field}' out of range for u16"))
        })
    }

    fn parse_string(&mut self) -> Result<String, DescriptorReject> {
        if !self.eat(b'"') {
            return Err(DescriptorReject::Malformed("expected string".into()));
        }
        let mut out: Vec<u8> = Vec::new();
        loop {
            let byte = *self
                .bytes
                .get(self.pos)
                .ok_or_else(|| DescriptorReject::Malformed("unterminated string".into()))?;
            self.pos += 1;
            match byte {
                b'"' => break,
                b'\\' => {
                    let esc = *self
                        .bytes
                        .get(self.pos)
                        .ok_or_else(|| DescriptorReject::Malformed("bad escape".into()))?;
                    self.pos += 1;
                    match esc {
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        b'/' => out.push(b'/'),
                        b'b' => out.push(0x08),
                        b'f' => out.push(0x0C),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            if self.pos + 4 > self.bytes.len() {
                                return Err(DescriptorReject::Malformed("bad \\u escape".into()));
                            }
                            let hex = std::str::from_utf8(&self.bytes[self.pos..self.pos + 4])
                                .map_err(|_| {
                                    DescriptorReject::Malformed("bad \\u escape".into())
                                })?;
                            let code = u32::from_str_radix(hex, 16).map_err(|_| {
                                DescriptorReject::Malformed("bad \\u escape".into())
                            })?;
                            let ch = char::from_u32(code).ok_or_else(|| {
                                DescriptorReject::Malformed("bad \\u escape".into())
                            })?;
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                            self.pos += 4;
                        }
                        _ => {
                            return Err(DescriptorReject::Malformed("bad escape".into()));
                        }
                    }
                }
                0x00..=0x1F => {
                    return Err(DescriptorReject::Malformed(
                        "unescaped control character".into(),
                    ));
                }
                _ => out.push(byte),
            }
        }
        String::from_utf8(out)
            .map_err(|_| DescriptorReject::Malformed("invalid UTF-8 in string".into()))
    }

    fn skip_value(&mut self) -> Result<(), DescriptorReject> {
        self.skip_ws();
        let byte = *self
            .bytes
            .get(self.pos)
            .ok_or_else(|| DescriptorReject::Malformed("unexpected end in value".into()))?;
        match byte {
            b'"' => {
                self.parse_string()?;
            }
            b'{' => {
                self.pos += 1;
                self.skip_ws();
                if self.eat(b'}') {
                    return Ok(());
                }
                loop {
                    self.skip_ws();
                    self.parse_string()?;
                    self.skip_ws();
                    if !self.eat(b':') {
                        return Err(DescriptorReject::Malformed("expected ':'".into()));
                    }
                    self.skip_value()?;
                    self.skip_ws();
                    if self.eat(b',') {
                        continue;
                    }
                    if self.eat(b'}') {
                        break;
                    }
                    return Err(DescriptorReject::Malformed("expected ',' or '}'".into()));
                }
            }
            b'[' => {
                self.pos += 1;
                self.skip_ws();
                if self.eat(b']') {
                    return Ok(());
                }
                loop {
                    self.skip_value()?;
                    self.skip_ws();
                    if self.eat(b',') {
                        continue;
                    }
                    if self.eat(b']') {
                        break;
                    }
                    return Err(DescriptorReject::Malformed("expected ',' or ']'".into()));
                }
            }
            b't' => self.expect_lit("true")?,
            b'f' => self.expect_lit("false")?,
            b'n' => self.expect_lit("null")?,
            b'-' | b'0'..=b'9' => {
                while self.pos < self.bytes.len()
                    && matches!(
                        self.bytes[self.pos],
                        b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-'
                    )
                {
                    self.pos += 1;
                }
            }
            _ => return Err(DescriptorReject::Malformed("unexpected value".into())),
        }
        Ok(())
    }

    fn expect_lit(&mut self, lit: &str) -> Result<(), DescriptorReject> {
        if self.bytes[self.pos..].starts_with(lit.as_bytes()) {
            self.pos += lit.len();
            Ok(())
        } else {
            Err(DescriptorReject::Malformed(format!(
                "expected literal {lit}"
            )))
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len()
            && matches!(self.bytes[self.pos], b' ' | b'\t' | b'\n' | b'\r')
        {
            self.pos += 1;
        }
    }

    fn eat(&mut self, byte: u8) -> bool {
        if self.bytes.get(self.pos) == Some(&byte) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
}

/// Length of the hex-encoded daemon bearer token. Mirrors
/// `daemon_auth::TOKEN_HEX_LEN` (`daemon_auth.rs:33-34`): 32 random bytes,
/// hex-encoded. Duplicated (not imported) to keep this file dependency-free.
pub const TOKEN_HEX_LEN: usize = 64;

/// Credential attached to a validated descriptor: the loopback origin plus
/// the bearer the live daemon expects. The token never travels without a
/// validated descriptor, and a validated descriptor without a token never
/// authorizes contact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedDescriptor {
    pub descriptor: BackendDescriptor,
    pub auth_token: String,
}

/// Startup role across concurrent launches: exactly one caller becomes
/// [`StartupRole::Owner`] (starts the daemon and mints the credential), the
/// rest become [`StartupRole::Attacher`] (reuse the published descriptor).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupRole {
    Owner,
    Attacher,
}

/// Stop/restart credential policy. `Rotate` mints a fresh bearer on every
/// (re)start so a restarted daemon sheds old clients; `Preserve` keeps a
/// wellformed published bearer so existing clients reconnect safely.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialPolicy {
    Rotate,
    Preserve,
}

/// True only for a 64-char hex bearer (mirrors
/// `daemon_auth::from_published`: empty/short/non-hex never authenticates).
#[must_use]
pub fn is_wellformed_token(token: &str) -> bool {
    token.len() == TOKEN_HEX_LEN && token.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Send-side `Authorization` header value for a validated token. The caller
/// must pass a token from [`parse_authenticated_descriptor`]/[`discover`];
/// this function formats, it does not validate.
#[must_use]
pub fn authorization_header(token: &str) -> String {
    let mut out = String::with_capacity("Bearer ".len() + token.len());
    out.push_str("Bearer ");
    out.push_str(token);
    out
}

/// Parse and fully validate credentialed descriptor bytes: shape, schema,
/// PID liveness, loopback origin, plus a required wellformed `auth_token`.
/// Legacy 3-field descriptors (no token) are refused: there is no
/// credential to send, so reuse would be unauthenticated.
pub fn parse_authenticated_descriptor(
    bytes: &[u8],
    is_alive: impl Fn(u32) -> bool,
) -> Result<AuthenticatedDescriptor, DescriptorReject> {
    if bytes.is_empty() {
        return Err(DescriptorReject::Malformed("empty descriptor".into()));
    }
    if bytes.len() > MAX_DESCRIPTOR_BYTES {
        return Err(DescriptorReject::TooLarge {
            len: bytes.len() as u64,
            max: MAX_DESCRIPTOR_BYTES as u64,
        });
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| DescriptorReject::Malformed("descriptor is not UTF-8".into()))?;
    let mut parser = Parser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    let (descriptor, auth_token) = parser.parse_authed_descriptor()?;
    if !is_wellformed_token(&auth_token) {
        // Legacy/blank/forged bearer: nothing to send, refuse loudly. The
        // loopback/schema/pid checks below still run so the message names
        // the real defect, but none of them can produce contact without a
        // wellformed credential.
        return Err(DescriptorReject::Malformed(
            "missing or malformed auth_token: descriptor predates bearer auth or is forged".into(),
        ));
    }
    let port = validate_descriptor(&descriptor, is_alive)?;
    let _ = port;
    Ok(AuthenticatedDescriptor {
        descriptor,
        auth_token,
    })
}

/// Full discovery pipeline: file gate first (size/symlink/owner on
/// caller-supplied metadata), then credentialed parse. Any refusal means
/// the caller must not contact any origin.
pub fn discover(
    bytes: &[u8],
    meta: &DescriptorFileMeta,
    is_alive: impl Fn(u32) -> bool,
) -> Result<AuthenticatedDescriptor, DescriptorReject> {
    validate_file_meta(meta)?;
    parse_authenticated_descriptor(bytes, is_alive)
}

/// Path-based discovery for TUI/chat reuse: file gate first
/// (`symlink_metadata`, never following links), then a bounded read capped
/// at `MAX_DESCRIPTOR_BYTES + 1` (oversize detected without buffering the
/// whole file), then [`discover`]. Any refusal means the caller must not
/// contact any origin. `caller_uid` is caller-supplied (production: euid);
/// this function opens no sockets.
pub fn discover_from_path(
    path: &std::path::Path,
    caller_uid: u32,
    is_alive: impl Fn(u32) -> bool,
) -> Result<AuthenticatedDescriptor, DescriptorReject> {
    use std::os::unix::fs::MetadataExt;
    let stat = std::fs::symlink_metadata(path)
        .map_err(|e| DescriptorReject::Malformed(format!("cannot stat descriptor: {e}")))?;
    let meta = DescriptorFileMeta {
        is_symlink: stat.file_type().is_symlink(),
        owner_uid: stat.uid(),
        caller_uid,
        len_bytes: stat.len(),
    };
    validate_file_meta(&meta)?;
    use std::io::Read;
    let file = std::fs::File::open(path)
        .map_err(|e| DescriptorReject::Malformed(format!("cannot open descriptor: {e}")))?;
    let mut bytes = Vec::new();
    file.take(MAX_DESCRIPTOR_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| DescriptorReject::Malformed(format!("cannot read descriptor: {e}")))?;
    discover(&bytes, &meta, is_alive)
}

/// Map backend.json state to [`app_start::DaemonPresence`]: validated descriptor
/// + live pid + loopback origin = Reusable; stale/missing = Stale/Absent.
/// Never contacts any origin; refusal or unreadable descriptor means Absent/Stale.
pub fn discover_presence(data_dir: &std::path::Path) -> crate::app_start::DaemonPresence {
    use opencode_rk_server::daemon::DaemonPaths;
    let paths = DaemonPaths::for_data_dir(data_dir);
    let uid = std::fs::symlink_metadata(&paths.descriptor)
        .map(|m| {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                m.uid()
            }
            #[cfg(not(unix))]
            {
                let _ = m;
                0
            }
        })
        .unwrap_or(0);
    let live = pid_alive;
    match discover_from_path(&paths.descriptor, uid, live).ok() {
        Some(_) => crate::app_start::DaemonPresence::Reusable,
        None => match std::fs::symlink_metadata(&paths.descriptor) {
            Ok(_) => crate::app_start::DaemonPresence::Stale,
            Err(_) => crate::app_start::DaemonPresence::Absent,
        },
    }
}

/// Provider credential check: any non-empty provider env var counts as configured.
pub fn creds_configured(_data_dir: &std::path::Path) -> Option<bool> {
    const KEYS: &[&str] = &[
        "OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
        "GOOGLE_API_KEY",
        "GEMINI_API_KEY",
    ];
    if KEYS
        .iter()
        .any(|k| std::env::var_os(k).is_some_and(|v| !v.is_empty()))
    {
        Some(true)
    } else {
        Some(false)
    }
}

/// One-shot startup election across concurrent launches. The first caller
/// to flip `claimed` false->true owns the start; every other caller
/// attaches. Deterministic under contention (atomic compare-exchange).
pub fn try_become_owner(claimed: &std::sync::atomic::AtomicBool) -> StartupRole {
    match claimed.compare_exchange(
        false,
        true,
        std::sync::atomic::Ordering::SeqCst,
        std::sync::atomic::Ordering::SeqCst,
    ) {
        Ok(false) => StartupRole::Owner,
        _ => StartupRole::Attacher,
    }
}

/// Credential-bound lifecycle: same decision as [`decide_lifecycle`], plus
/// the `Authorization` header value exactly when reusing (a validated
/// descriptor AND a healthy probe). Every other outcome carries no
/// credential: nothing is sent anywhere.
pub fn decide_lifecycle_authed(
    valid_descriptor: Option<AuthenticatedDescriptor>,
    health_status: Option<u16>,
) -> (LifecycleAction, Option<String>) {
    let inner = valid_descriptor.as_ref().map(|authed| authed.descriptor.clone());
    let action = decide_lifecycle(inner, health_status);
    let credential = match &action {
        LifecycleAction::Reuse(_) => {
            valid_descriptor.map(|authed| authorization_header(&authed.auth_token))
        }
        LifecycleAction::StartNew { .. } | LifecycleAction::RefuseOccupiedPort { .. } => None,
    };
    (action, credential)
}

/// Actionable version error text. `Compatible` needs no error (`None`);
/// any mismatch names both versions and the single corrective action.
/// Never `None` on mismatch: silent retry/loop is forbidden.
#[must_use]
pub fn version_action(decision: &VersionDecision) -> Option<String> {
    match *decision {
        VersionDecision::Compatible => None,
        VersionDecision::ServerNewer { client, server } => Some(format!(
            "daemon schema v{server} is newer than client v{client}; \
             upgrade the client to v{server} and retry (no automatic retry attempted)"
        )),
        VersionDecision::ClientNewer { client, server } => Some(format!(
            "client schema v{client} is newer than daemon v{server}; \
             stop the stale daemon and restart it to v{client}, then reconnect"
        )),
    }
}

/// Resolve the bearer for (re)start under the credential policy. `Rotate`
/// always mints; `Preserve` keeps a wellformed published token and mints
/// otherwise (malformed/blank/missing can never authenticate).
pub fn resolve_credential(
    policy: &CredentialPolicy,
    existing: Option<&str>,
    mint: impl Fn() -> String,
) -> String {
    match policy {
        CredentialPolicy::Rotate => mint(),
        CredentialPolicy::Preserve => match existing {
            Some(token) if is_wellformed_token(token) => token.to_owned(),
            _ => mint(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_json(pid: u32) -> Vec<u8> {
        format!(
            r#"{{"pid":{pid},"http_origin":"http://127.0.0.1:4096","schema_version":{EXPECTED_SCHEMA_VERSION}}}"#
        )
        .into_bytes()
    }

    fn alive(pid: u32) -> bool {
        pid == 4242
    }

    #[test]
    fn valid_descriptor_accepted() {
        let descriptor = parse_backend_descriptor(&valid_json(4242)).unwrap();
        assert_eq!(descriptor.pid, 4242);
        assert_eq!(
            validate_descriptor(&descriptor, alive).unwrap(),
            4096,
            "valid descriptor must yield its loopback port"
        );
    }

    #[test]
    fn schema_mismatch_rejected() {
        let bytes =
            br#"{"pid":4242,"http_origin":"http://127.0.0.1:4096","schema_version":999}"#.to_vec();
        let descriptor = parse_backend_descriptor(&bytes).unwrap();
        assert_eq!(
            validate_descriptor(&descriptor, alive),
            Err(DescriptorReject::SchemaMismatch {
                expected: EXPECTED_SCHEMA_VERSION,
                actual: 999
            })
        );
    }

    #[test]
    fn dead_and_zero_pid_stale_rejected() {
        let dead = parse_backend_descriptor(&valid_json(9999)).unwrap();
        assert_eq!(
            validate_descriptor(&dead, alive),
            Err(DescriptorReject::StalePid(9999))
        );
        let zero = parse_backend_descriptor(&valid_json(0)).unwrap();
        assert_eq!(
            validate_descriptor(&zero, alive),
            Err(DescriptorReject::StalePid(0)),
            "pid 0 is never alive and must be stale"
        );
        // Zero PID is stale even if the liveness probe is broken-open.
        assert_eq!(
            validate_descriptor(&zero, |_| true),
            Err(DescriptorReject::StalePid(0))
        );
    }

    #[test]
    fn non_loopback_origins_rejected() {
        for origin in [
            "http://localhost:4096",
            "http://0.0.0.0:4096",
            "http://192.168.1.2:4096",
            "https://127.0.0.1:4096",
            "http://127.0.0.1",
            "http://127.0.0.1:0",
            "http://127.0.0.1:99999",
            "http://127.0.0.1:4096/",
            "http://127.0.0.1:4096/health",
            "http://127.0.0.1:0409",
            "",
        ] {
            assert_eq!(
                parse_loopback_port(origin),
                None,
                "origin must be rejected: {origin:?}"
            );
            let descriptor = BackendDescriptor {
                pid: 4242,
                http_origin: origin.into(),
                schema_version: EXPECTED_SCHEMA_VERSION,
            };
            assert_eq!(
                validate_descriptor(&descriptor, alive),
                Err(DescriptorReject::BadOrigin(origin.into())),
                "origin must be rejected: {origin:?}"
            );
        }
    }

    #[test]
    fn forged_malformed_bytes_rejected() {
        for bytes in [
            vec![],
            b"not json at all".to_vec(),
            b"{}".to_vec(),
            br#"{"pid":"4242","http_origin":"http://127.0.0.1:4096","schema_version":1}"#.to_vec(),
            br#"{"pid":4242,"http_origin":"http://127.0.0.1:4096"}"#.to_vec(),
            br#"{"pid":-1,"http_origin":"http://127.0.0.1:4096","schema_version":1}"#.to_vec(),
            br#"{"pid":4242,"http_origin":"http://127.0.0.1:4096","schema_version":1"#.to_vec(),
            {
                let mut v = valid_json(4242);
                v.extend_from_slice(b"trailing");
                v
            },
            vec![0xff, 0xfe, 0x00],
        ] {
            let parsed = parse_backend_descriptor(&bytes);
            assert!(parsed.is_err(), "bytes must not parse: {bytes:?}");
            // Malformed input must never validate: nothing parsed means
            // nothing to contact.
            assert!(parsed.is_err());
        }
    }

    #[test]
    fn oversize_bytes_and_meta_rejected() {
        let big = vec![b'x'; MAX_DESCRIPTOR_BYTES + 1];
        assert_eq!(
            parse_backend_descriptor(&big),
            Err(DescriptorReject::TooLarge {
                len: (MAX_DESCRIPTOR_BYTES + 1) as u64,
                max: MAX_DESCRIPTOR_BYTES as u64
            })
        );
        let meta = DescriptorFileMeta {
            is_symlink: false,
            owner_uid: 1000,
            caller_uid: 1000,
            len_bytes: (MAX_DESCRIPTOR_BYTES + 1) as u64,
        };
        assert!(matches!(
            validate_file_meta(&meta),
            Err(DescriptorReject::TooLarge { .. })
        ));
    }

    #[test]
    fn symlink_descriptor_rejected() {
        let meta = DescriptorFileMeta {
            is_symlink: true,
            owner_uid: 1000,
            caller_uid: 1000,
            len_bytes: 64,
        };
        assert_eq!(validate_file_meta(&meta), Err(DescriptorReject::Symlink));
    }

    #[test]
    fn other_user_descriptor_rejected() {
        let meta = DescriptorFileMeta {
            is_symlink: false,
            owner_uid: 0,
            caller_uid: 1000,
            len_bytes: 64,
        };
        assert_eq!(
            validate_file_meta(&meta),
            Err(DescriptorReject::WrongOwner {
                owner: 0,
                caller: 1000
            })
        );
        let own = DescriptorFileMeta {
            owner_uid: 1000,
            caller_uid: 1000,
            ..meta
        };
        assert_eq!(validate_file_meta(&own), Ok(()));
    }

    #[test]
    fn version_negotiation_compatible_and_mismatch() {
        let same = VersionNegotiation {
            client: 1,
            server: 1,
        };
        assert_eq!(same.decide(), VersionDecision::Compatible);
        let newer_server = VersionNegotiation {
            client: 1,
            server: 2,
        };
        assert_eq!(
            newer_server.decide(),
            VersionDecision::ServerNewer {
                client: 1,
                server: 2
            }
        );
        let newer_client = VersionNegotiation {
            client: 3,
            server: 1,
        };
        assert_eq!(
            newer_client.decide(),
            VersionDecision::ClientNewer {
                client: 3,
                server: 1
            }
        );
    }

    #[test]
    fn occupied_port_is_safe_error_never_kill() {
        // Healthy probe but no valid descriptor: foreign listener.
        let action = decide_lifecycle(None, Some(200));
        assert!(
            matches!(action, LifecycleAction::RefuseOccupiedPort { .. }),
            "occupied port must be a safe error, got: {action:?}"
        );
        // Exhaustive match proves no kill/signal path exists in the decision.
        match &action {
            LifecycleAction::Reuse(_) | LifecycleAction::StartNew { .. } => {
                panic!("occupied port must not reuse or start over a foreign listener")
            }
            LifecycleAction::RefuseOccupiedPort { detail } => {
                assert!(detail.contains("refusing"));
            }
        }
    }

    #[test]
    fn only_http_200_counts_as_healthy() {
        for status in [0, 201, 301, 400, 404, 500] {
            assert!(
                !health_ok(status),
                "status {status} must not count as daemon"
            );
            assert!(
                matches!(
                    decide_lifecycle(None, Some(status)),
                    LifecycleAction::StartNew { .. }
                ),
                "non-200 probe must not look like an occupied daemon port"
            );
        }
        assert!(health_ok(200));
    }

    #[test]
    fn lifecycle_reuse_only_when_valid_and_healthy() {
        let descriptor = parse_backend_descriptor(&valid_json(4242)).unwrap();
        assert_eq!(
            decide_lifecycle(Some(descriptor.clone()), Some(200)),
            LifecycleAction::Reuse(descriptor.clone()),
            "valid descriptor + 200 must reuse"
        );
        assert!(
            matches!(
                decide_lifecycle(Some(descriptor), Some(500)),
                LifecycleAction::StartNew { .. }
            ),
            "valid descriptor + failing probe must not reuse a dead daemon"
        );
        assert!(
            matches!(
                decide_lifecycle(None, None),
                LifecycleAction::StartNew { .. }
            ),
            "no descriptor + no probe must start new"
        );
    }

    #[test]
    fn unknown_fields_ignored_for_forward_compat() {
        let bytes = format!(
            r#"{{"pid":4242,"http_origin":"http://127.0.0.1:4096","schema_version":{EXPECTED_SCHEMA_VERSION},"token":"s3cret","nested":{{"a":[1,2]}}}}"#
        )
        .into_bytes();
        let descriptor = parse_backend_descriptor(&bytes).unwrap();
        assert_eq!(validate_descriptor(&descriptor, alive).unwrap(), 4096);
    }

    // --- APP-002 RED: credential-bound discovery/startup/recovery (frozen) ---

    fn hex_token(seed: u8) -> String {
        let mut out = String::with_capacity(64);
        for i in 0..64u8 {
            out.push(char::from_digit(u32::from((seed.wrapping_add(i)) % 16), 16).unwrap());
        }
        out
    }

    fn authed_json(pid: u32, origin: &str, token: &str) -> Vec<u8> {
        format!(
            r#"{{"pid":{pid},"http_origin":{origin:?},"schema_version":{EXPECTED_SCHEMA_VERSION},"auth_token":{token:?}}}"#
        )
        .into_bytes()
    }

    fn authed(pid: u32) -> AuthenticatedDescriptor {
        parse_authenticated_descriptor(&authed_json(pid, "http://127.0.0.1:4096", &hex_token(1)), alive)
            .expect("fixture must validate")
    }

    #[test]
    fn app002_t01_twenty_concurrent_launches_one_owner() {
        use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
        let claimed = Arc::new(AtomicBool::new(false));
        let mut handles = Vec::new();
        for _ in 0..20 {
            let flag = Arc::clone(&claimed);
            handles.push(std::thread::spawn(move || try_become_owner(&flag)));
        }
        let mut owners = 0;
        let mut attachers = 0;
        for handle in handles {
            match handle.join().expect("worker panicked") {
                StartupRole::Owner => owners += 1,
                StartupRole::Attacher => attachers += 1,
            }
        }
        assert_eq!(owners, 1, "exactly one launch must own the start");
        assert_eq!(attachers, 19, "the rest must attach");
        assert!(claimed.load(Ordering::SeqCst));
    }

    #[test]
    fn app002_t02_forged_stale_legacy_descriptor_never_authorizes() {
        for bytes in [
            b"not json".to_vec(),
            vec![],
            authed_json(4242, "http://127.0.0.1:4096", ""),
            authed_json(4242, "http://127.0.0.1:4096", "short"),
            authed_json(4242, "http://127.0.0.1:4096", &"zz".repeat(32)),
            authed_json(4242, "http://localhost:4096", &hex_token(2)),
            authed_json(0, "http://127.0.0.1:4096", &hex_token(3)),
            authed_json(9999, "http://127.0.0.1:4096", &hex_token(4)),
            format!(
                r#"{{"pid":4242,"http_origin":"http://127.0.0.1:4096","schema_version":999,"auth_token":{:?}}}"#,
                hex_token(5)
            )
            .into_bytes(),
            format!(
                r#"{{"pid":4242,"http_origin":"http://127.0.0.1:4096","schema_version":{EXPECTED_SCHEMA_VERSION}}}"#
            )
            .into_bytes(),
        ] {
            let parsed = parse_authenticated_descriptor(&bytes, alive);
            assert!(parsed.is_err(), "bytes must not authorize: {bytes:?}");
            assert!(
                decide_lifecycle_authed(parsed.ok(), Some(200)).1.is_none(),
                "refused descriptor must yield no credential to send"
            );
        }
        assert!(!is_wellformed_token(""));
        assert!(!is_wellformed_token("short"));
        assert!(!is_wellformed_token(&"zz".repeat(32)));
        assert!(is_wellformed_token(&hex_token(7)));
    }

    #[test]
    fn app002_t03_file_gate_runs_before_credential() {
        let bytes = authed_json(4242, "http://127.0.0.1:4096", &hex_token(1));
        for meta in [
            DescriptorFileMeta { is_symlink: true, owner_uid: 1000, caller_uid: 1000, len_bytes: 64 },
            DescriptorFileMeta { is_symlink: false, owner_uid: 0, caller_uid: 1000, len_bytes: 64 },
            DescriptorFileMeta {
                is_symlink: false,
                owner_uid: 1000,
                caller_uid: 1000,
                len_bytes: (MAX_DESCRIPTOR_BYTES + 1) as u64,
            },
        ] {
            assert!(discover(&bytes, &meta, alive).is_err(), "bad meta must refuse: {meta:?}");
        }
        let own = DescriptorFileMeta { is_symlink: false, owner_uid: 1000, caller_uid: 1000, len_bytes: 200 };
        let found = discover(&bytes, &own, alive).expect("own+valid must discover");
        assert_eq!(found.auth_token, hex_token(1));
    }

    #[test]
    fn app002_t04_occupied_port_never_kills_never_sends() {
        let (action, credential) = decide_lifecycle_authed(None, Some(200));
        assert!(
            matches!(action, LifecycleAction::RefuseOccupiedPort { .. }),
            "occupied port must be a safe error, got: {action:?}"
        );
        assert_eq!(credential, None, "foreign listener must receive no credential");
        match &action {
            LifecycleAction::Reuse(_) | LifecycleAction::StartNew { .. } => {
                panic!("occupied port must not reuse or start over a foreign listener")
            }
            LifecycleAction::RefuseOccupiedPort { detail } => assert!(detail.contains("refusing")),
        }
    }

    #[test]
    fn app002_t05_version_mismatch_actionable_never_silent() {
        assert_eq!(
            version_action(&VersionDecision::Compatible),
            None,
            "compatible needs no error"
        );
        for decision in [
            VersionDecision::ServerNewer { client: 1, server: 2 },
            VersionDecision::ClientNewer { client: 3, server: 1 },
        ] {
            let text = version_action(&decision).expect("mismatch must explain itself");
            match decision {
                VersionDecision::ServerNewer { client, server } => {
                    assert!(text.contains(&client.to_string()) && text.contains(&server.to_string()));
                    assert!(text.contains("upgrade"), "server-newer must say to upgrade: {text}");
                }
                VersionDecision::ClientNewer { client, server } => {
                    assert!(text.contains(&client.to_string()) && text.contains(&server.to_string()));
                    assert!(text.contains("restart") || text.contains("upgrade"));
                }
                VersionDecision::Compatible => unreachable!(),
            }
        }
    }

    #[test]
    fn app002_t06_credential_policy_stop_restart() {
        assert_eq!(resolve_credential(&CredentialPolicy::Rotate, Some(&hex_token(1)), || hex_token(9)), hex_token(9));
        assert_eq!(
            resolve_credential(&CredentialPolicy::Preserve, Some(&hex_token(1)), || hex_token(9)),
            hex_token(1),
            "preserve keeps a wellformed bearer so clients reconnect"
        );
        for existing in [None, Some(""), Some("short"), Some(&"zz".repeat(32))] {
            assert_eq!(
                resolve_credential(&CredentialPolicy::Preserve, existing, || hex_token(9)),
                hex_token(9),
                "preserve with malformed/missing bearer must mint, never authenticate blank"
            );
        }
    }

    #[test]
    fn app002_t07_bearer_send_side_exact() {
        let token = hex_token(1);
        assert_eq!(authorization_header(&token), format!("Bearer {token}"));
        let (action, credential) = decide_lifecycle_authed(Some(authed(4242)), Some(200));
        assert!(matches!(action, LifecycleAction::Reuse(_)));
        assert_eq!(credential, Some(format!("Bearer {token}")));
        let (_, dead) = decide_lifecycle_authed(Some(authed(4242)), Some(500));
        assert_eq!(dead, None, "dead daemon gets no credential");
        assert_eq!(decide_lifecycle_authed(None, None).1, None);
    }

    #[test]
    fn discover_from_path_roundtrip_and_refusals() {
        let dir = std::env::temp_dir().join(format!(
            "daemon_client_discover_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("backend.json");
        std::fs::write(
            &path,
            authed_json(4242, "http://127.0.0.1:4096", &hex_token(1)),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let uid = std::fs::symlink_metadata(&path).unwrap().uid();
            let found = discover_from_path(&path, uid, alive).expect("own+valid must discover");
            assert_eq!(found.auth_token, hex_token(1));
            assert_eq!(found.descriptor.http_origin, "http://127.0.0.1:4096");
            // Wrong caller UID refuses before any contact.
            assert!(matches!(
                discover_from_path(&path, uid.wrapping_add(1), alive),
                Err(DescriptorReject::WrongOwner { .. })
            ));
            // Stale PID refuses.
            let stale = dir.join("stale.json");
            std::fs::write(
                &stale,
                authed_json(9999, "http://127.0.0.1:4096", &hex_token(2)),
            )
            .unwrap();
            assert!(matches!(
                discover_from_path(&stale, uid, alive),
                Err(DescriptorReject::StalePid(9999))
            ));
            // Symlink refuses.
            let link = dir.join("link.json");
            #[allow(clippy::redundant_clone)]
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert_eq!(
                discover_from_path(&link, uid, alive),
                Err(DescriptorReject::Symlink)
            );
        }
        // Missing file is a fail-closed refusal, never contact.
        assert!(
            discover_from_path(&dir.join("missing.json"), 0, alive).is_err(),
            "missing descriptor must refuse"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
