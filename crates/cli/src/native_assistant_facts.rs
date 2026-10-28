#![forbid(unsafe_code)]
//! Native assistant-turn execution facts: the real subset the RK native turn
//! protocol emits, and nothing else. Producer/consumer seam for the assistant
//! completion footer.
//!
//! Source of truth (inspected at canonical base `4b4021b9cf45061d13e4d0e60610c2c038118d0c`):
//! - `crates/server/src/lib.rs:2706-2715` and `:3001-3006`: the live
//!   `assistant_message` NDJSON frame carries `message` (a `MessageRecord`),
//!   `reasoning_summary`, `usage` and `stop_reason`. It does NOT carry
//!   `agent`, `model`, `started_ms`, `streamed_ms` or `completed_ms`.
//! - `crates/server/src/lib.rs:1975-1989`: `provider_id`/`model_id` exist only
//!   in turn-creation scope; they are not attached to the frame or the
//!   persisted `MessageRecord`, so historical agent/model remain ABSENT
//!   (never inferred from the selected UI model or `MessageRecord.created_at`).
//! - `crates/server/src/agent_loop.rs:35-57`: `TurnStop` (`stop_reason`) has NO
//!   cancellation variant — `stop_reason` never encodes an interruption.
//!
//! Cancellation — typed transport only, never message text:
//! - In-stream error frames are `{"type":"error","code":…,"message":…}`
//!   (`crates/server/src/lib.rs:3183-3189`, `stream_error`).
//! - HTTP failure bodies are `{"code":…,"message":…}`
//!   (`crates/server/src/lib.rs:3458-3470`, `ErrorBody`).
//! - The typed cancellation code is exactly `"cancelled"`:
//!   `lib.rs:2033/2499/2504/2505` (`message:"turn cancelled"`) and
//!   `task_children.rs:664-668/726` (`message:"child cancelled"`, and the
//!   `error.code == "cancelled"` comparison).
//! - Message strings are display data and are deliberately NEVER matched:
//!   tool outputs legitimately contain cancellation wording (e.g.
//!   `lib.rs:2838-2841` `"error: call not executed because turn was
//!   cancelled"`), and upstream's `"Step interrupted"` literal
//!   (`packages/tui/src/routes/session/index.tsx`) never occurs on this wire.
//! - Residual gap: the NON-streaming `/turns` cancel site (`lib.rs:2023`)
//!   emits `code:"bad_gateway"` with `message:"turn cancelled"`; the typed
//!   rule will not classify it as interrupted. Native turns use
//!   `/turns/stream` (whose cancel sites use `code:"cancelled"`), so this is
//!   recorded as a producer inconsistency to normalize — a missed suffix is
//!   preferred over a fabricated one.

use serde_json::Value;

/// The typed wire error code meaning cancellation
/// (`ApiFailure.code` / `stream_error` code `"cancelled"`).
pub const CANCELLED_CODE: &str = "cancelled";

/// Real execution facts attachable to one assistant [`ChatMessage`]
/// (`crate::native_view_chat::ChatMessage`). Every field is either populated
/// from authoritative execution metadata (the live turn stream) or left
/// absent (`None`/`false`) — never backfilled from `MessageRecord.created_at`,
/// wall time or the currently selected UI model.
///
/// Historical `agent`/`model`/timing are NOT modelled here: the native
/// protocol does not emit them (see module docs). They stay explicit gaps
/// until a producer patch adds them to the wire contract; when it does, they
/// extend this struct from that authoritative metadata only.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssistantFacts {
    /// Backend-reported stop reason (`completed`, `incomplete:...`,
    /// `max_steps:N`, `policy_denied:tool`). Real wire fact from the
    /// `assistant_message` frame. Never an interruption indicator
    /// (`TurnStop` has no cancellation variant).
    pub stop_reason: Option<String>,
    /// `usage.output_tokens` from the frame. Real wire fact. NOT rendered as a
    /// raw count (upstream `AssistantFooter` never shows one); retained as an
    /// execution fact for future throughput wiring when real streamed
    /// timestamps exist.
    pub output_tokens: Option<u64>,
    /// `usage.reasoning_tokens` from the frame. Same provenance and rendering
    /// policy as `output_tokens`.
    pub reasoning_tokens: Option<u64>,
    /// True only when the TYPED error transport code was [`CANCELLED_CODE`] —
    /// the native equivalent of upstream's `"Step interrupted"` suffix
    /// condition. Never derived from message text.
    pub interrupted: bool,
    /// Backend error message for a terminal error frame (`None` for a normal
    /// completion). Display data only: drives the upstream `Error: {message}`
    /// notice row for non-interruption errors and is never pattern-matched.
    pub error_message: Option<String>,
}

impl AssistantFacts {
    /// Facts from one `assistant_message` NDJSON frame: real `usage` tokens and
    /// `stop_reason` only. `agent`/`model`/timing are absent on the wire and
    /// remain `None`.
    #[must_use]
    pub fn from_message_frame(frame: &Value) -> Self {
        Self {
            stop_reason: frame
                .get("stop_reason")
                .and_then(Value::as_str)
                .map(str::to_owned),
            output_tokens: frame
                .get("usage")
                .and_then(|usage| usage.get("output_tokens"))
                .and_then(Value::as_u64),
            reasoning_tokens: frame
                .get("usage")
                .and_then(|usage| usage.get("reasoning_tokens"))
                .and_then(Value::as_u64),
            interrupted: false,
            error_message: None,
        }
    }

    /// Facts from one terminal transport error. `code` is the TYPED wire error
    /// code (`error` frame `code` or HTTP `ErrorBody.code`); `message` is its
    /// display text. `interrupted` is `true` only when `code == "cancelled"`.
    /// Message text is never inspected: other error paths legitimately contain
    /// cancellation wording (see module docs).
    #[must_use]
    pub fn from_error(code: Option<String>, message: String) -> Self {
        let interrupted = code.as_deref() == Some(CANCELLED_CODE);
        Self {
            stop_reason: None,
            output_tokens: None,
            reasoning_tokens: None,
            interrupted,
            error_message: Some(message),
        }
    }
}

#[cfg(test)]
mod native_assistant_facts_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn message_frame_extracts_only_real_wire_facts() {
        let frame = json!({
            "type": "assistant_message",
            "message": {
                "id": "m-1", "session_id": "s-1", "role": "assistant",
                "body": {"storage": "inline", "text": "hi"},
                "created_at": "2026-10-19T00:00:00Z"
            },
            "usage": {"input_tokens": 10, "output_tokens": 20, "cached_tokens": 0, "reasoning_tokens": 5},
            "stop_reason": "completed"
        });
        let facts = AssistantFacts::from_message_frame(&frame);
        assert_eq!(facts.output_tokens, Some(20));
        assert_eq!(facts.reasoning_tokens, Some(5));
        assert_eq!(facts.stop_reason.as_deref(), Some("completed"));
        assert!(!facts.interrupted);
        assert!(facts.error_message.is_none());
    }

    #[test]
    fn missing_usage_is_absent_not_zero() {
        let frame = json!({"type": "assistant_message", "stop_reason": "max_steps:8"});
        let facts = AssistantFacts::from_message_frame(&frame);
        assert!(facts.output_tokens.is_none());
        assert!(facts.reasoning_tokens.is_none());
        assert_eq!(facts.stop_reason.as_deref(), Some("max_steps:8"));
    }

    #[test]
    fn missing_stop_reason_stays_absent() {
        let facts = AssistantFacts::from_message_frame(&json!({"type": "assistant_message"}));
        assert!(facts.stop_reason.is_none());
    }

    #[test]
    fn typed_cancel_code_is_the_only_interruption_source() {
        // Typed code `cancelled` (lib.rs:2033/2499/2504/2505,
        // task_children.rs:664-668/726) classifies regardless of message text.
        let facts = AssistantFacts::from_error(Some(CANCELLED_CODE.to_owned()), "turn cancelled".to_owned());
        assert!(facts.interrupted);
        assert_eq!(facts.error_message.as_deref(), Some("turn cancelled"));
        let child = AssistantFacts::from_error(Some(CANCELLED_CODE.to_owned()), "child cancelled".to_owned());
        assert!(child.interrupted);
    }

    #[test]
    fn message_text_never_classifies_interruption() {
        // A different error path may legitimately say "cancelled" (tool output
        // text lib.rs:2838-2841, or the mis-coded non-streaming cancel at
        // lib.rs:2023 `code:"bad_gateway"`). Without the typed code none of
        // these is an interruption.
        for message in [
            "turn cancelled",
            "error: call not executed because turn was cancelled",
            "child cancelled",
            "Step interrupted",
            "",
        ] {
            let facts = AssistantFacts::from_error(None, message.to_owned());
            assert!(!facts.interrupted, "no code, no interruption: {message:?}");
            assert_eq!(facts.error_message.as_deref(), Some(message));
        }
        let mis_coded = AssistantFacts::from_error(Some("bad_gateway".to_owned()), "turn cancelled".to_owned());
        assert!(!mis_coded.interrupted, "non-cancelled typed code is not an interruption");
    }

    #[test]
    fn generic_error_codes_are_not_interruptions() {
        for code in ["internal_error", "bad_gateway", "provider_stream_failed", "context_budget_exceeded"] {
            let facts = AssistantFacts::from_error(Some(code.to_owned()), "provider stream failed".to_owned());
            assert!(!facts.interrupted, "{code} must not interrupt");
        }
    }
}
