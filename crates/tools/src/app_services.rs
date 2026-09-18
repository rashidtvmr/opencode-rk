//! Tool-service composition boundary (PAR-005 slice).
//!
//! Pure composition types between the tool registry (`registry.rs:60-133`,
//! lookup only, no dispatch), the executor (`executor.rs:79-130`, spawns via
//! `Command::new("bash -c")` at `:130` with no broker), and the permission
//! stub (`permission.rs:1`). AUD-005 traces this chain as `unwired` and
//! AUD-009 traces subprocess dispatch as `bypass broker`; this module is the
//! types-only half of the repair: `ToolCall` (`id`/`tool`/`args`/`scope`),
//! `ToolResult`, `DispatchError` (`Denied`/`Timeout`/`Cancelled`/`Bounds`),
//! size/timeout caps, and an approval-digest binding helper mirroring the
//! `tool_binding_immutable` trigger and `intent_hash` columns in
//! `crates/storage/schema/v2/workspace.sql:186-220`.
//!
//! No I/O, no threads, no clock, no `Command`: execution bypass is
//! impossible from this module by construction. Validation order is
//! length/shape caps first (`Bounds`), then traversal/absolute-path gates
//! and broker/binding denial (`Denied`), matching the check order convention
//! in `ext_secure.rs:1-2`. Intended dispatch order for callers:
//! `check_cancel` -> `validate` -> `check_binding` -> execute under the
//! returned effective timeout through the permission broker.
//!
//! ponytail: `approval_digest` is a std-only FNV-1a multi-lane fold, not the
//! blake3-256 used for storage checksums (`schema_v2.rs:5-7`), because
//! `blake3` is not an accepted dependency of `crates/tools`. Upgrade path:
//! swap the fold for blake3 when the dependency is approved; the 32-byte
//! shape already matches `intent_hash`.
#![forbid(unsafe_code)]

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

/// Max serialized [`ToolCall::args`] bytes accepted by [`validate`].
/// Mirrors `MAX_ARGS_BYTES` in `plugin_scoped_exec.rs:10`.
pub const MAX_ARGS_BYTES: usize = 4096;
/// Max [`ToolCall::id`] length in bytes (ASCII labels, so bytes == chars).
pub const MAX_ID_LEN: usize = 128;
/// Max [`ToolCall::tool`] length in bytes.
pub const MAX_TOOL_LEN: usize = 128;
/// Max [`ToolCall::scope`] length in bytes.
pub const MAX_SCOPE_LEN: usize = 256;
/// Max single path string inspected by [`validate_path`].
pub const MAX_PATH_LEN: usize = 1024;
/// Effective timeout when [`ToolCall::timeout_ms`] is `None`.
/// Mirrors `TimeoutConfig::default` in `executor.rs:24-31`.
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;
/// Hard ceiling for [`ToolCall::timeout_ms`]. Never clamped silently:
/// callers exceeding it get [`DispatchError::Bounds`].
/// Mirrors `max_timeout_ms` in `executor.rs:29`.
pub const MAX_TIMEOUT_MS: u64 = 300_000;

/// One tool-service invocation. Construction never fails and never touches
/// the host; dispatch requires [`validate`] (caps + path gates),
/// [`check_cancel`], and [`check_binding`] (approval digest).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    /// Caller-assigned correlation id (transport only, not digest-bound).
    pub id: String,
    /// Tool name as registered in the registry (e.g. `"read"`, `"bash"`).
    pub tool: String,
    /// Opaque serialized arguments; size-capped and path-scanned.
    pub args: Vec<u8>,
    /// Execution scope (e.g. `"project:default"`). Relative only: absolute
    /// scopes and `..` segments are denied.
    pub scope: String,
    /// Requested timeout in milliseconds; `None` selects
    /// [`DEFAULT_TIMEOUT_MS`].
    pub timeout_ms: Option<u64>,
}

impl ToolCall {
    /// Build a call with no explicit timeout. Infallible by design; call
    /// [`validate`] before dispatch.
    pub fn new(
        id: impl Into<String>,
        tool: impl Into<String>,
        args: Vec<u8>,
        scope: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            tool: tool.into(),
            args,
            scope: scope.into(),
            timeout_ms: None,
        }
    }

    /// Builder: requested timeout in milliseconds.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }
}

/// Outcome of one dispatched [`ToolCall`]. `output` is already truncated to
/// caller budgets by the executor; this type only carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResult {
    /// Correlation id copied from the call.
    pub id: String,
    /// Whether the tool applied successfully.
    pub ok: bool,
    /// Bounded output bytes (opaque, never logged by this module).
    pub output: Vec<u8>,
    /// Typed failure, if any.
    pub error: Option<DispatchError>,
}

impl ToolResult {
    /// Successful outcome.
    pub fn ok(id: impl Into<String>, output: Vec<u8>) -> Self {
        Self {
            id: id.into(),
            ok: true,
            output,
            error: None,
        }
    }

    /// Failed outcome; `ok` is always false.
    pub fn fail(id: impl Into<String>, error: DispatchError) -> Self {
        Self {
            id: id.into(),
            ok: false,
            output: Vec::new(),
            error: Some(error),
        }
    }
}

/// Every dispatch failure mode. `Denied` never mutates caller state:
/// [`validate`], [`check_cancel`], and [`check_binding`] take `&ToolCall`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchError {
    /// Rejected by a path gate, the permission broker, or a digest
    /// mismatch. Carries a static-shape reason (no secrets, no raw args).
    Denied(String),
    /// Execution exceeded its effective timeout.
    Timeout,
    /// Cancelled via the caller's cancellation flag before or during dispatch.
    Cancelled,
    /// A size/shape/timeout cap was violated. Carries the offending bound.
    Bounds(String),
}

impl fmt::Display for DispatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Denied(reason) => write!(f, "dispatch denied: {reason}"),
            Self::Timeout => write!(f, "dispatch timed out"),
            Self::Cancelled => write!(f, "dispatch cancelled"),
            Self::Bounds(bound) => write!(f, "dispatch out of bounds: {bound}"),
        }
    }
}

impl std::error::Error for DispatchError {}

fn label_ok(s: &str, max: usize) -> bool {
    if s.is_empty() || s.len() > max {
        return false;
    }
    let mut bytes = s.bytes();
    match bytes.next() {
        Some(b) if b.is_ascii_alphanumeric() => (),
        _ => return false,
    }
    s.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

/// True when any `/`- or `\`-separated segment is exactly `..`.
fn has_traversal(s: &str) -> bool {
    s.split(['/', '\\']).any(|seg| seg == "..")
}

/// True for POSIX absolute tokens or Windows drive-absolute tokens
/// (`C:/...`, `C:\...`). Operates on one delimiter-split token, never on
/// raw prose, so `a/b` stays allowed while `/etc/passwd` is gated.
fn token_is_absolute(token: &str) -> bool {
    if token.starts_with('/') {
        return true;
    }
    let b = token.as_bytes();
    b.len() >= 3
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && (b[2] == b'/' || b[2] == b'\\')
}

/// True when the lossy-decoded args contain an absolute-path token.
/// Tokens containing `://` are URLs, not filesystem paths, and are skipped.
fn args_contain_absolute(args: &[u8]) -> bool {
    let text = String::from_utf8_lossy(args);
    text.split([
        ' ', '\t', '\n', '\r', '"', '\'', '`', '{', '}', '[', ']', '(', ')', ',', ';', '=',
        '|', '&', '<', '>', '$',
    ])
    .filter(|t| !t.is_empty() && !t.contains("://"))
    .any(token_is_absolute)
}

/// Validate one path-like string: NUL/oversize are shape errors (`Bounds`);
/// absolute paths and `..` traversal are policy denials (`Denied`).
/// Mirrors the relative-only rule in `plugin_discover.rs:70-89` (`file_ok`)
/// and the absolute/non-normal rejection in `skill_gate.rs:84-97`.
pub fn validate_path(path: &str) -> Result<(), DispatchError> {
    if path.contains('\0') {
        return Err(DispatchError::Bounds("path contains nul byte".to_owned()));
    }
    if path.len() > MAX_PATH_LEN {
        return Err(DispatchError::Bounds(format!(
            "path too long: max {MAX_PATH_LEN}, got {}",
            path.len()
        )));
    }
    if path.starts_with('/') || token_is_absolute(path) {
        return Err(DispatchError::Denied("absolute path".to_owned()));
    }
    if has_traversal(path) {
        return Err(DispatchError::Denied("path traversal `..`".to_owned()));
    }
    Ok(())
}

/// Resolve the timeout to enforce. `None` selects [`DEFAULT_TIMEOUT_MS`];
/// zero and above-[`MAX_TIMEOUT_MS`] values are `Bounds`, never clamped.
pub fn effective_timeout(timeout_ms: Option<u64>) -> Result<u64, DispatchError> {
    match timeout_ms {
        None => Ok(DEFAULT_TIMEOUT_MS),
        Some(0) => Err(DispatchError::Bounds("timeout must be > 0".to_owned())),
        Some(t) if t > MAX_TIMEOUT_MS => Err(DispatchError::Bounds(format!(
            "timeout too large: max {MAX_TIMEOUT_MS}, got {t}"
        ))),
        Some(t) => Ok(t),
    }
}

/// Cancellation gate: `Err(Cancelled)` when the caller's flag is set, `Ok`
/// otherwise. Checked before and after the broker assertion by callers.
pub fn check_cancel(cancel: &AtomicBool) -> Result<(), DispatchError> {
    if cancel.load(Ordering::SeqCst) {
        return Err(DispatchError::Cancelled);
    }
    Ok(())
}

/// Validate a call for dispatch without mutating it. Returns the effective
/// timeout on success. Caps first (`Bounds`), then traversal/absolute gates
/// (`Denied`), then the timeout range (`Bounds`).
pub fn validate(call: &ToolCall) -> Result<u64, DispatchError> {
    if call.id.is_empty() {
        return Err(DispatchError::Bounds("call id is empty".to_owned()));
    }
    if !label_ok(&call.id, MAX_ID_LEN) {
        return Err(DispatchError::Bounds(format!(
            "invalid call id: max {MAX_ID_LEN}"
        )));
    }
    if call.tool.is_empty() {
        return Err(DispatchError::Bounds("tool name is empty".to_owned()));
    }
    if !label_ok(&call.tool, MAX_TOOL_LEN) {
        return Err(DispatchError::Bounds(format!(
            "invalid tool name: max {MAX_TOOL_LEN}"
        )));
    }
    if call.scope.is_empty() {
        return Err(DispatchError::Bounds("scope is empty".to_owned()));
    }
    if call.scope.len() > MAX_SCOPE_LEN {
        return Err(DispatchError::Bounds(format!(
            "scope too long: max {MAX_SCOPE_LEN}, got {}",
            call.scope.len()
        )));
    }
    validate_path(&call.scope)?;
    if call.args.len() > MAX_ARGS_BYTES {
        return Err(DispatchError::Bounds(format!(
            "args too large: max {MAX_ARGS_BYTES}, got {}",
            call.args.len()
        )));
    }
    if call.args.contains(&0) {
        return Err(DispatchError::Bounds("args contain nul byte".to_owned()));
    }
    let args_text = String::from_utf8_lossy(&call.args);
    if has_traversal(&args_text) {
        return Err(DispatchError::Denied("args path traversal `..`".to_owned()));
    }
    if args_contain_absolute(&call.args) {
        return Err(DispatchError::Denied("args absolute path".to_owned()));
    }
    effective_timeout(call.timeout_ms)
}

fn fnv_lane(seed: u64, chunks: &[&[u8]]) -> u64 {
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

/// Deterministic 32-byte approval digest over the approved invocation
/// content: `tool`, `args`, `scope`. The transport id is deliberately
/// excluded (correlation, not invocation binding), as is the timeout
/// (enforcement parameter, not approved content) -- mirroring the immutable
/// binding columns (`name`, `intent_hash`, `input_payload_pk`) guarded by
/// the `tool_binding_immutable` trigger in `workspace.sql:219-220`.
pub fn approval_digest(call: &ToolCall) -> [u8; 32] {
    let chunks: &[&[u8]] = &[
        b"par005-v1",
        b"tool",
        call.tool.as_bytes(),
        b"args",
        &call.args,
        b"scope",
        call.scope.as_bytes(),
    ];
    let lanes = [
        0xcbf29ce484222325,
        0x84222325cbf29ce4,
        0x9e3779b97f4a7c15,
        0xbf58476d1ce4e5b9,
    ];
    let mut out = [0u8; 32];
    for (i, seed) in lanes.iter().enumerate() {
        out[i * 8..(i + 1) * 8].copy_from_slice(&fnv_lane(*seed, chunks).to_le_bytes());
    }
    out
}

/// Lowercase hex of a digest, safe to log (digests are not secrets).
pub fn digest_hex(digest: &[u8; 32]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Verify the call still matches the approved `intent_hash` (e.g. the
/// `intent_hash` stored beside the tool row / approval record). Any drift in
/// tool, args, or scope is `Denied`; the call is never mutated.
pub fn check_binding(call: &ToolCall, intent_hash: &[u8; 32]) -> Result<(), DispatchError> {
    if &approval_digest(call) == intent_hash {
        Ok(())
    } else {
        Err(DispatchError::Denied(
            "approval digest mismatch".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    fn call(id: &str, tool: &str, args: &[u8], scope: &str) -> ToolCall {
        ToolCall::new(id, tool, args.to_vec(), scope)
    }

    fn benign() -> ToolCall {
        call("c1", "read", br#"{"path":"notes/todo.md"}"#, "project:default")
    }

    #[test]
    fn benign_validates_with_default_timeout() {
        let c = benign();
        assert_eq!(validate(&c), Ok(DEFAULT_TIMEOUT_MS));
    }

    #[test]
    fn rejects_traversal_in_scope_without_mutation() {
        let c = call("c2", "read", b"{}", "project/../etc");
        let before = c.clone();
        let err = validate(&c).unwrap_err();
        assert!(matches!(err, DispatchError::Denied(_)));
        assert!(err.to_string().contains("traversal"));
        assert_eq!(c, before, "denied validation must not mutate the call");
    }

    #[test]
    fn rejects_traversal_in_args() {
        let c = call("c3", "read", br#"{"path":"a/../../etc/passwd"}"#, "project:default");
        let err = validate(&c).unwrap_err();
        assert!(matches!(err, DispatchError::Denied(_)));
        assert!(err.to_string().contains("traversal"));
    }

    #[test]
    fn rejects_absolute_path_in_args() {
        for args in [br#"{"path":"/etc/passwd"}"#.as_slice(), b"cat /etc/hosts"] {
            let c = call("c4", "read", args, "project:default");
            let err = validate(&c).unwrap_err();
            assert!(matches!(err, DispatchError::Denied(_)), "args: {args:?}");
        }
    }

    #[test]
    fn rejects_absolute_scope_and_windows_drive() {
        let abs = call("c5", "read", b"{}", "/etc/work");
        assert!(matches!(validate(&abs), Err(DispatchError::Denied(_))));
        let win = call("c6", "read", b"{}", "project:default");
        let mut win_args = win.clone();
        win_args.args = b"open C:\\Windows\\x".to_vec();
        assert!(matches!(validate(&win_args), Err(DispatchError::Denied(_))));
    }

    #[test]
    fn rejects_oversize_args() {
        let big = vec![b'a'; MAX_ARGS_BYTES + 1];
        let c = call("c7", "read", &big, "project:default");
        let err = validate(&c).unwrap_err();
        assert!(matches!(err, DispatchError::Bounds(_)));
        assert_eq!(c.args.len(), MAX_ARGS_BYTES + 1, "no truncation on deny");
    }

    #[test]
    fn rejects_empty_and_bad_labels() {
        assert!(matches!(
            validate(&call("", "read", b"{}", "project:default")),
            Err(DispatchError::Bounds(_))
        ));
        assert!(matches!(
            validate(&call("c8", "", b"{}", "project:default")),
            Err(DispatchError::Bounds(_))
        ));
        assert!(matches!(
            validate(&call("c9", "rm -rf", b"{}", "project:default")),
            Err(DispatchError::Bounds(_))
        ));
        assert!(matches!(
            validate(&call("c10", "read", b"{}", "")),
            Err(DispatchError::Bounds(_))
        ));
    }

    #[test]
    fn timeout_caps_hold() {
        assert_eq!(effective_timeout(None), Ok(DEFAULT_TIMEOUT_MS));
        assert_eq!(effective_timeout(Some(1_000)), Ok(1_000));
        assert_eq!(effective_timeout(Some(MAX_TIMEOUT_MS)), Ok(MAX_TIMEOUT_MS));
        assert!(matches!(
            effective_timeout(Some(0)),
            Err(DispatchError::Bounds(_))
        ));
        assert!(matches!(
            effective_timeout(Some(MAX_TIMEOUT_MS + 1)),
            Err(DispatchError::Bounds(_))
        ));
        let capped = benign().with_timeout(MAX_TIMEOUT_MS + 1);
        assert!(matches!(validate(&capped), Err(DispatchError::Bounds(_))));
    }

    #[test]
    fn cancel_gate_trips() {
        let live = AtomicBool::new(false);
        assert!(check_cancel(&live).is_ok());
        let dead = AtomicBool::new(true);
        assert_eq!(check_cancel(&dead), Err(DispatchError::Cancelled));
    }

    #[test]
    fn approval_digest_stable_and_bound() {
        let a = benign();
        let same = benign();
        assert_eq!(approval_digest(&a), approval_digest(&same));
        let intent = approval_digest(&a);
        assert!(check_binding(&a, &intent).is_ok());
        // Drift in any bound field breaks the binding.
        let mut drifted = a.clone();
        drifted.args = br#"{"path":"notes/other.md"}"#.to_vec();
        assert_ne!(approval_digest(&drifted), intent);
        assert_eq!(
            check_binding(&drifted, &intent),
            Err(DispatchError::Denied("approval digest mismatch".to_owned()))
        );
        // Transport id is not bound: re-correlation keeps approval valid.
        let mut recorrelated = a.clone();
        recorrelated.id = "c99".to_owned();
        assert!(check_binding(&recorrelated, &intent).is_ok());
        // Digest renders as 64 hex chars.
        assert_eq!(digest_hex(&intent).len(), 64);
    }

    #[test]
    fn deny_paths_report_without_side_effects() {
        // Denied validation leaves every field byte-identical.
        let cases = [
            call("d1", "read", br#"{"path":"/abs"}"#, "project:default"),
            call("d2", "read", b"{}", "s/../escape"),
            call("d3", "read", &vec![b'x'; MAX_ARGS_BYTES + 1], "project:default"),
        ];
        for c in &cases {
            let before = c.clone();
            assert!(validate(c).is_err());
            assert_eq!(*c, before);
        }
        // ToolResult::fail always carries ok=false.
        let r = ToolResult::fail("d1", DispatchError::Denied("x".to_owned()));
        assert!(!r.ok);
        assert!(matches!(r.error, Some(DispatchError::Denied(_))));
    }
}
