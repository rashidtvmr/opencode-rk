#![forbid(unsafe_code)]
//! APP-008: headless-engine bridge types.
//!
//! Same-transcript normalization (headless vs TUI compare), meaningful exit
//! codes, capability-query passthrough marker, disconnect-lifecycle contract.
//! std only. No I/O, no threads, no statics. All retained output bounded.

/// Max transcript bytes normalized per call (1 MiB, cut at char boundary).
pub const MAX_TRANSCRIPT_BYTES: usize = 1_048_576;
/// Max chars in a capability service name (1..=128).
pub const MAX_CAPABILITY_NAME_CHARS: usize = 128;

/// Canonicalize one transcript so headless and TUI renders compare equal:
/// CRLF/CR -> LF, ANSI escapes + control chars (except LF/TAB) stripped,
/// trailing spaces/tabs per line removed, leading/trailing blank lines cut.
/// Empty transcript -> "". Interior blank lines preserved.
pub fn normalize_transcript(input: &str) -> String {
    // Bound retained input at a char boundary.
    let mut end = input.len().min(MAX_TRANSCRIPT_BYTES);
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    let cut = &input[..end];
    // CRLF/CR -> LF.
    let lf = cut.replace("\r\n", "\n").replace('\r', "\n");
    // Strip ANSI escape sequences (CSI `ESC [ ... final`, else ESC + 1 char)
    // and drop control chars except LF/TAB.
    let mut clean = String::with_capacity(lf.len());
    let mut chars = lf.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c2 in chars.by_ref() {
                    if ('@'..='~').contains(&c2) {
                        break;
                    }
                }
            } else {
                chars.next();
            }
        } else if c.is_control() && c != '\n' && c != '\t' {
            // drop C0/C1 controls (NUL, BEL, etc.)
        } else {
            clean.push(c);
        }
    }
    // Trim trailing spaces/tabs per line, cut leading/trailing blank lines,
    // preserve interior blank lines.
    let mut lines: Vec<&str> = Vec::new();
    for line in clean.split('\n') {
        let t = line.trim_end_matches([' ', '\t']);
        lines.push(t);
    }
    while lines.first() == Some(&"") {
        lines.remove(0);
    }
    while lines.last() == Some(&"") {
        lines.pop();
    }
    lines.join("\n").trim().to_owned()
}

/// True when both sides normalize to the same canonical form.
pub fn transcripts_match(a: &str, b: &str) -> bool {
    normalize_transcript(a) == normalize_transcript(b)
}

/// Meaningful process exit codes: 0 ok; 1 usage/model; 2 internal/limit;
/// 3 not found; 4 unauthorized; 5 unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExitCode {
    Success = 0,
    Usage = 1,
    Internal = 2,
    NotFound = 3,
    Unauthorized = 4,
    Unavailable = 5,
}

impl ExitCode {
    pub fn code(self) -> u8 {
        self as u8
    }

    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Success),
            1 => Some(Self::Usage),
            2 => Some(Self::Internal),
            3 => Some(Self::NotFound),
            4 => Some(Self::Unauthorized),
            5 => Some(Self::Unavailable),
            _ => None,
        }
    }

    pub fn is_success(self) -> bool {
        matches!(self, Self::Success)
    }

    pub fn meaning(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Usage => "usage or model refusal",
            Self::Internal => "internal error or limit exceeded",
            Self::NotFound => "session or resource not found",
            Self::Unauthorized => "unauthorized",
            Self::Unavailable => "engine unavailable",
        }
    }
}

/// Capability query carrying a live-probe passthrough marker. Only queries
/// built via [`CapabilityQuery::passthrough`] carry the marker; a hardcoded
/// success without a live engine probe must not pass [`accept`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityQuery {
    service: String,
    passthrough: bool,
}

impl CapabilityQuery {
    /// Mark one live capability probe. Err on empty/overlong/control names.
    pub fn passthrough(service: &str) -> Result<Self, &'static str> {
        if service.is_empty() || service.chars().count() > MAX_CAPABILITY_NAME_CHARS {
            return Err("invalid capability name");
        }
        if service.chars().any(|c| c.is_control()) {
            return Err("invalid capability name");
        }
        Ok(Self {
            service: service.to_owned(),
            passthrough: true,
        })
    }

    pub fn service(&self) -> &str {
        &self.service
    }

    pub fn is_passthrough(&self) -> bool {
        self.passthrough
    }
}

/// Live capability answer. `live_probed` is true only for engine probes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityReport {
    pub service: String,
    pub available: bool,
    pub live_probed: bool,
}

/// Record one live engine answer. None unless the query is passthrough-marked.
pub fn report(query: &CapabilityQuery, available: bool) -> Option<CapabilityReport> {
    if !query.is_passthrough() {
        return None;
    }
    Some(CapabilityReport {
        service: query.service.to_owned(),
        available,
        live_probed: true,
    })
}

/// Accept only live-probed answers. Hardcoded success (live_probed=false)
/// yields None, never Some(true).
pub fn accept(report: &CapabilityReport) -> Option<bool> {
    if !report.live_probed {
        return None;
    }
    Some(report.available)
}

/// Disconnect lifecycle contract: Abort cancels in-flight work, Detach
/// leaves it running per the selected policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectPolicy {
    Abort,
    Detach,
}

/// Observed outcome of applying one disconnect policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectOutcome {
    Aborted,
    Detached,
}

/// Apply the selected disconnect contract.
pub fn on_disconnect(policy: DisconnectPolicy) -> DisconnectOutcome {
    match policy {
        DisconnectPolicy::Abort => DisconnectOutcome::Aborted,
        DisconnectPolicy::Detach => DisconnectOutcome::Detached,
    }
}

/// Default daemon host for headless wire building (same as TUI/CI default).
pub const DEFAULT_DAEMON_HOST: &str = "127.0.0.1:4096";
/// Fixed headless turn path: an `/api/*` route so the bearer gate applies.
pub const HEADLESS_TURN_PATH: &str = "/api/sessions/turns";

/// Pure wire builder sharing the TUI (`chat.rs`) / CI (`ci_run.rs`) execution
/// service contract: every `/api/*` path requires `Some("Bearer <token>")`
/// and fails closed otherwise; non-`/api/*` paths (e.g. `/health`) never
/// carry a credential. No socket I/O; returns raw HTTP/1.1 bytes.
pub fn build_api_wire(
    method: &str,
    host: &str,
    path: &str,
    body: &str,
    bearer: Option<&str>,
) -> Result<Vec<u8>, &'static str> {
    if path.starts_with("/api/") {
        let credential = bearer.unwrap_or("").trim();
        if credential.is_empty() {
            return Err("missing daemon credential: refusing unauthenticated /api/* request");
        }
        return Ok(format!(
            "{method} {path} HTTP/1.1\r\nhost: {host}\r\nauthorization: {credential}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes());
    }
    Ok(format!(
        "{method} {path} HTTP/1.1\r\nhost: {host}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes())
}

fn json_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 2);
    for c in input.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Headless single-turn over the same daemon execution service as the TUI:
/// builds the authenticated `POST /api/sessions/turns` wire for `prompt`.
/// Pure (no I/O): the caller sends the returned bytes. Fail-closed: empty
/// prompt -> `Err(Usage)`; missing/blank bearer -> `Err(Unauthorized)` with
/// zero wire bytes built.
pub fn run_headless(prompt: &str, bearer: Option<&str>) -> Result<Vec<u8>, ExitCode> {
    if prompt.is_empty() {
        return Err(ExitCode::Usage);
    }
    if prompt.len() > MAX_TRANSCRIPT_BYTES {
        return Err(ExitCode::Internal);
    }
    let credential = bearer.unwrap_or("").trim();
    if credential.is_empty() {
        return Err(ExitCode::Unauthorized);
    }
    let body = format!("{{\"text\":\"{}\"}}", json_escape(prompt));
    build_api_wire(
        "POST",
        DEFAULT_DAEMON_HOST,
        HEADLESS_TURN_PATH,
        &body,
        Some(credential),
    )
    .map_err(|_| ExitCode::Unauthorized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_transcripts_match_across_clients() {
        let tui = "\x1b[1mhello   \x1b[0m\r\nworld\t \r\n";
        let headless = "hello\nworld\n";
        assert_eq!(normalize_transcript(tui), "hello\nworld");
        assert_eq!(normalize_transcript(headless), "hello\nworld");
        assert!(transcripts_match(tui, headless));
    }

    #[test]
    fn normalization_trims_edges_and_controls() {
        assert_eq!(normalize_transcript("\n\n  a  \n\n"), "a");
        assert_eq!(normalize_transcript(""), "");
        assert_eq!(normalize_transcript("a\x00b"), "ab");
        assert!(transcripts_match("a  \n b", "a\n b"));
        assert!(!transcripts_match("a", "b"));
    }

    #[test]
    fn exit_codes_meaningful() {
        use ExitCode::*;
        assert_eq!(Success.code(), 0);
        assert!(Success.is_success());
        assert!(!Usage.is_success());
        assert!(!Internal.is_success());
        let all = [
            Success,
            Usage,
            Internal,
            NotFound,
            Unauthorized,
            Unavailable,
        ];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.code(), b.code());
            }
            assert_eq!(ExitCode::from_code(a.code()), Some(*a));
            assert!(!a.meaning().is_empty());
        }
        assert_eq!(ExitCode::from_code(99), None);
    }

    #[test]
    fn capability_requires_passthrough_marker() {
        let q = CapabilityQuery::passthrough("fs.read").expect("valid name");
        assert!(q.is_passthrough());
        assert_eq!(q.service(), "fs.read");
        let rep = report(&q, true).expect("live probe accepted");
        assert!(rep.live_probed);
        assert_eq!(accept(&rep), Some(true));
        let fake = CapabilityReport {
            service: "fs.read".to_owned(),
            available: true,
            live_probed: false,
        };
        assert_eq!(accept(&fake), None);
        assert!(CapabilityQuery::passthrough("").is_err());
        assert!(CapabilityQuery::passthrough(&"x".repeat(129)).is_err());
    }

    #[test]
    fn disconnect_follows_selected_contract() {
        assert_eq!(
            on_disconnect(DisconnectPolicy::Abort),
            DisconnectOutcome::Aborted
        );
        assert_eq!(
            on_disconnect(DisconnectPolicy::Detach),
            DisconnectOutcome::Detached
        );
    }

    #[test]
    fn api_wire_carries_bearer() {
        let wire = String::from_utf8(
            build_api_wire(
                "POST",
                "127.0.0.1:4096",
                "/api/sessions/turns",
                "{}",
                Some("Bearer abc"),
            )
            .expect("credentialed /api/* must build"),
        )
        .unwrap();
        assert!(wire.contains("authorization: Bearer abc"));
        assert!(wire.contains("content-length: 2"));
        assert!(wire.starts_with("POST /api/sessions/turns HTTP/1.1\r\n"));
    }

    #[test]
    fn api_wire_missing_bearer_fails_closed() {
        for bearer in [None, Some(""), Some("   ")] {
            let err = build_api_wire("POST", "127.0.0.1:4096", "/api/sessions/turns", "{}", bearer)
                .expect_err("credential-less /api/* must refuse");
            assert!(err.contains("refusing unauthenticated"));
        }
    }

    #[test]
    fn health_probe_carries_no_bearer() {
        let wire = String::from_utf8(
            build_api_wire("GET", "127.0.0.1:4096", "/health", "", None)
                .expect("public /health must build"),
        )
        .unwrap();
        assert!(!wire.to_ascii_lowercase().contains("authorization:"));
        assert!(wire.starts_with("GET /health HTTP/1.1\r\n"));
    }

    #[test]
    fn run_headless_threads_bearer_fail_closed() {
        let wire = run_headless("hello", Some("Bearer tok")).expect("must build");
        let text = String::from_utf8(wire).unwrap();
        assert!(text.contains("authorization: Bearer tok"));
        assert!(text.contains(HEADLESS_TURN_PATH));
        assert_eq!(run_headless("hello", None), Err(ExitCode::Unauthorized));
        assert_eq!(run_headless("hello", Some("  ")), Err(ExitCode::Unauthorized));
        assert_eq!(run_headless("", Some("Bearer tok")), Err(ExitCode::Usage));
    }
}
