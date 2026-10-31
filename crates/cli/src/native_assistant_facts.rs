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
//!   in turn-creation scope; they are not attached to the frame, so the LIVE
//!   frame still carries no agent/model/timing. They ARE persisted for the
//!   message (`message_execution`) and replayed on `/messages` and `/history`
//!   as the additive `execution` object, which is what
//!   [`AssistantFacts::from_history_record`] and
//!   [`AssistantFacts::model_label`] read. Nothing is inferred from the
//!   selected UI model or `MessageRecord.created_at`; timing and agent remain
//!   absent because no producer records them.
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
/// from authoritative execution metadata (the live turn stream, or the stored
/// `execution` object a completed turn wrote) or left absent (`None`/`false`) —
/// never backfilled from `MessageRecord.created_at`, wall time or the
/// currently selected UI model.
///
/// `agent` and timing are NOT modelled here: no producer records them, so they
/// stay explicit gaps until one does; when it does, they extend this struct
/// from that authoritative metadata only.
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

    /// Facts from one stored history record's additive `execution` object.
    ///
    /// `None` when the record carries no stored execution facts, so a message
    /// that predates the producer paints no footer at all. History has no
    /// transport error, so `interrupted` and `error_message` stay absent —
    /// interruption is a live-turn fact, not a stored one.
    #[must_use]
    pub fn from_history_record(record: &Value) -> Option<Self> {
        let execution = record.get("execution")?;
        let usage = execution.get("usage");
        let facts = Self {
            stop_reason: execution
                .get("stop_reason")
                .and_then(Value::as_str)
                .map(str::to_owned),
            output_tokens: usage
                .and_then(|usage| usage.get("output_tokens"))
                .and_then(Value::as_u64),
            reasoning_tokens: usage
                .and_then(|usage| usage.get("reasoning_tokens"))
                .and_then(Value::as_u64),
            interrupted: false,
            error_message: None,
        };
        (facts != Self::default()).then_some(facts)
    }

    /// Footer model text from the stored execution facts, in upstream's form
    /// (`routes/session/index.tsx:2014-2021`): `providerID/id`, or the bare id
    /// when the producer recorded no provider. History projection has no model
    /// catalog, so the catalog-name branch cannot resolve here.
    #[must_use]
    pub fn model_label(record: &Value) -> Option<String> {
        let execution = record.get("execution")?;
        let model = execution
            .get("model_id")
            .and_then(Value::as_str)
            .filter(|model| !model.is_empty())?;
        let provider = execution
            .get("provider_id")
            .and_then(Value::as_str)
            .filter(|provider| !provider.is_empty());
        Some(match provider {
            Some(provider) => format!("{provider}/{model}"),
            None => model.to_owned(),
        })
    }
}

// ---------------------------------------------------------------------------
// Completion footer (MSG-008)
// ---------------------------------------------------------------------------
//
// Upstream renders one footer under each assistant message
// (`packages/tui/src/routes/session/index.tsx:2010-2067`):
//
// - error notice row first, only when `message.error && !interrupted &&
//   !retry`; the notice is the free-limit notice, a usage-limit notice, or the
//   generic `Error: {message}` row (`:2035-2047`).
// - the footer box has `paddingLeft=3` and `marginTop=1` when `retry` or a
//   non-interrupted error is present, else no top margin (`:2050`).
// - slots in fixed order: agent (titlecased, always), ` · model` when width
//   >= 28 (`:2055-2057`), ` · duration` when a duration exists and width < 28
//   or width >= 36 (`:2058-2060`), ` · {tps} tok/s` when the session tps
//   setting is on and throughput exists (`:2061-2063`), ` · interrupted` when
//   the error is exactly `"Step interrupted"` (`:2064-2066`).
//
// Formatting mirrors `packages/tui/src/util/locale.ts:3-5` (titlecase) and
// `:29-49` (duration): ms below 1s, one-decimal seconds below 1 min,
// `m s`, `h m`, `d h` above. Upstream reads model/duration/tps from message
// metadata this wire does not carry; they are only rendered when supplied by
// authoritative live metadata, never inferred (see module docs).

/// Width at and above which the model slot is rendered (`index.tsx:2055`).
pub const FOOTER_MODEL_MIN_WIDTH: u16 = 28;
/// Width below which the duration slot is rendered (`index.tsx:2058`).
pub const FOOTER_DURATION_NARROW_MAX_WIDTH: u16 = 28;
/// Width at and above which the duration slot is rendered (`index.tsx:2058`).
pub const FOOTER_DURATION_WIDE_MIN_WIDTH: u16 = 36;

/// Upstream `Locale.titlecase` (`util/locale.ts:3-5`): capitalise the first
/// letter of every word.
#[must_use]
pub fn titlecase(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut at_word_start = true;
    for ch in input.chars() {
        if ch.is_whitespace() {
            at_word_start = true;
            out.push(ch);
        } else if at_word_start {
            out.extend(ch.to_uppercase());
            at_word_start = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Upstream `Locale.duration` (`util/locale.ts:29-49`).
#[must_use]
pub fn format_duration(ms: u64) -> String {
    if ms < 1_000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else if ms < 3_600_000 {
        format!("{}m {}s", ms / 60_000, (ms % 60_000) / 1_000)
    } else if ms < 86_400_000 {
        format!("{}h {}m", ms / 3_600_000, (ms % 3_600_000) / 60_000)
    } else {
        format!("{}d {}h", ms / 86_400_000, (ms % 86_400_000) / 3_600_000)
    }
}

/// One rendered footer slot; the order in [`FooterRow::slots`] is the
/// upstream order (`index.tsx:2053-2066`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FooterSlotKind {
    Agent,
    Model,
    Duration,
    TokensPerSecond,
    Interrupted,
}

/// A footer slot and its display text (separator-free; the renderer joins
/// slots with ` · `).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FooterSlot {
    pub kind: FooterSlotKind,
    pub text: String,
}

/// The notice row that precedes the footer (`index.tsx:2035-2047`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FooterNotice {
    None,
    /// Generic `Error: {message}` row.
    Error(String),
    /// `provider.free-limit` notice (`:2031`, `:2043-2045`).
    FreeLimit,
    /// Usage-limit notice (`:2032`, `:2046`).
    UsageLimit,
}

/// Everything the completion footer can render. Only authoritative live
/// metadata may populate `agent`/`model`/`duration_ms`/`tokens_per_second`;
/// error, retry, interruption and limit flags come from typed facts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AssistantFooter {
    pub agent: Option<String>,
    pub model: Option<String>,
    pub duration_ms: Option<u64>,
    pub tokens_per_second: Option<f64>,
    /// Session tps setting (`config.data.session.tps`, `index.tsx:2061`).
    pub tps_enabled: bool,
    pub interrupted: bool,
    pub error_message: Option<String>,
    pub error_is_free_limit: bool,
    pub error_is_usage_limit: bool,
    pub retry: bool,
}

/// The notice row, top margin and ordered slots of one footer.
#[derive(Clone, Debug, PartialEq)]
pub struct FooterRow {
    pub notice: FooterNotice,
    pub margin_top: u16,
    pub slots: Vec<FooterSlot>,
}

impl AssistantFooter {
    /// Footer facts from the execution [`AssistantFacts`]. Agent/model/timing
    /// stay absent unless the caller adds authoritative live metadata.
    #[must_use]
    pub fn from_facts(facts: &AssistantFacts) -> Self {
        Self {
            interrupted: facts.interrupted,
            error_message: facts.error_message.clone(),
            ..Self::default()
        }
    }

    /// `props.message.error` presence (`index.tsx:2035`).
    #[must_use]
    pub fn has_error(&self) -> bool {
        self.error_message.is_some() || self.error_is_free_limit || self.error_is_usage_limit
    }

    /// `index.tsx:2035-2047`: no notice for interruptions or retries; typed
    /// limit errors select their notice, everything else is `Error: {msg}`.
    #[must_use]
    pub fn notice(&self) -> FooterNotice {
        if !self.has_error() || self.interrupted || self.retry {
            return FooterNotice::None;
        }
        if self.error_is_free_limit {
            FooterNotice::FreeLimit
        } else if self.error_is_usage_limit {
            FooterNotice::UsageLimit
        } else {
            FooterNotice::Error(self.error_message.clone().unwrap_or_default())
        }
    }

    /// `index.tsx:2050`: one row of top margin above the footer when a retry
    /// is offered or a non-interrupted error notice is shown.
    #[must_use]
    pub fn margin_top(&self) -> u16 {
        if self.retry || (self.has_error() && !self.interrupted) {
            1
        } else {
            0
        }
    }

    /// Build the full footer for a terminal `width`: notice, margin and slots
    /// in upstream order with the width breakpoints at 28 and 36.
    #[must_use]
    pub fn row(&self, width: u16) -> FooterRow {
        let mut slots = Vec::new();
        if let Some(agent) = self.agent.as_deref() {
            slots.push(FooterSlot {
                kind: FooterSlotKind::Agent,
                text: titlecase(agent),
            });
        }
        if width >= FOOTER_MODEL_MIN_WIDTH {
            if let Some(model) = self.model.as_deref() {
                slots.push(FooterSlot {
                    kind: FooterSlotKind::Model,
                    text: model.to_owned(),
                });
            }
        }
        if let Some(ms) = self.duration_ms {
            if width < FOOTER_DURATION_NARROW_MAX_WIDTH || width >= FOOTER_DURATION_WIDE_MIN_WIDTH {
                slots.push(FooterSlot {
                    kind: FooterSlotKind::Duration,
                    text: format_duration(ms),
                });
            }
        }
        if self.tps_enabled {
            if let Some(tps) = self.tokens_per_second {
                slots.push(FooterSlot {
                    kind: FooterSlotKind::TokensPerSecond,
                    text: format!("{tps:.1} tok/s"),
                });
            }
        }
        if self.interrupted {
            slots.push(FooterSlot {
                kind: FooterSlotKind::Interrupted,
                text: "interrupted".to_owned(),
            });
        }
        FooterRow {
            notice: self.notice(),
            margin_top: self.margin_top(),
            slots,
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

    // MSG-008: completion footer slots, width breakpoints and notice rows
    // (`packages/tui/src/routes/session/index.tsx:2035-2066`;
    // `packages/tui/src/util/locale.ts:3-5,29-49`).

    fn full_footer() -> AssistantFooter {
        AssistantFooter {
            agent: Some("build".to_owned()),
            model: Some("anthropic/claude-sonnet".to_owned()),
            duration_ms: Some(1_500),
            tokens_per_second: Some(20.0),
            tps_enabled: true,
            ..AssistantFooter::default()
        }
    }

    fn kinds(row: &FooterRow) -> Vec<FooterSlotKind> {
        row.slots.iter().map(|slot| slot.kind).collect()
    }

    #[test]
    fn msg008_footer_slots_follow_terminal_width_breakpoints() {
        use FooterSlotKind::*;
        // 27: model hidden (<28), duration shown (<28).
        let narrow = full_footer().row(27);
        assert_eq!(kinds(&narrow), vec![Agent, Duration, TokensPerSecond]);
        assert_eq!(narrow.slots[0].text, "Build");
        assert_eq!(narrow.slots[1].text, "1.5s");
        assert_eq!(narrow.slots[2].text, "20.0 tok/s");
        // 28 and 35: model shown, duration hidden.
        for width in [28, 35] {
            assert_eq!(
                kinds(&full_footer().row(width)),
                vec![Agent, Model, TokensPerSecond],
                "width {width}"
            );
        }
        // 36: model and duration both shown.
        assert_eq!(
            kinds(&full_footer().row(36)),
            vec![Agent, Model, Duration, TokensPerSecond]
        );
        assert_eq!(full_footer().row(36).slots[1].text, "anthropic/claude-sonnet");
    }

    #[test]
    fn msg008_footer_notice_rows_and_spacing_are_typed() {
        let mut error = full_footer();
        error.error_message = Some("provider stream failed".to_owned());
        let row = error.row(40);
        assert_eq!(row.notice, FooterNotice::Error("provider stream failed".to_owned()));
        assert_eq!(row.margin_top, 1);

        // Interrupted: no notice, no margin, ordered suffix last.
        let mut interrupted = AssistantFooter::from_facts(&AssistantFacts::from_error(
            Some(CANCELLED_CODE.to_owned()),
            "Step interrupted".to_owned(),
        ));
        interrupted.agent = Some("build".to_owned());
        interrupted.model = Some("m".to_owned());
        let row = interrupted.row(40);
        assert_eq!(row.notice, FooterNotice::None);
        assert_eq!(row.margin_top, 0);
        assert_eq!(kinds(&row), vec![FooterSlotKind::Agent, FooterSlotKind::Model, FooterSlotKind::Interrupted]);
        assert_eq!(row.slots[2].text, "interrupted");

        // Retry suppresses the notice but keeps the one-row margin.
        let mut retry = error.clone();
        retry.retry = true;
        let row = retry.row(40);
        assert_eq!(row.notice, FooterNotice::None);
        assert_eq!(row.margin_top, 1);

        // Typed limit errors pick their own notice rows.
        let mut free = error.clone();
        free.error_is_free_limit = true;
        assert_eq!(free.row(40).notice, FooterNotice::FreeLimit);
        let mut usage = error;
        usage.error_is_usage_limit = true;
        assert_eq!(usage.row(40).notice, FooterNotice::UsageLimit);

        // No error: no notice, no margin.
        let row = full_footer().row(40);
        assert_eq!(row.notice, FooterNotice::None);
        assert_eq!(row.margin_top, 0);
    }

    #[test]
    fn msg008_footer_omits_slots_without_authoritative_metadata() {
        // Historical messages lack agent/model/timing: nothing is fabricated.
        let bare = AssistantFooter::from_facts(&AssistantFacts::from_message_frame(&json!({
            "type": "assistant_message",
            "usage": {"output_tokens": 20},
            "stop_reason": "completed"
        })));
        let row = bare.row(40);
        assert!(row.slots.is_empty());
        assert_eq!(row.notice, FooterNotice::None);

        // tps requires both the session setting and a real throughput value.
        let mut no_tps = full_footer();
        no_tps.tps_enabled = false;
        assert!(!kinds(&no_tps.row(40)).contains(&FooterSlotKind::TokensPerSecond));
        let mut no_value = full_footer();
        no_value.tokens_per_second = None;
        assert!(!kinds(&no_value.row(40)).contains(&FooterSlotKind::TokensPerSecond));

        // Duration and model are independent of each other.
        let mut no_duration = full_footer();
        no_duration.duration_ms = None;
        assert_eq!(kinds(&no_duration.row(27)), vec![FooterSlotKind::Agent, FooterSlotKind::TokensPerSecond]);
        let mut no_model = full_footer();
        no_model.model = None;
        assert_eq!(kinds(&no_model.row(40)), vec![FooterSlotKind::Agent, FooterSlotKind::Duration, FooterSlotKind::TokensPerSecond]);
    }

    #[test]
    fn msg008_duration_and_titlecase_match_upstream_locale() {
        // util/locale.ts:29-49.
        assert_eq!(format_duration(0), "0ms");
        assert_eq!(format_duration(999), "999ms");
        assert_eq!(format_duration(1_000), "1.0s");
        assert_eq!(format_duration(1_500), "1.5s");
        assert_eq!(format_duration(59_999), "60.0s");
        assert_eq!(format_duration(60_000), "1m 0s");
        assert_eq!(format_duration(65_000), "1m 5s");
        assert_eq!(format_duration(3_600_000), "1h 0m");
        assert_eq!(format_duration(90_000_000), "1d 1h");
        // util/locale.ts:3-5.
        assert_eq!(titlecase("build"), "Build");
        assert_eq!(titlecase("plan mode"), "Plan Mode");
        assert_eq!(titlecase(""), "");
    }
}
