#![forbid(unsafe_code)]
//! Assistant-completion footer renderer for the native chat view.
//!
//! Mirrors pinned upstream `packages/tui/src/routes/session/index.tsx:2055-2071`
//! (`AssistantFooter`) composition exactly, span by span:
//! - agent span: `Locale.titlecase(agent)` (upstream `locale.ts:3-5`:
//!   `\b\w` uppercased), present only when the caller actually knows the
//!   agent. The native protocol never supplies one today, so the span is
//!   omitted — never inferred from `MessageRecord.created_at` or the
//!   currently selected UI model.
//! - model span: ` · {model}` only at `terminal.width >= 28`.
//! - duration span: ` · {Locale.duration(ms)}` only when a real turn duration
//!   exists and width is `< 28 || >= 36`. The native protocol emits no
//!   streamed/started/completed boundaries (`crates/server/src/lib.rs:2706-2715`),
//!   so no duration ever exists here: the span is an explicit gap, not a
//!   fixture/wall-clock guess.
//! - throughput span: ` · {x.toFixed(1)} tok/s` only when upstream
//!   `config.data.session.tps` is on AND a real throughput exists. No such
//!   native config key exists (verified absent from `crates/`), and without
//!   streamed timestamps no throughput is computable: explicit gap.
//! - interrupted span: ` · interrupted` only for a real native cancellation
//!   fact (`AssistantFacts::interrupted`, sourced from the TYPED transport
//!   error code `"cancelled"` — never message text) — the native equivalent of
//!   upstream's `"Step interrupted"` condition.
//!
//! Error notice (upstream `index.tsx:2035-2047`): `Error: {message}` in the
//! error fg, rendered only for non-interruption errors, followed by the
//! metadata line with upstream's `marginTop` gap row (one blank row).
//!
//! Rendering honesty: the raw token count is NEVER surfaced — upstream
//! `AssistantFooter` shows throughput only, never a raw count. Missing facts
//! render nothing: no placeholder, no empty agent, no zero duration.
//!
//! Row content is emitted WITHOUT caller decoration; the typed consumer
//! (`native_view_chat::typed_message_rows_with_prefix`) applies the same
//! message prefix/inset as ordinary prose rows (upstream's footer `paddingLeft`
//! and `TextPart` inset are the same three columns).

use crate::native_assistant_facts::AssistantFacts;
use crate::native_view_chat::{ChatMessage, ChatRole, ChatRowClass};

/// Untrusted backend text bound for one footer label (error message, agent,
/// model): a bounded UTF-8 prefix rendered display-only, matching the chat
/// view's label policy.
pub const MAX_FOOTER_LABEL_BYTES: usize = 256;
/// Width threshold mirrored from upstream (`index.tsx:2057`): the model span
/// renders only at `terminal.width >= 28`.
pub const WIDTH_MODEL_MIN: usize = 28;
/// Width band mirrored from upstream (`index.tsx:2060`): the duration span
/// renders only at `width < 28 || width >= 36` — i.e. it is hidden in the
/// `28..=35` band. No duration fact exists in the native protocol today, so
/// this band is currently unreachable; retained for producer wiring parity.
pub const WIDTH_DURATION_GAP_LOW: usize = 28;
pub const WIDTH_DURATION_GAP_HIGH: usize = 36;
/// Exact interruption suffix text (upstream `index.tsx:2065`: ` · interrupted`).
pub const INTERRUPTED_SPAN: &str = " · interrupted";

/// One rendered footer row: content (no caller decoration) and its paint class.
pub type FooterRow = (String, ChatRowClass);

/// Rows for one assistant-completion footer; `None` means no real fact was
/// available to render.
pub struct FooterRows {
    pub rows: Vec<FooterRow>,
}

/// Render the assistant-completion footer for one assistant message.
///
/// `width` is the terminal width used for the upstream width gates. Returns
/// `None` when the message is not an assistant message, carries no facts, or
/// has no renderable fact at all — missing facts render nothing.
#[must_use]
pub fn render_assistant_footer(message: &ChatMessage, width: usize) -> Option<FooterRows> {
    if message.role != ChatRole::Assistant {
        return None;
    }
    let facts: &AssistantFacts = message.assistant_facts.as_ref()?;
    let interrupted = facts.interrupted;
    // Upstream `index.tsx:2035`: the error notice renders only when an error
    // exists AND the turn was not interrupted (and has no retry — retry rows
    // are composer-owned and modelled elsewhere).
    let error_notice = facts
        .error_message
        .as_deref()
        .map(bounded_label)
        .filter(|message| !message.is_empty())
        .filter(|_| !interrupted);

    let agent_span = message
        .agent
        .as_deref()
        .map(bounded_label)
        .filter(|agent| !agent.is_empty())
        .map(|agent| titlecase(&agent));
    let model_span = (width >= WIDTH_MODEL_MIN)
        .then(|| {
            message
                .model
                .as_deref()
                .map(bounded_label)
                .filter(|model| !model.is_empty())
        })
        .flatten()
        .map(|model| format!(" · {model}"));
    // Duration/throughput spans: explicit gaps (module docs). They become
    // spans only from real streamed/started timestamps plus the native tps
    // config — none of which the wire provides today.
    let interrupted_span = interrupted.then(|| INTERRUPTED_SPAN.to_owned());

    // Upstream composes the metadata line by concatenating the spans: the
    // agent span is unprefixed and every optional span starts with ` · `.
    let mut metadata = String::new();
    metadata.push_str(agent_span.as_deref().unwrap_or(""));
    for span in model_span.iter().chain(interrupted_span.iter()) {
        metadata.push_str(span);
    }

    let mut rows: Vec<FooterRow> = Vec::new();
    if let Some(message) = error_notice {
        rows.push((format!("Error: {message}"), ChatRowClass::FooterError));
    }
    if metadata.is_empty() {
        // No metadata fact to render. Upstream's `marginTop` gap belongs to
        // the metadata box; with no metadata row there is no gap.
        return (!rows.is_empty()).then_some(FooterRows { rows });
    }
    if !rows.is_empty() {
        // Upstream `index.tsx:2051`: the metadata box carries
        // `marginTop={error && !interrupted ? 1 : 0}` — one blank gap row
        // between the error notice and the metadata line.
        rows.push((String::new(), ChatRowClass::Blank));
    }
    rows.push((metadata, ChatRowClass::AssistantFooter));
    Some(FooterRows { rows })
}

/// `Locale.titlecase` (`packages/tui/src/util/locale.ts:3-5`): uppercase the
/// first `\w` (`[A-Za-z0-9_]`) character at every word boundary. Non-ASCII
/// characters are left unchanged (JS `\w` is ASCII-only).
fn titlecase(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut at_boundary = true;
    for ch in text.chars() {
        let is_word = ch.is_ascii_alphanumeric() || ch == '_';
        if is_word && at_boundary {
            output.push(ch.to_ascii_uppercase());
        } else {
            output.push(ch);
        }
        at_boundary = !is_word;
    }
    output
}

/// Bounded inline label: bounded UTF-8 prefix, ANSI stripped, every remaining
/// control scalar becomes a space.
fn bounded_label(text: &str) -> String {
    strip_ansi(clip_utf8_prefix_bytes(text, MAX_FOOTER_LABEL_BYTES))
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect()
}

fn clip_utf8_prefix_bytes(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Minimal ANSI stripper matching the native chat policy (CSI/SGR, OSC-8,
/// lone ESC/C1 introducers removed).
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            match chars.peek() {
                Some('[') => {
                    chars.next();
                    for c in chars.by_ref() {
                        if c.is_alphabetic() {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    for c in chars.by_ref() {
                        if matches!(c, '\u{1b}' | '\u{07}') {
                            break;
                        }
                    }
                }
                Some(other) if ('\u{00}'..='\u{1f}').contains(other) => {
                    chars.next();
                }
                _ => {}
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod native_footer_tests {
    use super::*;
    use crate::native_view_chat::ChatPart;

    fn assistant_message(facts: AssistantFacts, agent: Option<&str>, model: Option<&str>) -> ChatMessage {
        ChatMessage {
            role: ChatRole::Assistant,
            parts: vec![ChatPart::Text("body".to_owned())],
            agent: agent.map(str::to_owned),
            model: model.map(str::to_owned),
            assistant_facts: Some(facts),
        }
    }

    fn row_texts(rows: &FooterRows) -> Vec<&str> {
        rows.rows.iter().map(|(text, _)| text.as_str()).collect()
    }

    #[test]
    fn missing_facts_render_nothing() {
        let mut message = assistant_message(AssistantFacts::default(), None, None);
        message.assistant_facts = None;
        assert!(render_assistant_footer(&message, 80).is_none());
    }

    #[test]
    fn non_assistant_role_never_renders() {
        let mut message = assistant_message(AssistantFacts::default(), None, None);
        message.role = ChatRole::User;
        assert!(render_assistant_footer(&message, 80).is_none());
    }

    #[test]
    fn completed_turn_without_agent_model_renders_nothing() {
        // No raw token count, no duration, no throughput: a completed turn
        // with no agent/model fact has no footer surface at all.
        let facts = AssistantFacts::from_message_frame(&serde_json::json!({
            "usage": {"output_tokens": 20, "reasoning_tokens": 5},
            "stop_reason": "completed"
        }));
        let message = assistant_message(facts, None, None);
        assert!(render_assistant_footer(&message, 80).is_none());
    }

    #[test]
    fn interruption_renders_exact_suffix_span() {
        let facts = AssistantFacts::from_error(Some("cancelled".to_owned()), "turn cancelled".to_owned());
        let message = assistant_message(facts, None, None);
        let rows = render_assistant_footer(&message, 80).unwrap();
        // Span-exact upstream composition: the interrupted span is verbatim
        // ` · interrupted`; with no agent span the line is exactly that span.
        assert_eq!(row_texts(&rows), [" · interrupted"]);
        assert_eq!(rows.rows[0].1, ChatRowClass::AssistantFooter);
    }

    #[test]
    fn interruption_with_agent_and_model_composes_span_exact() {
        let facts = AssistantFacts::from_error(Some("cancelled".to_owned()), "turn cancelled".to_owned());
        let message = assistant_message(facts, Some("build"), Some("openai/gpt-x"));
        let rows = render_assistant_footer(&message, 80).unwrap();
        assert_eq!(row_texts(&rows), ["Build · openai/gpt-x · interrupted"]);
    }

    #[test]
    fn model_span_respects_width_gate() {
        let facts = AssistantFacts::from_error(Some("cancelled".to_owned()), "turn cancelled".to_owned());
        let message = assistant_message(facts, Some("build"), Some("openai/gpt-x"));
        // width < 28: model span omitted, agent + interrupted remain.
        let narrow = render_assistant_footer(&message, WIDTH_MODEL_MIN - 1).unwrap();
        assert_eq!(row_texts(&narrow), ["Build · interrupted"]);
        // width >= 28: model span present.
        let wide = render_assistant_footer(&message, WIDTH_MODEL_MIN).unwrap();
        assert_eq!(row_texts(&wide), ["Build · openai/gpt-x · interrupted"]);
    }

    #[test]
    fn generic_error_renders_notice_then_gap_then_metadata() {
        let facts = AssistantFacts::from_error(Some("provider_stream_failed".to_owned()), "provider stream failed".to_owned());
        let message = assistant_message(facts, Some("build"), None);
        let rows = render_assistant_footer(&message, 80).unwrap();
        assert_eq!(
            row_texts(&rows),
            ["Error: provider stream failed", "", "Build"],
            "upstream error row, marginTop gap, metadata line"
        );
        assert_eq!(rows.rows[0].1, ChatRowClass::FooterError);
        assert_eq!(rows.rows[1].1, ChatRowClass::Blank);
        assert_eq!(rows.rows[2].1, ChatRowClass::AssistantFooter);
    }

    #[test]
    fn generic_error_without_metadata_fact_renders_notice_only() {
        // Upstream's metadata box is unconditional, but with no real native
        // agent/model/interrupt fact there is nothing to put in it; the gap
        // belongs to that box, so no gap row is emitted either.
        let facts = AssistantFacts::from_error(Some("provider_stream_failed".to_owned()), "provider stream failed".to_owned());
        let message = assistant_message(facts, None, None);
        let rows = render_assistant_footer(&message, 80).unwrap();
        assert_eq!(row_texts(&rows), ["Error: provider stream failed"]);
        assert_eq!(rows.rows[0].1, ChatRowClass::FooterError);
    }

    #[test]
    fn interruption_suppresses_error_notice() {
        let facts = AssistantFacts::from_error(Some("cancelled".to_owned()), "turn cancelled".to_owned());
        let message = assistant_message(facts, None, None);
        let rows = render_assistant_footer(&message, 80).unwrap();
        assert!(
            !rows.rows.iter().any(|(text, _)| text.starts_with("Error:")),
            "no error notice for interruption"
        );
    }

    #[test]
    fn cancellation_wording_without_typed_code_is_error_notice_not_interrupted() {
        // Message text is display data. A different error path may say
        // "turn cancelled" (tool outputs do, `crates/server/src/lib.rs:2838-2841`);
        // without the typed transport code the footer shows the error notice
        // and never the ` · interrupted` suffix.
        let facts = AssistantFacts::from_error(None, "turn cancelled".to_owned());
        let message = assistant_message(facts, None, None);
        let rows = render_assistant_footer(&message, 80).unwrap();
        assert_eq!(row_texts(&rows), ["Error: turn cancelled"]);
        assert_eq!(rows.rows[0].1, ChatRowClass::FooterError);
    }

    #[test]
    fn agent_is_titlecased_like_upstream_locale() {
        // `Locale.titlecase`: `\b\w` uppercased — "build agent" -> "Build Agent",
        // digits/underscores are word chars, punctuation boundaries reset.
        assert_eq!(titlecase("build agent"), "Build Agent");
        assert_eq!(titlecase("plan2_exec"), "Plan2_exec");
        assert_eq!(titlecase("claude-fable"), "Claude-Fable");
        assert_eq!(titlecase(""), "");
    }

    #[test]
    fn raw_token_counts_and_duration_are_never_rendered() {
        // Usage tokens exist as facts but upstream shows no raw count; no
        // timestamps exist so no duration/throughput span can exist.
        let facts = AssistantFacts::from_message_frame(&serde_json::json!({
            "usage": {"output_tokens": 9999, "reasoning_tokens": 8888},
            "stop_reason": "completed"
        }));
        for (agent, model) in [(Some("build"), Some("openai/gpt-x")), (None, None)] {
            let message = assistant_message(facts.clone(), agent, model);
            for width in [27, 28, 35, 36, 80, 200] {
                if let Some(rows) = render_assistant_footer(&message, width) {
                    for (text, _) in &rows.rows {
                        assert!(!text.contains("tok"), "no raw token count: {text}");
                        assert!(!text.contains("ms"), "no duration: {text}");
                        assert!(!text.contains("/s"), "no throughput: {text}");
                    }
                }
            }
        }
    }

    #[test]
    fn hostile_labels_are_sanitized_and_bounded() {
        let facts = AssistantFacts::from_error(Some("internal_error".to_owned()), format!("evil\u{0000}\u{1b}[31m{}", "X".repeat(20 * 1024)));
        let message = assistant_message(
            facts,
            Some("agent\u{1b}]8;;evil\u{07}name"),
            Some("model\nname"),
        );
        let rows = render_assistant_footer(&message, 80).unwrap();
        for (text, _) in &rows.rows {
            assert!(!text.contains('\u{1b}'), "no ESC leaks: {text:?}");
            assert!(!text.contains('\u{0000}'), "no NUL leaks: {text:?}");
            assert!(!text.contains('\n'), "single-row labels: {text:?}");
            assert!(text.len() <= 16 + 3 * MAX_FOOTER_LABEL_BYTES, "bounded: {text:?}");
        }
    }

    #[test]
    fn partial_usage_facts_stay_truthful() {
        // Only reasoning tokens on the wire: still no rendered surface.
        let facts = AssistantFacts::from_message_frame(&serde_json::json!({
            "usage": {"reasoning_tokens": 7},
            "stop_reason": "incomplete:max_output_tokens"
        }));
        assert_eq!(facts.output_tokens, None);
        assert_eq!(facts.reasoning_tokens, Some(7));
        assert_eq!(facts.stop_reason.as_deref(), Some("incomplete:max_output_tokens"));
        let message = assistant_message(facts, None, None);
        assert!(render_assistant_footer(&message, 80).is_none());
    }

    #[test]
    fn width_band_constants_match_upstream_assistant_footer() {
        // Upstream `index.tsx:2057-2065`: model at width >= 28, duration hidden
        // in the 28..=35 band, exact ` · interrupted` suffix span. These
        // anchors stay pinned even though no duration fact exists on the
        // native wire today.
        assert_eq!(WIDTH_MODEL_MIN, 28);
        assert_eq!(WIDTH_DURATION_GAP_LOW, 28);
        assert_eq!(WIDTH_DURATION_GAP_HIGH, 36);
        assert_eq!(INTERRUPTED_SPAN, " · interrupted");
    }
}
