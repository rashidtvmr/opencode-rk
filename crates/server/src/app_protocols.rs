//! PAR-009: versioned app-protocol envelope types for SDK/sync/proxy/headless.
//!
//! Pure synchronous wire boundary: versioned envelopes, opaque wire IDs,
//! content digests, idempotency keys, event cursors with resync-required
//! semantics, bounded frame caps and an ephemeral-vs-durable classifier for
//! replay. No I/O, no clock, no threads, no globals, no logging. The caller
//! owns queues, sockets and retained logs.
//!
//! Bounds align with the sibling transports at this revision: a 256 KiB
//! single frame and 256-item / 4 MiB queues match `workspace_proxy`
//! (`MAX_PROXY_ITEM_BYTES`, `MAX_PROXY_QUEUE_ITEMS`,
//! `MAX_PROXY_QUEUE_BYTES`) and `remote_sync` (`MAX_SYNC_EVENT_BYTES`,
//! `MAX_SYNC_QUEUE_ITEMS`, `MAX_SYNC_QUEUE_BYTES`). `WIRE_VERSION` tracks
//! `opencode_rk_contracts::WIRE_SCHEMA_VERSION` (both `1`); version skew
//! fails closed via [`ProtocolError::UnsupportedVersion`].
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt;

/// Wire protocol version spoken by every envelope in this module.
pub const WIRE_VERSION: u16 = 1;
/// Largest single encoded envelope/frame in bytes (256 KiB).
pub const MAX_FRAME_BYTES: usize = 262_144;
/// Largest queued frame count per caller-owned queue.
pub const MAX_QUEUE_ITEMS: usize = 256;
/// Largest queued frame bytes total per caller-owned queue (4 MiB).
pub const MAX_QUEUE_BYTES: usize = 4_194_304;
/// Largest wire-ID length in chars.
pub const MAX_ID_CHARS: usize = 128;
/// Largest idempotency-key length in chars (`request:digest` derived form).
pub const MAX_IDEMPOTENCY_KEY_CHARS: usize = 320;
/// Largest event-type token length in chars.
pub const MAX_EVENT_TYPE_CHARS: usize = 64;

/// Wire failures. Unit variants plus one numeric version only; never carry
/// IDs, keys, digests, payloads or frame bytes, so `Debug`/`Display` output
/// is secret-free by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    /// Envelope version is not [`WIRE_VERSION`]; carries the numeric version only.
    UnsupportedVersion { actual: u16 },
    /// A wire ID failed token validation (empty, oversize, control chars,
    /// path separators or leading punctuation).
    MalformedId,
    /// An idempotency key failed validation.
    MalformedKey,
    /// An op digest failed hex validation.
    MalformedDigest,
    /// Bytes were not valid UTF-8 JSON, not an object envelope, or carried
    /// a structurally invalid field.
    MalformedFrame,
    /// Encoded bytes exceed [`MAX_FRAME_BYTES`]; nothing was parsed.
    FrameTooLarge,
    /// Caller-owned queue is at item or byte cap; queue left byte-identical.
    QueueFull,
    /// Cursor sequence is zero; sequences start at 1 and zero is never valid.
    BadCursor,
    /// Cursor is behind retention or ahead of the log head; the subscriber
    /// must take a fresh snapshot instead of replaying.
    ResyncRequired,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion { actual } => write!(f, "unsupported version: {actual}"),
            Self::MalformedId => write!(f, "malformed id"),
            Self::MalformedKey => write!(f, "malformed idempotency key"),
            Self::MalformedDigest => write!(f, "malformed op digest"),
            Self::MalformedFrame => write!(f, "malformed frame"),
            Self::FrameTooLarge => write!(f, "frame too large"),
            Self::QueueFull => write!(f, "queue full"),
            Self::BadCursor => write!(f, "bad cursor"),
            Self::ResyncRequired => write!(f, "resync required"),
        }
    }
}

impl std::error::Error for ProtocolError {}

/// Token rule shared by IDs, keys and event types: non-empty, within `max`
/// chars, first char ASCII alphanumeric, rest alphanumeric or `.` `_` `-`
/// (keys additionally allow `:` for the derived `request:digest` form).
fn valid_token(value: &str, max_chars: usize, colon: bool) -> bool {
    if value.is_empty() || value.chars().count() > max_chars {
        return false;
    }
    let mut chars = value.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    chars.all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') || (colon && c == ':')
    })
}

macro_rules! wire_id {
    ($name:ident) => {
        #[doc = concat!("Opaque wire identifier (token rule, max [`MAX_ID_CHARS`] chars).")]
        #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Validate caller bytes into an ID. Fails closed on any
            /// malformed input; the rejected value is never retained.
            pub fn parse(value: &str) -> Result<Self, ProtocolError> {
                if valid_token(value, MAX_ID_CHARS, false) {
                    Ok(Self(value.to_string()))
                } else {
                    Err(ProtocolError::MalformedId)
                }
            }

            /// Borrow the raw wire value. No normalization is applied.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            fn validate(&self) -> Result<(), ProtocolError> {
                if valid_token(&self.0, MAX_ID_CHARS, false) {
                    Ok(())
                } else {
                    Err(ProtocolError::MalformedId)
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

wire_id!(RequestId);
wire_id!(DeviceId);
wire_id!(SessionId);

/// Hex content digest labelling the operation bytes (32..=128 hex chars,
/// covering 128-bit through 512-bit digests). Case-insensitive hex;
/// empty, truncated, non-hex or overlong values fail closed.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OpDigest(String);

impl OpDigest {
    /// Validate caller bytes into a digest. Fails closed; never retains input.
    pub fn parse(value: &str) -> Result<Self, ProtocolError> {
        if valid_digest(value) {
            Ok(Self(value.to_string()))
        } else {
            Err(ProtocolError::MalformedDigest)
        }
    }

    /// Borrow the raw wire value. No normalization is applied.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        if valid_digest(&self.0) {
            Ok(())
        } else {
            Err(ProtocolError::MalformedDigest)
        }
    }
}

impl fmt::Display for OpDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

fn valid_digest(value: &str) -> bool {
    (32..=128).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Opaque idempotency key for at-most-once submission. Keys are compared
/// byte-identically by the caller store; this module only validates shape
/// and derives deterministic keys.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    /// Validate caller bytes into a key. Fails closed; never retains input.
    pub fn parse(value: &str) -> Result<Self, ProtocolError> {
        if valid_token(value, MAX_IDEMPOTENCY_KEY_CHARS, true) {
            Ok(Self(value.to_string()))
        } else {
            Err(ProtocolError::MalformedKey)
        }
    }

    /// Borrow the raw wire value. No normalization is applied.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        if valid_token(&self.0, MAX_IDEMPOTENCY_KEY_CHARS, true) {
            Ok(())
        } else {
            Err(ProtocolError::MalformedKey)
        }
    }
}

impl fmt::Display for IdempotencyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Derive a deterministic idempotency key for (`request`, `op`): the exact
/// string `request:digest`. Same inputs always yield the same key, so
/// retries of one operation collapse in the caller store while distinct
/// operations never collide. Inputs are already bounded, so the derived
/// form always fits [`MAX_IDEMPOTENCY_KEY_CHARS`].
pub fn idempotency_key(request: &RequestId, op: &OpDigest) -> Result<IdempotencyKey, ProtocolError> {
    IdempotencyKey::parse(&format!("{}:{}", request.as_str(), op.as_str()))
}

/// Replay cursor: 1-based sequence of the next event the subscriber wants.
/// Zero is never valid (see [`ProtocolError::BadCursor`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventCursor(u64);

impl EventCursor {
    /// Wrap a raw sequence. Zero is rejected later by [`check_cursor`].
    #[must_use]
    pub const fn new(seq: u64) -> Self {
        Self(seq)
    }

    /// Raw sequence.
    #[must_use]
    pub const fn seq(self) -> u64 {
        self.0
    }

    /// Next sequence; saturates instead of wrapping so a cursor can never
    /// alias an old event.
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1).max(1))
    }
}

/// Validate a subscriber cursor against the retained log window.
///
/// `head_seq` is the last stored sequence (0 when the log is empty);
/// `retained_from_seq` is the oldest sequence still stored (>= 1 when
/// non-empty). Returns the sequence to replay from. Fails closed:
/// zero is [`ProtocolError::BadCursor`]; a cursor behind retention or
/// ahead of `head + 1` is [`ProtocolError::ResyncRequired`] so the caller
/// takes a fresh snapshot instead of replaying a gapped prefix.
pub fn check_cursor(
    cursor: EventCursor,
    head_seq: u64,
    retained_from_seq: u64,
) -> Result<u64, ProtocolError> {
    let seq = cursor.seq();
    if seq == 0 {
        return Err(ProtocolError::BadCursor);
    }
    if head_seq == 0 {
        return if seq == 1 {
            Ok(1)
        } else {
            Err(ProtocolError::ResyncRequired)
        };
    }
    if seq < retained_from_seq.max(1) {
        return Err(ProtocolError::ResyncRequired);
    }
    if seq > head_seq.saturating_add(1) {
        return Err(ProtocolError::ResyncRequired);
    }
    Ok(seq)
}

/// Replay durability of one event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Durability {
    /// Rebuilds durable state on replay (session lifecycle, message
    /// history, completed tool outcomes, permission resolutions).
    Durable,
    /// Never replays as durable state (streaming deltas, progress,
    /// presence, transient prompts, keepalives, and anything unknown).
    Ephemeral,
}

impl Durability {
    /// True for [`Durability::Durable`].
    #[must_use]
    pub const fn is_durable(self) -> bool {
        matches!(self, Self::Durable)
    }
}

/// Classify an event type for replay. Total function over caller bytes:
/// the eight durable lifecycle/history types replay, everything else —
/// including unknown future types — is ephemeral, so a projector rebuild
/// can never invent durable messages from deltas or unrecognized input.
#[must_use]
pub fn classify(event_type: &str) -> Durability {
    match event_type {
        "session.created" | "session.renamed" | "session.archived" | "message.appended"
        | "message.compacted" | "tool.completed" | "permission.granted" | "permission.denied" => {
            Durability::Durable
        }
        _ => Durability::Ephemeral,
    }
}

/// Validate one event frame header: well-formed type token plus payload
/// within [`MAX_FRAME_BYTES`], returning its replay durability. The byte
/// cap is conservative (payload alone, before envelope overhead) and fails
/// closed; unknown types pass as [`Durability::Ephemeral`].
pub fn check_event_frame(
    event_type: &str,
    payload_len: usize,
) -> Result<Durability, ProtocolError> {
    if !valid_token(event_type, MAX_EVENT_TYPE_CHARS, false) {
        return Err(ProtocolError::MalformedFrame);
    }
    if payload_len > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    Ok(classify(event_type))
}

/// Versioned request envelope shared by SDK, proxy, headless and sync
/// callers. `version` must equal [`WIRE_VERSION`]; IDs and keys obey the
/// token rules above. `op_digest` is optional and labels retried payloads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope<T> {
    pub version: u16,
    pub request_id: RequestId,
    pub device_id: DeviceId,
    pub session_id: SessionId,
    pub idempotency_key: IdempotencyKey,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op_digest: Option<OpDigest>,
    pub payload: T,
}

impl<T> Envelope<T> {
    /// Wrap a payload, stamping [`WIRE_VERSION`]. Inputs are already-validated
    /// IDs/keys, so this cannot fail.
    #[must_use]
    pub fn new(
        request_id: RequestId,
        device_id: DeviceId,
        session_id: SessionId,
        idempotency_key: IdempotencyKey,
        payload: T,
    ) -> Self {
        Self {
            version: WIRE_VERSION,
            request_id,
            device_id,
            session_id,
            idempotency_key,
            op_digest: None,
            payload,
        }
    }

    /// Attach an op digest, returning the envelope.
    #[must_use]
    pub fn with_op_digest(mut self, op: OpDigest) -> Self {
        self.op_digest = Some(op);
        self
    }

    /// Re-validate a decoded envelope: version first (old/new peers fail
    /// closed here), then every ID, key and digest.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        check_version(self.version)?;
        self.request_id.validate()?;
        self.device_id.validate()?;
        self.session_id.validate()?;
        self.idempotency_key.validate()?;
        if let Some(op) = &self.op_digest {
            op.validate()?;
        }
        Ok(())
    }
}

/// Accept exactly [`WIRE_VERSION`]; anything else fails closed so schema
/// changes are compatibility-tested instead of silently coerced.
pub fn check_version(version: u16) -> Result<(), ProtocolError> {
    if version == WIRE_VERSION {
        Ok(())
    } else {
        Err(ProtocolError::UnsupportedVersion { actual: version })
    }
}

/// True iff `version` is spoken here (exactly [`WIRE_VERSION`]).
#[must_use]
pub const fn version_supported(version: u16) -> bool {
    version == WIRE_VERSION
}

/// Encode one envelope to JSON bytes. Rejects payloads whose encoding
/// exceeds [`MAX_FRAME_BYTES`]; serialization failure maps to
/// [`ProtocolError::MalformedFrame`] (fail closed, never a partial frame).
pub fn encode<T: Serialize>(envelope: &Envelope<T>) -> Result<Vec<u8>, ProtocolError> {
    let bytes = serde_json::to_vec(envelope).map_err(|_| ProtocolError::MalformedFrame)?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    Ok(bytes)
}

/// Decode one envelope from caller bytes: byte cap first (parse never
/// attempted past it), then UTF-8, then JSON, then [`Envelope::validate`].
/// Every malformed input fails closed; error variants never echo input.
pub fn decode<T>(bytes: &[u8]) -> Result<Envelope<T>, ProtocolError>
where
    T: for<'de> Deserialize<'de>,
{
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    std::str::from_utf8(bytes).map_err(|_| ProtocolError::MalformedFrame)?;
    let envelope: Envelope<T> =
        serde_json::from_slice(bytes).map_err(|_| ProtocolError::MalformedFrame)?;
    envelope.validate()?;
    Ok(envelope)
}

/// Caller-owned bounded FIFO of encoded frames. Rejections leave the queue
/// byte-identical; [`drain`](Self::drain) removes all frames in FIFO order.
#[derive(Debug, Default)]
pub struct FrameQueue {
    items: VecDeque<Vec<u8>>,
    bytes: usize,
}

impl FrameQueue {
    /// Empty queue; allocates only the empty deque.
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: VecDeque::new(),
            bytes: 0,
        }
    }

    /// Buffer one encoded frame. Single frames past [`MAX_FRAME_BYTES`]
    /// are [`ProtocolError::FrameTooLarge`]; a full item count or byte
    /// total is [`ProtocolError::QueueFull`].
    pub fn push(&mut self, frame: Vec<u8>) -> Result<(), ProtocolError> {
        if frame.len() > MAX_FRAME_BYTES {
            return Err(ProtocolError::FrameTooLarge);
        }
        if self.items.len() >= MAX_QUEUE_ITEMS
            || self.bytes.saturating_add(frame.len()) > MAX_QUEUE_BYTES
        {
            return Err(ProtocolError::QueueFull);
        }
        self.bytes = self.bytes.saturating_add(frame.len());
        self.items.push_back(frame);
        Ok(())
    }

    /// Remove all frames in FIFO order; the queue is empty afterwards.
    pub fn drain(&mut self) -> Vec<Vec<u8>> {
        self.bytes = 0;
        self.items.drain(..).collect()
    }

    /// Queued frame count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True when no frames are queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Live queued byte total.
    #[must_use]
    pub fn queued_bytes(&self) -> usize {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    const DIGEST: &str = "9f86d081884c7d659a2feaa0c55ad015";
    const DIGEST2: &str = "d4735e3a265e16eee03f59718b9b5d03";

    fn ids() -> (RequestId, DeviceId, SessionId) {
        (
            RequestId::parse("req.01").unwrap(),
            DeviceId::parse("dev-01").unwrap(),
            SessionId::parse("sess_01").unwrap(),
        )
    }

    fn envelope(payload: Value) -> Envelope<Value> {
        let (r, d, s) = ids();
        let key = IdempotencyKey::parse("key-01").unwrap();
        Envelope::new(r, d, s, key, payload)
    }

    #[test]
    fn envelope_roundtrip_ok() {
        let env = envelope(json!({"op": "attach", "dir": "/tmp/w"}));
        let bytes = encode(&env).unwrap();
        let back: Envelope<Value> = decode(&bytes).unwrap();
        assert_eq!(back, env);
        assert_eq!(back.version, WIRE_VERSION);
    }

    #[test]
    fn wrong_version_fails_closed() {
        let env = envelope(json!({"op": "attach"}));
        let mut value = serde_json::to_value(&env).unwrap();
        value["version"] = json!(WIRE_VERSION + 1);
        let bytes = serde_json::to_vec(&value).unwrap();
        let err = decode::<Value>(&bytes).unwrap_err();
        assert_eq!(
            err,
            ProtocolError::UnsupportedVersion {
                actual: WIRE_VERSION + 1
            }
        );
        assert!(!version_supported(WIRE_VERSION + 1));
        assert!(version_supported(WIRE_VERSION));
    }

    #[test]
    fn oversized_frame_fails_closed() {
        let big = "x".repeat(MAX_FRAME_BYTES + 1);
        let env = envelope(json!({"blob": big}));
        assert_eq!(encode(&env).unwrap_err(), ProtocolError::FrameTooLarge);
        let raw = vec![b'{'; MAX_FRAME_BYTES + 1];
        assert_eq!(
            decode::<Value>(&raw).unwrap_err(),
            ProtocolError::FrameTooLarge
        );
        let mut queue = FrameQueue::new();
        assert_eq!(
            queue.push(vec![0u8; MAX_FRAME_BYTES + 1]).unwrap_err(),
            ProtocolError::FrameTooLarge
        );
        assert!(queue.is_empty());
    }

    #[test]
    fn malformed_ids_fail_closed() {
        let bad = [
            "",
            " leading",
            "/etc/passwd",
            "../req",
            "req id",
            "-lead",
            ".lead",
            "null\u{0}byte",
            &"r".repeat(MAX_ID_CHARS + 1),
        ];
        for value in bad {
            assert!(RequestId::parse(value).is_err(), "request {value:?}");
            assert!(DeviceId::parse(value).is_err(), "device {value:?}");
            assert!(SessionId::parse(value).is_err(), "session {value:?}");
        }
        assert!(RequestId::parse("ok.req-01_2").is_ok());
    }

    #[test]
    fn malformed_wire_bytes_fail_closed() {
        assert_eq!(
            decode::<Value>(&[0xff, 0xfe, 0x00]).unwrap_err(),
            ProtocolError::MalformedFrame
        );
        assert_eq!(
            decode::<Value>(b"{truncated").unwrap_err(),
            ProtocolError::MalformedFrame
        );
        assert_eq!(
            decode::<Value>(b"[1,2,3]").unwrap_err(),
            ProtocolError::MalformedFrame
        );
        let env = envelope(json!({"op": "x"}));
        let mut value = serde_json::to_value(&env).unwrap();
        value["request_id"] = json!("../escape");
        let bytes = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            decode::<Value>(&bytes).unwrap_err(),
            ProtocolError::MalformedId
        );
    }

    #[test]
    fn digests_validate_hex_bounds() {
        assert!(OpDigest::parse(DIGEST).is_ok());
        assert!(OpDigest::parse(&DIGEST.to_ascii_uppercase()).is_ok());
        for bad in ["", "xyz", &"a".repeat(31), &"g".repeat(32), &"a".repeat(129)] {
            assert!(OpDigest::parse(bad).is_err(), "digest {bad:?}");
        }
    }

    #[test]
    fn idempotency_key_deterministic() {
        let (r, _, _) = ids();
        let a = OpDigest::parse(DIGEST).unwrap();
        let b = OpDigest::parse(DIGEST2).unwrap();
        let k1 = idempotency_key(&r, &a).unwrap();
        let k2 = idempotency_key(&r, &a).unwrap();
        let k3 = idempotency_key(&r, &b).unwrap();
        assert_eq!(k1, k2);
        assert_ne!(k1, k3);
        assert_eq!(k1.as_str(), format!("{}:{DIGEST}", r.as_str()));
        assert!(IdempotencyKey::parse(k1.as_str()).is_ok());
        assert_eq!(
            IdempotencyKey::parse("").unwrap_err(),
            ProtocolError::MalformedKey
        );
        assert_eq!(
            IdempotencyKey::parse("../escape").unwrap_err(),
            ProtocolError::MalformedKey
        );
    }

    #[test]
    fn stale_cursor_requires_resync() {
        assert_eq!(
            check_cursor(EventCursor::new(0), 10, 1).unwrap_err(),
            ProtocolError::BadCursor
        );
        assert_eq!(
            check_cursor(EventCursor::new(3), 10, 5).unwrap_err(),
            ProtocolError::ResyncRequired
        );
        assert_eq!(check_cursor(EventCursor::new(5), 10, 5), Ok(5));
        assert_eq!(check_cursor(EventCursor::new(10), 10, 5), Ok(10));
        assert_eq!(check_cursor(EventCursor::new(11), 10, 5), Ok(11));
    }

    #[test]
    fn future_cursor_requires_resync() {
        assert_eq!(
            check_cursor(EventCursor::new(12), 10, 1).unwrap_err(),
            ProtocolError::ResyncRequired
        );
        assert_eq!(
            check_cursor(EventCursor::new(u64::MAX), 10, 1).unwrap_err(),
            ProtocolError::ResyncRequired
        );
    }

    #[test]
    fn empty_log_cursor() {
        assert_eq!(check_cursor(EventCursor::new(1), 0, 0), Ok(1));
        assert_eq!(
            check_cursor(EventCursor::new(2), 0, 0).unwrap_err(),
            ProtocolError::ResyncRequired
        );
        assert_eq!(
            check_cursor(EventCursor::new(0), 0, 0).unwrap_err(),
            ProtocolError::BadCursor
        );
    }

    #[test]
    fn cursor_next_never_aliases() {
        assert_eq!(EventCursor::new(7).next(), EventCursor::new(8));
        assert_eq!(EventCursor::new(u64::MAX).next().seq(), u64::MAX);
        assert_eq!(EventCursor::new(0).next().seq(), 1);
    }

    #[test]
    fn ephemeral_deltas_never_durable() {
        for durable in [
            "session.created",
            "session.renamed",
            "session.archived",
            "message.appended",
            "message.compacted",
            "tool.completed",
            "permission.granted",
            "permission.denied",
        ] {
            assert_eq!(classify(durable), Durability::Durable, "{durable}");
            assert!(classify(durable).is_durable());
        }
        for ephemeral in [
            "message.delta",
            "assistant.delta",
            "assistant_delta",
            "reasoning.delta",
            "reasoning_summary_delta",
            "tool.invoked",
            "permission.requested",
            "presence.heartbeat",
            "typing.started",
            "progress.update",
            "ping",
            "keepalive",
            "future.unknown.v9",
            "",
        ] {
            assert_eq!(classify(ephemeral), Durability::Ephemeral, "{ephemeral}");
            assert!(!classify(ephemeral).is_durable());
        }
    }

    #[test]
    fn event_frame_header_checks() {
        assert_eq!(
            check_event_frame("message.appended", 10),
            Ok(Durability::Durable)
        );
        assert_eq!(
            check_event_frame("assistant.delta", 10),
            Ok(Durability::Ephemeral)
        );
        assert_eq!(
            check_event_frame("", 10).unwrap_err(),
            ProtocolError::MalformedFrame
        );
        assert_eq!(
            check_event_frame("../escape", 10).unwrap_err(),
            ProtocolError::MalformedFrame
        );
        assert_eq!(
            check_event_frame("message.appended", MAX_FRAME_BYTES + 1).unwrap_err(),
            ProtocolError::FrameTooLarge
        );
    }

    #[test]
    fn queue_full_fails_closed() {
        let mut queue = FrameQueue::new();
        for _ in 0..MAX_QUEUE_ITEMS {
            queue.push(vec![0u8; 1]).unwrap();
        }
        let before = (queue.len(), queue.queued_bytes());
        assert_eq!(queue.push(vec![0u8; 1]).unwrap_err(), ProtocolError::QueueFull);
        assert_eq!((queue.len(), queue.queued_bytes()), before);
        let frames = queue.drain();
        assert_eq!(frames.len(), MAX_QUEUE_ITEMS);
        assert!(queue.is_empty());
        assert_eq!(queue.queued_bytes(), 0);
    }

    #[test]
    fn queue_byte_cap_fails_closed() {
        let mut queue = FrameQueue::new();
        let frames = MAX_QUEUE_BYTES / MAX_FRAME_BYTES;
        for _ in 0..frames {
            queue.push(vec![0u8; MAX_FRAME_BYTES]).unwrap();
        }
        let before = (queue.len(), queue.queued_bytes());
        assert_eq!(
            queue.push(vec![0u8; 1]).unwrap_err(),
            ProtocolError::QueueFull
        );
        assert_eq!((queue.len(), queue.queued_bytes()), before);
    }

    #[test]
    fn errors_carry_no_input() {
        let err: String = format!("{}", ProtocolError::MalformedId);
        assert!(!err.contains("escape"));
        let debug = format!("{:?}", ProtocolError::MalformedFrame);
        assert!(!debug.contains('{'));
    }
}
