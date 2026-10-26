#![forbid(unsafe_code)]
//! JSON → typed chat projection for the daemon's real message records.
//!
//! Pure and IO-free: `serde_json::Value` in, [`crate::native_view_chat`]
//! types out. No clock, no storage, no network, no credential access; the
//! caller owns every fetch.
//!
//! Source of truth (inspected, not guessed):
//! - `crates/contracts/src/lib.rs:178-213`: `MessageRole { System, User,
//!   Assistant, Tool }` (`#[serde(rename_all = "snake_case")]`) and
//!   `MessageRecord { id, session_id, role, body: PayloadRef, created_at }`
//!   where `PayloadRef` is `#[serde(tag = "storage", rename_all =
//!   "snake_case")]` with `Inline { text }` / `Blob { hash, bytes }`.
//! - `crates/server/src/lib.rs:739-750`: `GET /api/sessions/{id}/messages`
//!   serializes those records directly.
//! - `crates/storage/src/lib.rs:85-99`: typed tool history is
//!   `ToolHistoryItem { round_id, pair_index, kind, call_id, name, payload,
//!   message_id }` with `kind` `"call"`/`"output"` (`server/src/lib.rs:2063-2073`),
//!   carried as the caller's real typed tool facts.
//! - Reasoning summaries are real persisted parts: `MAX_REASONING_SUMMARY_BYTES`
//!   (`crates/contracts/src/lib.rs:24`, 8 KiB), written to `message_activity`
//!   (`crates/storage/src/lib.rs:859-861`) and as
//!   `MESSAGE_PART_REASONING_SUMMARY = 1` (`crates/storage/src/writer_v2.rs:17`).
//! - Live tool events (`crates/cli/src/native_turn.rs:150-151`) are
//!   `{"type":"tool_call","name":..}` / `{"type":"tool_output","name":..,
//!   "output":..}`; a caller may attach those exact objects to a message.
//! - `crates/cli/src/tui_entry.rs` fetches a bounded startup page (`limit=200`)
//!   and passes that page to this projection. The projection preserves that
//!   bounded page; it does not claim to provide whole-conversation pagination.
//!
//! Honesty rules: the role comes ONLY from the explicit `role` enum string;
//! an unknown/missing role is `None` — never a fabricated `System`. Tool
//! results use the backend's own `is_error` flag only (an `"error: ..."`
//! output prefix is text and never sets it). A blob body has no inline
//! payload, so it contributes no fabricated text: a record with no displayable
//! part projects to `None` instead of a placeholder.
//!
//! Bounds: every retained string is a char-boundary-safe ≤32 KiB prefix, and
//! one projected message retains at most 32 KiB across its parts.

use serde_json::Value;

use crate::native_view_chat::{ChatMessage, ChatPart, ChatRole};

/// Max bytes retained per message part (matches the chat view's per-row input
/// bound `native_view_chat::MAX_INPUT_ROW_BYTES`).
pub const MAX_PART_TEXT_BYTES: usize = 32 * 1024;
/// Max bytes retained across all parts of one projected message.
pub const MAX_MESSAGE_BYTES: usize = 32 * 1024;
/// Max projected messages returned by [`project_history`]. This matches the
/// current bounded startup fetch (`limit=200`); it is not whole-conversation
/// pagination. Older history requires the paged loader once startup wires it.
pub const MAX_HISTORY_MESSAGES: usize = 200;
/// Max raw entries examined from the tail of the bounded API response.
pub const MAX_HISTORY_SCAN: usize = 200;

/// Max bytes retained for the non-part metadata fields (`agent`, `model`).
pub const MAX_METADATA_BYTES: usize = 256;

/// Char-boundary-safe prefix of at most `max` bytes.
///
/// `max` is a byte ceiling on a possibly multi-byte scalar sequence; the
/// returned prefix always ends on a `char` boundary.
fn bounded(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

/// Explicit role enum string → view role. Unknown/missing values return
/// `None`: no text and no default role ever invents one.
fn role_of(message: &Value) -> Option<ChatRole> {
    match message.get("role").and_then(Value::as_str)? {
        "user" => Some(ChatRole::User),
        "assistant" => Some(ChatRole::Assistant),
        "tool" => Some(ChatRole::Tool),
        "system" => Some(ChatRole::System),
        _ => None,
    }
}

/// Char-boundary-safe prefix of at most `max` bytes, retaining it from
/// `budget`. Returns an empty string once the budget is spent.
fn take(text: &str, budget: &mut usize) -> String {
    let clipped = bounded(text, (*budget).min(MAX_PART_TEXT_BYTES));
    *budget = (*budget).saturating_sub(clipped.len());
    clipped
}

/// Optional non-empty string field, bounded to the small metadata ceiling.
fn opt_string(message: &Value, key: &str) -> Option<String> {
    message
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(|text| bounded(text, MAX_METADATA_BYTES))
}

/// Typed tool call fact from the caller's real tool metadata: the live event
/// object (`name`, `call_id`/`id`) or the stored `ToolHistoryItem` shape
/// (`kind: "call"`, `call_id`, `name`). `None` when unnamed or unaffordable.
fn tool_call_part(message: &Value, budget: &mut usize) -> Option<ChatPart> {
    let call = message.get("tool_call")?;
    let name = take(call.get("name").and_then(Value::as_str)?, budget);
    if name.is_empty() {
        return None;
    }
    let id = call
        .get("call_id")
        .or_else(|| call.get("id"))
        .and_then(Value::as_str)
        .map(|id| take(id, budget))
        .unwrap_or_default();
    Some(ChatPart::ToolCall { name, id })
}

/// Typed tool result fact from the caller's real tool metadata. `is_error`
/// comes only from a real boolean flag; its absence is `false` (the wire
/// `function_call_output` carries no error flag — `server/src/agent_loop.rs:
/// 203-209`), never inferred from output text.
fn tool_result_part(message: &Value, budget: &mut usize) -> Option<ChatPart> {
    let output = message.get("tool_output")?;
    let name = take(output.get("name").and_then(Value::as_str)?, budget);
    if name.is_empty() {
        return None;
    }
    let text = take(
        output
            .get("output")
            .or_else(|| output.get("payload"))
            .and_then(Value::as_str)?,
        budget,
    );
    let is_error = output
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Some(ChatPart::ToolResult {
        name,
        output: text,
        is_error,
    })
}

fn tool_identity_complete(part: &Value) -> bool {
    part.get("round_id").and_then(Value::as_str).is_some_and(|id| !id.is_empty())
        && part.get("pair_index").and_then(Value::as_u64).is_some()
}

fn same_optional_tool_field(left: &Value, right: &Value, key: &str) -> bool {
    match (
        left.get(key).filter(|value| !value.is_null()),
        right.get(key).filter(|value| !value.is_null()),
    ) {
        (None, None) => true,
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

fn same_tool_pair_metadata(call: &Value, result: &Value) -> bool {
    same_optional_tool_field(call, result, "round_id")
        && same_optional_tool_field(call, result, "pair_index")
}

fn tool_pair_matches(call: &Value, result: &Value) -> bool {
    let id = call.get("call_id").and_then(Value::as_str).unwrap_or("");
    !id.is_empty()
        && result.get("call_id").and_then(Value::as_str) == Some(id)
        && call.get("name").and_then(Value::as_str).is_some_and(|name| !name.is_empty())
        && call.get("name") == result.get("name")
        && same_tool_pair_metadata(call, result)
}

fn hydrated_parts(message: &Value, budget: &mut usize) -> Vec<ChatPart> {
    let Some(all_parts) = message.get("parts").and_then(Value::as_array) else {
        return Vec::new();
    };
    let parts = &all_parts[..all_parts.len().min(64)];
    let mut call_outputs = vec![None; parts.len()];
    let mut paired_outputs = vec![false; parts.len()];
    let mut ambiguous = vec![false; parts.len()];

    // The server emits tool records as role=tool, one storage pair per message.
    // Match only that real role. Duplicate IDs require unique storage identity.
    if role_of(message) != Some(ChatRole::Tool) {
        // Do not fold non-tool records, but quarantine duplicate identities
        // instead of presenting their independent statuses as authoritative.
        for (index, part) in parts.iter().enumerate() {
            if !matches!(part.get("type").and_then(Value::as_str), Some("tool_call" | "tool_output")) {
                continue;
            }
            let id = part.get("call_id").and_then(Value::as_str).unwrap_or("");
            if !id.is_empty() && parts.iter().filter(|other| {
                matches!(other.get("type").and_then(Value::as_str), Some("tool_call" | "tool_output"))
                    && other.get("call_id").and_then(Value::as_str) == Some(id)
            }).count() > 1 {
                for (other_index, other) in parts.iter().enumerate() {
                    if matches!(other.get("type").and_then(Value::as_str), Some("tool_call" | "tool_output"))
                        && other.get("call_id").and_then(Value::as_str) == Some(id)
                    {
                        ambiguous[other_index] = true;
                    }
                }
            }
        }
    }
    if role_of(message) == Some(ChatRole::Tool) {
        if all_parts.len() > 64 {
            for (index, part) in parts.iter().enumerate() {
                if matches!(part.get("type").and_then(Value::as_str), Some("tool_call" | "tool_output")) {
                    ambiguous[index] = true;
                }
            }
        } else {
            let mut handled = vec![false; parts.len()];
            for (index, part) in parts.iter().enumerate() {
                if handled[index] {
                    continue;
                }
                let kind = part.get("type").and_then(Value::as_str).unwrap_or("");
                if !matches!(kind, "tool_call" | "tool_output") {
                    continue;
                }
                let id = part.get("call_id").and_then(Value::as_str).unwrap_or("");
                if id.is_empty() {
                    continue;
                }
                let group: Vec<_> = parts.iter().enumerate().filter_map(|(other_index, other)| {
                    (matches!(other.get("type").and_then(Value::as_str), Some("tool_call" | "tool_output"))
                        && other.get("call_id").and_then(Value::as_str) == Some(id))
                        .then_some(other_index)
                }).collect();
                for other_index in &group {
                    handled[*other_index] = true;
                }
                let calls: Vec<_> = group.iter().copied().filter(|other_index| {
                    parts[*other_index].get("type").and_then(Value::as_str) == Some("tool_call")
                }).collect();
                let results: Vec<_> = group.iter().copied().filter(|other_index| {
                    parts[*other_index].get("type").and_then(Value::as_str) == Some("tool_output")
                }).collect();
                let duplicate_id = calls.len() > 1 || results.len() > 1;
                if duplicate_id && group.iter().any(|other_index| !tool_identity_complete(&parts[*other_index])) {
                    for other_index in group {
                        ambiguous[other_index] = true;
                    }
                    continue;
                }

                let mut candidates = vec![Vec::new(); calls.len()];
                let mut reverse = vec![Vec::new(); results.len()];
                for (call_offset, call_index) in calls.iter().enumerate() {
                    for (result_offset, result_index) in results.iter().enumerate() {
                        if tool_pair_matches(&parts[*call_index], &parts[*result_index]) {
                            candidates[call_offset].push(result_offset);
                            reverse[result_offset].push(call_offset);
                        }
                    }
                }
                if !duplicate_id && calls.len() == 1 && results.len() == 1
                    && candidates[0].len() != 1
                {
                    ambiguous[calls[0]] = true;
                    ambiguous[results[0]] = true;
                    continue;
                }
                for (call_offset, matches) in candidates.iter().enumerate() {
                    if duplicate_id && matches.is_empty() || matches.len() > 1 {
                        ambiguous[calls[call_offset]] = true;
                        for result_offset in matches {
                            ambiguous[results[*result_offset]] = true;
                        }
                    }
                }
                for (result_offset, matches) in reverse.iter().enumerate() {
                    if duplicate_id && matches.is_empty() || matches.len() > 1 {
                        ambiguous[results[result_offset]] = true;
                        for call_offset in matches {
                            ambiguous[calls[*call_offset]] = true;
                        }
                    }
                }
                for (call_offset, matches) in candidates.iter().enumerate() {
                    if matches.len() == 1 {
                        let result_offset = matches[0];
                        if reverse[result_offset].len() == 1
                            && !ambiguous[calls[call_offset]]
                            && !ambiguous[results[result_offset]]
                        {
                            call_outputs[calls[call_offset]] = Some(results[result_offset]);
                            paired_outputs[results[result_offset]] = true;
                        }
                    }
                }
            }
        }
    }

    let mut output = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        let text = |key: &str| part.get(key).and_then(Value::as_str).unwrap_or("");
        match text("type") {
            "tool_call" => {
                let name = take(text("name"), budget);
                let id = take(text("call_id"), budget);
                let result = call_outputs[index].map(|result_index| &parts[result_index]);
                let is_error = result.is_some_and(tool_output_is_error);
                let status = if ambiguous[index] {
                    "ambiguous"
                } else {
                    result.map_or_else(
                        || part.get("status").and_then(Value::as_str)
                            .filter(|status| !status.is_empty()).unwrap_or("pending"),
                        |_| if is_error { "error" } else { "completed" },
                    )
                };
                let status = take(status, budget);
                let args = take(
                    &part.get("arguments").map(Value::to_string).unwrap_or_default(),
                    budget,
                );
                let (body, patch) = result.map_or_else(
                    || (String::new(), None),
                    |result| tool_output_content(
                        result.get("output").or_else(|| result.get("payload"))
                            .and_then(Value::as_str).unwrap_or(""),
                        is_error,
                        budget,
                    ),
                );
                output.push(ChatPart::ToolState { name, id, status, args, output: body, is_error });
                if let Some(patch) = patch { output.push(patch); }
            }
            "tool_output" => {
                if paired_outputs[index] {
                    continue;
                }
                let is_error = tool_output_is_error(part);
                output.extend(tool_output_parts_bounded(
                    text("name"),
                    text("call_id"),
                    part.get("output").or_else(|| part.get("payload"))
                        .and_then(Value::as_str).unwrap_or(""),
                    is_error,
                    ambiguous[index],
                    budget,
                ));
            }
            "text" => output.push(ChatPart::Text(take(text("text"), budget))),
            "reasoning" => output.push(ChatPart::Thinking(take(text("text"), budget))),
            "file" => output.push(ChatPart::File {
                filename: take(part.get("filename").or_else(|| part.get("path"))
                    .and_then(Value::as_str).unwrap_or(""), budget),
                mime: take(text("mime"), budget),
            }),
            "subtask" | "task" => output.push(ChatPart::Subtask {
                agent: take(text("agent"), budget), description: take(text("description"), budget),
                background: part.get("background").and_then(Value::as_bool).unwrap_or(false),
            }),
            "retry" => output.push(ChatPart::Retry {
                attempt: part.get("attempt").and_then(Value::as_u64).unwrap_or(0)
                    .min(usize::MAX as u64) as usize,
                message: take(text("message"), budget),
            }),
            "step-start" | "step-finish" | "snapshot" | "agent" | "agent-switched"
            | "model-switched" => output.push(ChatPart::Step {
                kind: take(text("type"), budget), detail: take(&part.to_string(), budget),
            }),
            "compaction" => output.push(ChatPart::Compaction { reason: take(text("reason"), budget) }),
            "patch" => output.push(ChatPart::Patch {
                path: take(text("path"), budget), diff: take(text("diff"), budget),
            }),
            _ => {}
        }
    }
    output
}

/// Only a versioned real-tool envelope supplies diff facts. Ordinary prose
/// resembling a patch stays ordinary tool output.
pub fn tool_output_parts(name: &str, id: &str, output: &str, is_error: bool) -> Vec<ChatPart> {
    let mut budget = MAX_MESSAGE_BYTES;
    tool_output_parts_bounded(name, id, output, is_error, false, &mut budget)
}
fn tool_output_parts_bounded(
    name: &str,
    id: &str,
    output: &str,
    is_error: bool,
    ambiguous: bool,
    budget: &mut usize,
) -> Vec<ChatPart> {
    let status = take(if ambiguous { "ambiguous" } else if is_error { "error" } else { "completed" }, budget);
    let name = take(name, budget);
    let id = take(id, budget);
    let (body, patch) = tool_output_content(output, is_error, budget);
    let mut parts = vec![ChatPart::ToolState {
        name, id, status, args: String::new(), output: body, is_error,
    }];
    if let Some(patch) = patch { parts.push(patch); }
    parts
}

fn tool_output_is_error(part: &Value) -> bool {
    part.get("is_error").and_then(Value::as_bool)
        .or_else(|| part.get("success").and_then(Value::as_bool).map(|success| !success))
        .or_else(|| part.get("error").map(|error| !error.is_null()))
        .unwrap_or(false)
}

fn tool_output_content(output: &str, is_error: bool, budget: &mut usize) -> (String, Option<ChatPart>) {
    let mut end = output.len().min(MAX_MESSAGE_BYTES);
    while !output.is_char_boundary(end) { end -= 1; }
    let output = &output[..end];
    let typed = serde_json::from_str::<Value>(output).ok()
        .filter(|value| value["format"] == "opencode.file-edit.v1" && !is_error);
    let body = typed.as_ref().and_then(|value| value["output"].as_str()).unwrap_or(output);
    let body = take(body, budget);
    let patch = typed.and_then(|edit| edit["diff"].as_str().map(|diff| ChatPart::Patch {
        path: take(edit["path"].as_str().unwrap_or(""), budget),
        diff: take(diff, budget),
    }));
    (body, patch)
}

/// Project one daemon message record into a typed chat message.
///
/// `None` for anything that is not a well-formed record with at least one
/// displayable part: non-objects, unknown/missing roles, blob bodies with no
/// typed part, and empty inline text. The projection is deterministic and
/// restart-stable (same JSON → equal `ChatMessage`).
#[must_use]
pub fn project_message(message: &Value) -> Option<ChatMessage> {
    if !message.is_object() {
        return None;
    }
    let role = role_of(message)?;
    let mut budget = MAX_MESSAGE_BYTES;
    let mut parts: Vec<ChatPart> = Vec::with_capacity(4);

    parts.extend(hydrated_parts(message, &mut budget));

    // Real inline payload only: `{"body":{"storage":"inline","text":...}}`.
    // A blob body has no inline text here and never gets a placeholder.
    if message.get("parts").and_then(Value::as_array).is_none() {
        if let Some(text) = message
            .get("body")
            .filter(|body| body.get("storage").and_then(Value::as_str) == Some("inline"))
            .and_then(|body| body.get("text"))
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
        {
            let text = take(text, &mut budget);
            if !text.is_empty() {
                parts.push(ChatPart::Text(text));
            }
        }
    }

    // Persisted reasoning summary (real typed part; `writer_v2.rs:17`).
    if let Some(summary) = message
        .get("reasoning_summary")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
    {
        let summary = take(summary, &mut budget);
        if !summary.is_empty() {
            parts.push(ChatPart::Thinking(summary));
        }
    }

    if let Some(call) = tool_call_part(message, &mut budget) {
        parts.push(call);
    }
    if let Some(result) = tool_result_part(message, &mut budget) {
        parts.push(result);
    }

    if parts.is_empty() {
        return None;
    }
    Some(ChatMessage {
        role,
        parts,
        agent: opt_string(message, "agent"),
        model: opt_string(message, "model"),
    })
}

/// Project a bounded daemon history array: at most [`MAX_HISTORY_SCAN`] raw
/// entries are examined and at most [`MAX_HISTORY_MESSAGES`] messages are
/// returned, in original order. Malformed entries are skipped, never replaced
/// by a fabricated message. This is not whole-conversation pagination.
#[must_use]
pub fn project_history(messages: &[Value]) -> Vec<ChatMessage> {
    let from = messages.len().saturating_sub(MAX_HISTORY_SCAN);
    let mut projected: Vec<ChatMessage> = Vec::new();
    for message in &messages[from..] {
        if let Some(message) = project_message(message) {
            projected.push(message);
        }
    }
    let overflow = projected.len().saturating_sub(MAX_HISTORY_MESSAGES);
    if overflow > 0 {
        projected.drain(..overflow);
    }
    projected
}

/// Project the same bounded history while retaining the backend message id
/// beside each display message.  The id is an anchor identity only; all
/// rendering still uses the exact typed projection returned by
/// `project_history`.
#[must_use]
pub fn project_history_entries(messages: &[Value]) -> Vec<(String, ChatMessage)> {
    let from = messages.len().saturating_sub(MAX_HISTORY_SCAN);
    let mut projected = Vec::new();
    for message in &messages[from..] {
        let Some(id) = message.get("id").and_then(Value::as_str) else { continue; };
        let Some(message) = project_message(message) else { continue; };
        projected.push((id.to_owned(), message));
    }
    let overflow = projected.len().saturating_sub(MAX_HISTORY_MESSAGES);
    if overflow > 0 { projected.drain(..overflow); }
    projected
}

#[cfg(test)]
mod history_window_tests {
    use super::*;
    use serde_json::json;
    fn row(id: usize) -> Value { json!({"id": format!("{id:03}"), "role":"user", "body":{"storage":"inline","text":format!("entry{id:03}")}}) }
    #[test]
    fn rejects_malformed_page_without_losing_visible_messages() {
        let initial = vec![row(200), row(201)];
        let cursor = "11111111-1111-4111-8111-111111111111";
        let mut window = NativeHistoryWindow::new(&initial, Some(cursor.to_owned()));
        let before = window.messages().to_vec();
        let error = window.load_older(|_, _| Ok(json!({"next_before": "199"}))).unwrap_err();
        assert!(error.contains("missing messages"));
        assert_eq!(window.messages(), before.as_slice());
        assert_eq!(window.next_before(), Some(cursor));
    }

    #[test]
    fn retains_bounded_window_without_lifetime_traversal_cutoff() {
        let initial: Vec<_> = (300..500).map(row).collect();
        let cursor = "22222222-2222-4222-8222-222222222222";
        let mut window = NativeHistoryWindow::new(&initial, Some(cursor.to_owned()));
        let changed = window.load_older(|_, _| {
            Ok(json!({"messages": [row(100), row(200)], "next_before": null}))
        }).unwrap();
        assert!(changed);
        assert_eq!(window.messages().len(), NativeHistoryWindow::MAX_MESSAGES);
        assert_eq!(window.messages().first().and_then(|v| v["id"].as_str()), Some("100"));
        assert_eq!(window.messages().last().and_then(|v| v["id"].as_str()), Some("497"));
        assert!(!window.has_older());
    }

    #[test]
    fn latest_recovery_replaces_evicted_window() {
        let initial: Vec<_> = (300..500).map(row).collect();
        let mut window = NativeHistoryWindow::new(&initial, Some("77777777-7777-4777-8777-777777777777".into()));
        window.load_older(|_, _| Ok(json!({"messages": [row(0)], "next_before": null}))).unwrap();
        assert_eq!(window.messages().first().and_then(|v| v["id"].as_str()), Some("000"));
        assert!(window.replace_latest(&json!({"messages": (500..600).map(row).collect::<Vec<_>>(), "next_before": null})).unwrap());
        assert_eq!(window.messages().first().and_then(|v| v["id"].as_str()), Some("500"));
        assert_eq!(window.messages().last().and_then(|v| v["id"].as_str()), Some("599"));
    }

    #[test]
    fn traverses_beyond_any_retained_window_and_restores_newest_page() {
        let initial: Vec<_> = (400..500).map(row).collect();
        let cursors = [
            "33333333-3333-4333-8333-333333333333",
            "44444444-4444-4444-8444-444444444444",
            "55555555-5555-4555-8555-555555555555",
            "66666666-6666-4666-8666-666666666666",
        ];
        let mut window = NativeHistoryWindow::new(&initial, Some(cursors[0].to_owned()));
        for page in 0..4 {
            let cursor = &cursors[page];
            let start = 300usize.saturating_sub(page * 100);
            let next = if page == 3 { None } else { Some(cursors[page + 1].to_owned()) };
            window.load_older(|_, _| Ok(json!({
                "messages": (start..start + 100).map(row).collect::<Vec<_>>(),
                "next_before": next,
            }))).unwrap();
        }
        assert_eq!(window.messages().len(), NativeHistoryWindow::MAX_MESSAGES);
        assert_eq!(window.messages().first().and_then(|v| v["id"].as_str()), Some("000"));
        assert!(!window.has_older());
        assert!(window.restore_newest());
        assert_eq!(window.messages().last().and_then(|v| v["id"].as_str()), Some("499"));
    }

    #[test]
    fn resumable_newer_walk_finds_adjacent_page_across_four_boundaries() {
        let initial: Vec<_> = (401..501).map(row).collect();
        let mut window = NativeHistoryWindow::new(&initial, Some("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into()));
        for (start, next) in [(301, "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"),
            (201, "cccccccc-cccc-4ccc-8ccc-cccccccccccc"),
            (101, "dddddddd-dddd-4ddd-8ddd-dddddddddddd")] {
            window.load_older(|_, _| Ok(json!({
                "messages": (start..start + 100).map(row).collect::<Vec<_>>(),
                "next_before": next,
            }))).unwrap();
        }
        assert_eq!(window.messages().first().and_then(|v| v["id"].as_str()), Some("101"));
        // Exhaust the retained window to the oldest records.
        window.load_older(|_, _| Ok(json!({
            "messages": (1..101).map(row).collect::<Vec<_>>(),
            "next_before": null,
        }))).unwrap();
        assert_eq!(window.messages().first().and_then(|v| v["id"].as_str()), Some("001"));
        window.begin_newer_recovery().unwrap();
        assert!(!window.advance_newer_recovery(&json!({"messages": (301..401).map(row).collect::<Vec<_>>(), "next_before": "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee"})).unwrap());
        assert!(!window.advance_newer_recovery(&json!({"messages": (201..301).map(row).collect::<Vec<_>>(), "next_before": "ffffffff-ffff-4fff-8fff-ffffffffffff"})).unwrap());
        assert!(window.advance_newer_recovery(&json!({"messages": (101..201).map(row).collect::<Vec<_>>(), "next_before": "99999999-9999-4999-8999-999999999999"})).unwrap());
        assert_eq!(window.newer_page.as_ref().unwrap().first().and_then(|v| v["id"].as_str()), Some("201"));
    }

    fn uuid_row(index: usize) -> Value {
        json!({
            "id": format!("00000000-0000-4000-8000-{index:012x}"),
            "role": "user",
            "body": {"storage": "inline", "text": format!("entry{index:03}")}
        })
    }

    #[test]
    fn newer_eviction_retains_before_cursor_for_recoverable_older_history() {
        let initial: Vec<_> = (0..200).map(uuid_row).collect();
        let mut window = NativeHistoryWindow::new(&initial, None);
        window.set_adjacent_newer(&json!({
            "messages": (200..300).map(uuid_row).collect::<Vec<_>>()
        })).unwrap();
        assert!(window.advance_newer());
        assert!(window.has_older(), "eviction must expose the dropped older page");
        assert_eq!(window.next_before(), Some("00000000-0000-4000-8000-000000000064"));

        let mut requested = None;
        assert!(window.load_older(|before, _| {
            requested = Some(before.to_owned());
            Ok(json!({
                "messages": (0..100).map(uuid_row).collect::<Vec<_>>(),
                "next_before": null
            }))
        }).unwrap());
        assert_eq!(requested.as_deref(), Some("00000000-0000-4000-8000-000000000064"));
        assert_eq!(window.messages().len(), NativeHistoryWindow::MAX_MESSAGES);
        assert_eq!(window.messages().first().expect("recovered history")["id"], uuid_row(0)["id"]);
        assert_eq!(window.messages().last().expect("recovered tail")["id"], uuid_row(199)["id"]);
        assert!(!window.has_older());
    }

    #[test]
    fn newer_walk_keeps_immediate_candidate_when_first_page_overlaps_tail() {
        let initial: Vec<_> = (201..401).map(row).collect();
        let mut window = NativeHistoryWindow::new(
            &initial,
            Some("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into()),
        );
        window.tail_page = (401..501).map(row).collect();
        window.tail_before = Some("bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb".into());
        window.begin_newer_recovery().unwrap();
        assert!(window.advance_newer_recovery(&json!({
            "messages": (301..401).map(row).collect::<Vec<_>>(),
            "next_before": "cccccccc-cccc-4ccc-8ccc-cccccccccccc"
        })).unwrap());
        assert_eq!(window.newer_page.as_ref().unwrap().first().expect("adjacent page")["id"], "401");
        assert_eq!(window.newer_page.as_ref().unwrap().last().expect("adjacent tail")["id"], "500");
        assert!(window.advance_newer());
        assert_eq!(window.messages().len(), NativeHistoryWindow::MAX_MESSAGES);
        assert_eq!(window.messages().first().expect("bounded first")["id"], "301");
        assert_eq!(window.messages().last().expect("newest last")["id"], "500");
        let ids: Vec<_> = window.messages().iter().map(|entry| entry["id"].as_str().unwrap()).collect();
        assert!(ids.windows(2).all(|pair| pair[0] != pair[1]));
        assert!(window.at_newest_page());
    }
}

pub const HISTORY_PAGE_SIZE: usize = 100;

/// A bounded, cursor-addressed history window.  The daemon exposes an older
/// cursor only (`before`), so this type deliberately keeps the raw records and
/// cursor metadata together; callers can replace the presentation atomically
/// after a successful page fetch.
#[derive(Debug, Clone, PartialEq)]
pub struct NativeHistoryWindow {
    messages: Vec<Value>,
    next_before: Option<String>,
    exhausted: bool,
    /// The newest server page is retained as cursor metadata, not added to
    /// the browsing window.  This is the bounded recovery anchor used when
    /// older-page eviction drops the newest adjacent page.
    tail_page: Vec<Value>,
    tail_before: Option<String>,
    newer_page: Option<Vec<Value>>,
    newer_before: Option<String>,
    newer_candidate: Option<Vec<Value>>,
}

impl NativeHistoryWindow {
    pub const MAX_MESSAGES: usize = 200;

    pub fn new(initial: &[Value], next_before: Option<String>) -> Self {
        let mut messages = initial.to_vec();
        if messages.len() > Self::MAX_MESSAGES {
            messages.drain(..messages.len() - Self::MAX_MESSAGES);
        }
        Self {
            exhausted: next_before.is_none() || messages.is_empty(),
            tail_page: messages.clone(),
            tail_before: next_before.clone(),
            newer_page: None,
            newer_before: None,
            newer_candidate: None,
            messages,
            next_before,
        }
    }

    pub fn messages(&self) -> &[Value] { &self.messages }
    pub fn next_before(&self) -> Option<&str> { self.next_before.as_deref() }
    pub fn has_older(&self) -> bool { !self.exhausted && self.next_before.is_some() }
    pub fn has_newer_recovery(&self) -> bool { !self.tail_page.is_empty() && self.tail_before.is_some() }

    pub fn has_adjacent_newer(&self) -> bool { self.newer_page.as_ref().is_some_and(|page| !page.is_empty()) }
    /// Render-row budget occupied by the pending adjacent page.  This mirrors
    /// the flat transcript projection used by the native loop and lets the
    /// caller preserve the old visible tail when that page is admitted.
    pub fn adjacent_newer_rendered_rows(&self, width: usize) -> usize {
        self.newer_page.as_ref().map_or(0, |rows| {
            rows.iter().filter_map(|row| {
                row.get("body").and_then(|body| body.get("text")).and_then(Value::as_str)
            }).map(|text| crate::native_view_chat::transcript_entry_rows(width, text)).sum()
        })
    }
    pub fn adjacent_newer_messages(&self) -> Option<Vec<crate::native_view_chat::ChatMessage>> {
        self.newer_page.as_ref().map(|rows| project_history(rows))
    }
    pub fn at_newest_page(&self) -> bool {
        self.messages.last().and_then(|row| row.get("id").and_then(Value::as_str))
            == self.tail_page.last().and_then(|row| row.get("id").and_then(Value::as_str))
    }

    pub fn newer_recovery_cursor(&self) -> Option<&str> { self.newer_before.as_deref() }
    pub fn oldest_id(&self) -> Option<&str> { self.messages.first()?.get("id").and_then(Value::as_str) }

    pub fn newer_recovery_pending(&self) -> bool { self.newer_before.is_some() }

    /// Start a resumable before-cursor walk.  Only the cursor and one
    /// candidate page are retained; unlike the old helper this is not capped
    /// by an arbitrary page-count lifetime limit.
    pub fn begin_newer_recovery(&mut self) -> Result<(), String> {
        let before = self.tail_before.clone().ok_or_else(|| "newer history exhausted".to_owned())?;
        self.newer_before = Some(before);
        self.newer_candidate = if self.at_newest_page() {
            None
        } else if self.tail_page.is_empty() {
            None
        } else {
            Some(self.tail_page.clone())
        };
        self.newer_page = None;
        Ok(())
    }

    /// Consume one response from the resumable walk.  Pages are traversed
    /// newest-to-oldest until one overlaps the retained edge; the immediately
    /// preceding page is then the sole adjacent-newer page.
    pub fn advance_newer_recovery(&mut self, value: &Value) -> Result<bool, String> {
        let before = self.newer_before.clone().ok_or_else(|| "newer recovery is not pending".to_owned())?;
        let rows = value.get("messages").and_then(Value::as_array)
            .ok_or_else(|| "history page is missing messages".to_owned())?;
        if rows.len() > HISTORY_PAGE_SIZE { return Err("history page exceeds requested limit".to_owned()); }
        let next = value.get("next_before").and_then(Value::as_str).map(str::to_owned);
        if let Some(cursor) = &next {
            if cursor.parse::<opencode_rk_contracts::MessageId>().is_err() || cursor == &before {
                return Err("history page has an invalid cursor".to_owned());
            }
        }
        if rows.is_empty() { self.newer_before = None; self.newer_candidate = None; return Ok(false); }
        let overlaps = rows.iter().any(|row| {
            let id = row.get("id").and_then(Value::as_str);
            id.is_some() && self.messages.iter().any(|old| old.get("id").and_then(Value::as_str) == id)
        });
        if overlaps {
            self.newer_page = self.newer_candidate.take();
            self.newer_before = None;
            return Ok(self.has_adjacent_newer());
        }
        self.newer_candidate = Some(rows.to_vec());
        self.newer_before = next;
        if self.newer_before.is_none() { self.newer_candidate = None; }
        Ok(false)
    }

    /// Advance by the retained adjacent page. Only one neighboring page is
    /// kept, so this operation cannot grow the browsing cache.
    pub fn advance_newer(&mut self) -> bool {
        let Some(page) = self.newer_page.take() else { return false; };
        let mut merged = self.messages.clone();
        merged.extend(page);
        let evicted = merged.len().saturating_sub(Self::MAX_MESSAGES);
        if evicted > 0 {
            let first = merged[evicted]
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| "evicted history page has no message id".to_owned());
            let Ok(first) = first else { return false; };
            self.next_before = Some(first.to_owned());
            self.exhausted = false;
            merged.drain(..evicted);
        }
        self.messages = merged;
        true
    }

    pub fn set_adjacent_newer(&mut self, value: &Value) -> Result<(), String> {
        let rows = value.get("messages").and_then(Value::as_array)
            .ok_or_else(|| "history page is missing messages".to_owned())?;
        if rows.len() > HISTORY_PAGE_SIZE { return Err("history page exceeds requested limit".to_owned()); }
        self.newer_page = Some(rows.to_vec());
        Ok(())
    }

    /// Walk from the newest cursor until the current oldest id is found. The
    /// immediately preceding page is retained as the sole adjacent-newer
    /// page. The callback is invoked one page at a time by the async worker;
    /// the walk itself stores only two bounded pages.
    pub fn recover_adjacent_newer<F>(&mut self, oldest_id: &str, mut fetch: F) -> Result<bool, String>
    where
        F: FnMut(&str, usize) -> Result<Value, String>,
    {
        let mut before = self.tail_before.clone().ok_or_else(|| "newer history exhausted".to_owned())?;
        let mut candidate: Option<Vec<Value>> = None;
        loop {
            let value = fetch(&before, HISTORY_PAGE_SIZE)?;
            let rows = value.get("messages").and_then(Value::as_array)
                .ok_or_else(|| "history page is missing messages".to_owned())?;
            if rows.len() > HISTORY_PAGE_SIZE { return Err("history page exceeds requested limit".to_owned()); }
            if rows.iter().any(|row| row.get("id").and_then(Value::as_str) == Some(oldest_id)) {
                self.newer_page = candidate;
                return Ok(self.has_adjacent_newer());
            }
            if rows.is_empty() { return Ok(false); }
            candidate = Some(rows.to_vec());
            before = value.get("next_before").and_then(Value::as_str)
                .ok_or_else(|| "history page ended before current window".to_owned())?.to_owned();
            if before.parse::<opencode_rk_contracts::MessageId>().is_err() { return Err("history page has an invalid cursor".to_owned()); }
        }
    }

    /// Commit a fresh newest page obtained without a cursor. This is the
    /// before-only API's bounded recovery operation after older traversal has
    /// evicted the original tail page.
    pub fn replace_latest(&mut self, value: &Value) -> Result<bool, String> {
        let rows = value.get("messages").and_then(Value::as_array)
            .ok_or_else(|| "history page is missing messages".to_owned())?;
        if rows.len() > HISTORY_PAGE_SIZE { return Err("history page exceeds requested limit".to_owned()); }
        let next = value.get("next_before").and_then(Value::as_str).map(str::to_owned);
        if let Some(cursor) = &next {
            if cursor.parse::<opencode_rk_contracts::MessageId>().is_err() { return Err("history page has an invalid cursor".to_owned()); }
        }
        if rows.is_empty() { return Ok(false); }
        let changed = self.messages != *rows;
        self.messages = rows.clone();
        self.tail_page = rows.clone();
        self.tail_before = next.clone();
        self.next_before = next;
        self.exhausted = self.next_before.is_none();
        Ok(changed)
    }

    /// Refresh only the newest-page anchor.  This is used by same-session
    /// external-turn polling while the user is browsing older records: the
    /// browsed window and its cursor remain untouched.
    pub fn refresh_tail(&mut self, value: &Value) -> Result<bool, String> {
        let rows = value.get("messages").and_then(Value::as_array)
            .ok_or_else(|| "history page is missing messages".to_owned())?;
        if rows.len() > HISTORY_PAGE_SIZE { return Err("history page exceeds requested limit".to_owned()); }
        let next = value.get("next_before").and_then(Value::as_str).map(str::to_owned);
        if let Some(cursor) = &next {
            if cursor.parse::<opencode_rk_contracts::MessageId>().is_err() {
                return Err("history page has an invalid cursor".to_owned());
            }
        }
        if rows.is_empty() { return Ok(false); }
        let changed = self.tail_page != *rows;
        self.tail_page = rows.to_vec();
        self.tail_before = next;
        Ok(changed)
    }

    /// Restore the newest retained page after older-page prepending evicted
    /// it. The overlap is intentionally deduplicated by message id and the
    /// result remains bounded to one window.
    pub fn restore_newest(&mut self) -> bool {
        if self.tail_page.is_empty() { return false; }
        let mut merged = self.messages.clone();
        for row in &self.tail_page {
            let id = row.get("id").and_then(Value::as_str);
            if id.is_none() || !merged.iter().any(|existing| existing.get("id").and_then(Value::as_str) == id) {
                merged.push(row.clone());
            }
        }
        if merged == self.messages { return false; }
        if merged.len() > Self::MAX_MESSAGES { merged.drain(..merged.len() - Self::MAX_MESSAGES); }
        self.messages = merged;
        true
    }

    /// Recover the page immediately newer than the retained window.  The
    /// server is before-only, so recovery starts at the newest retained tail
    /// cursor and walks one bounded page per invocation.  No message or
    /// cursor lifetime limit is applied.
    pub fn load_newer<F>(&mut self, fetch: F) -> Result<bool, String>
    where
        F: FnOnce(&str, usize) -> Result<Value, String>,
    {
        let before = self.tail_before.clone().ok_or_else(|| "newer history exhausted".to_owned())?;
        let value = fetch(&before, HISTORY_PAGE_SIZE)?;
        let rows = value.get("messages").and_then(Value::as_array)
            .ok_or_else(|| "history page is missing messages".to_owned())?;
        if rows.len() > HISTORY_PAGE_SIZE { return Err("history page exceeds requested limit".to_owned()); }
        let next = value.get("next_before").and_then(Value::as_str).map(str::to_owned);
        if let Some(cursor) = &next {
            if cursor.parse::<opencode_rk_contracts::MessageId>().is_err() || cursor == &before {
                return Err("history page has an invalid cursor".to_owned());
            }
        }
        if rows.is_empty() { self.tail_before = None; return Ok(false); }
        self.tail_page = rows.to_vec();
        self.tail_before = next;
        Ok(true)
    }

    /// Fetch and prepend one page.  The response is committed only after its
    /// shape/cursor is checked, so malformed pages leave the visible window
    /// untouched.  The retained window is bounded; cursor metadata remains
    /// valid even when newer records are evicted.
    pub fn load_older<F>(&mut self, fetch: F) -> Result<bool, String>
    where
        F: FnOnce(&str, usize) -> Result<Value, String>,
    {
        let before = self.next_before.clone().ok_or_else(|| "history exhausted".to_owned())?;
        if before.parse::<opencode_rk_contracts::MessageId>().is_err() {
            return Err("history cursor is not a canonical message id".to_owned());
        }
        let value = fetch(&before, HISTORY_PAGE_SIZE)?;
        let rows = value.get("messages").and_then(Value::as_array)
            .ok_or_else(|| "history page is missing messages".to_owned())?;
        if rows.len() > HISTORY_PAGE_SIZE { return Err("history page exceeds requested limit".to_owned()); }
        let next = value.get("next_before").and_then(Value::as_str).map(str::to_owned);
        if let Some(cursor) = &next {
            if cursor.parse::<opencode_rk_contracts::MessageId>().is_err() || cursor == &before {
                return Err("history page has an invalid cursor".to_owned());
            }
        }
        let mut committed = rows.to_vec();
        committed.extend(self.messages.iter().cloned());
        let evicted = committed.len().saturating_sub(Self::MAX_MESSAGES);
        if evicted > 0 {
            self.newer_page = Some(committed[Self::MAX_MESSAGES..].to_vec());
        }
        if committed.len() > Self::MAX_MESSAGES {
            committed.truncate(Self::MAX_MESSAGES);
        }
        let changed = !rows.is_empty();
        self.messages = committed;
        self.next_before = next;
        self.exhausted = rows.is_empty() || self.next_before.is_none();
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn actual_tool_record(parts: Value, body: &str) -> Value {
        json!({
            "id": "11111111-1111-4111-8111-111111111111",
            "session_id": "22222222-2222-4222-8222-222222222222",
            "role": "tool",
            "body": { "storage": "inline", "text": body },
            "created_at": "2026-10-19T00:00:00Z",
            "parts": parts,
        })
    }

    #[test]
    fn actual_emitted_tool_error_record_pairs_once_with_real_tool_role() {
        let record = actual_tool_record(json!([
            {"type":"tool_call","call_id":"call-7","name":"bash","status":"completed","arguments":{"command":"cat secret.txt"}},
            {"type":"tool_output","call_id":"call-7","name":"bash","output":"error: permission denied","success":false,"is_error":true,"error":"error: permission denied"}
        ]), "[bash] error: permission denied");
        let projected = project_message(&record).expect("server MessageRecord projects");
        assert_eq!(projected.role, ChatRole::Tool);
        assert_eq!(projected.parts.len(), 1);
        assert!(matches!(&projected.parts[0], ChatPart::ToolState {
            name, id, status, args, output, is_error
        } if name == "bash" && id == "call-7" && status == "error"
            && args.contains("cat secret.txt") && output == "error: permission denied" && *is_error));
    }

    #[test]
    fn actual_emitted_edit_record_keeps_typed_diff_and_shared_budget() {
        let output = json!({
            "format":"opencode.file-edit.v1", "path":"src/lib.rs",
            "diff":"--- a/src/lib.rs\n+++ b/src/lib.rs\n-old\n+new\n", "output":"Edit applied"
        }).to_string();
        let record = actual_tool_record(json!([
            {"type":"tool_call","call_id":"edit-3","name":"edit","status":"completed","arguments":{"path":"src/lib.rs","oldString":"old","newString":"new"}},
            {"type":"tool_output","call_id":"edit-3","name":"edit","output":output,"success":true,"is_error":false,"error":null}
        ]), "[edit] Edit applied");
        let projected = project_message(&record).expect("server edit MessageRecord projects");
        assert_eq!(projected.parts.len(), 2);
        assert!(matches!(&projected.parts[0], ChatPart::ToolState {
            name, id, status, output, is_error, ..
        } if name == "edit" && id == "edit-3" && status == "completed"
            && output == "Edit applied" && !is_error));
        assert!(matches!(&projected.parts[1], ChatPart::Patch { path, diff }
            if path == "src/lib.rs" && diff.contains("+new")));
        let retained: usize = projected.parts.iter().map(|part| match part {
            ChatPart::ToolState { name, id, status, args, output, .. } => name.len() + id.len() + status.len() + args.len() + output.len(),
            ChatPart::Patch { path, diff } => path.len() + diff.len(),
            _ => 0,
        }).sum();
        assert!(retained <= MAX_MESSAGE_BYTES);
    }

    #[test]
    fn duplicate_ids_without_storage_identity_are_quarantined_not_first_matched() {
        let record = actual_tool_record(json!([
            {"type":"tool_call","call_id":"dup","name":"bash","status":"completed","arguments":{"command":"one"}},
            {"type":"tool_call","call_id":"dup","name":"bash","status":"completed","arguments":{"command":"two"}},
            {"type":"tool_output","call_id":"dup","name":"bash","output":"one output","success":true,"is_error":false,"error":null}
        ]), "[bash] one output");
        let projected = project_message(&record).unwrap();
        assert_eq!(projected.parts.len(), 3);
        assert!(projected.parts.iter().all(|part| matches!(part,
            ChatPart::ToolState { status, .. } if status == "ambiguous")));
        let outputs = projected.parts.iter().filter(|part| matches!(part,
            ChatPart::ToolState { output, .. } if output == "one output")).count();
        assert_eq!(outputs, 1, "output remains visible but is not assigned to a call");
    }

    #[test]
    fn duplicate_ids_on_non_tool_records_are_quarantined_without_pairing() {
        let record = json!({"role":"assistant","parts":[
            {"type":"tool_call","call_id":"dup","name":"bash","status":"completed","arguments":{}},
            {"type":"tool_output","call_id":"dup","name":"bash","output":"result","success":true}
        ]});
        let projected = project_message(&record).unwrap();
        assert_eq!(projected.parts.len(), 2);
        assert!(projected.parts.iter().all(|part| matches!(part,
            ChatPart::ToolState { status, .. } if status == "ambiguous")));
    }

    #[test]
    fn duplicate_outputs_without_storage_identity_are_quarantined() {
        let record = actual_tool_record(json!([
            {"type":"tool_call","call_id":"dup","name":"bash","status":"completed","arguments":{}},
            {"type":"tool_output","call_id":"dup","name":"bash","output":"first","success":true},
            {"type":"tool_output","call_id":"dup","name":"bash","output":"second","success":false}
        ]), "[bash] second");
        let projected = project_message(&record).unwrap();
        assert_eq!(projected.parts.len(), 3);
        assert!(projected.parts.iter().all(|part| matches!(part,
            ChatPart::ToolState { status, .. } if status == "ambiguous")));
    }

    #[test]
    fn repeated_call_ids_pair_only_by_round_and_storage_pair_index() {
        let record = actual_tool_record(json!([
            {"type":"tool_call","call_id":"reuse","name":"bash","round_id":"r1","pair_index":0,"arguments":{}},
            {"type":"tool_call","call_id":"reuse","name":"bash","round_id":"r1","pair_index":1,"arguments":{}},
            {"type":"tool_output","call_id":"reuse","name":"bash","round_id":"r1","pair_index":1,"output":"second"},
            {"type":"tool_output","call_id":"reuse","name":"bash","round_id":"r1","pair_index":0,"output":"first"}
        ]), "[bash] first");
        let projected = project_message(&record).unwrap();
        assert_eq!(projected.parts.len(), 2);
        assert!(matches!(&projected.parts[0], ChatPart::ToolState { status, output, .. }
            if status == "completed" && output == "first"));
        assert!(matches!(&projected.parts[1], ChatPart::ToolState { status, output, .. }
            if status == "completed" && output == "second"));
    }

    #[test]
    fn actual_tool_output_clip_keeps_utf8_and_total_message_budget() {
        let record = actual_tool_record(json!([
            {"type":"tool_call","call_id":"clip","name":"bash","arguments":{}},
            {"type":"tool_output","call_id":"clip","name":"bash","output":"漢".repeat(20 * 1024),"success":true}
        ]), "[bash] clipped");
        let projected = project_message(&record).unwrap();
        let ChatPart::ToolState { name, id, status, args, output, .. } = &projected.parts[0] else {
            panic!("paired tool state expected");
        };
        assert!(output.chars().all(|character| character == '漢'));
        let retained = name.len() + id.len() + status.len() + args.len() + output.len();
        assert!(retained <= MAX_MESSAGE_BYTES);
        assert!(output.len() <= MAX_MESSAGE_BYTES);
        assert!(output.len() >= MAX_MESSAGE_BYTES - 64);
    }

    /// The exact `/api/sessions/{id}/messages` record shape
    /// (`contracts::MessageRecord` + `PayloadRef::Inline` tags).
    fn inline(role: &str, text: &str) -> Value {
        json!({
            "id": "11111111-1111-4111-8111-111111111111",
            "session_id": "22222222-2222-4222-8222-222222222222",
            "role": role,
            "body": { "storage": "inline", "text": text },
            "created_at": "2026-10-19T00:00:00Z",
        })
    }

    #[test]
    fn inline_records_project_their_explicit_role_and_text() {
        let user = project_message(&inline("user", "hi")).expect("user projects");
        assert_eq!(user.role, ChatRole::User);
        assert_eq!(user.parts, vec![ChatPart::Text("hi".to_owned())]);
        assert_eq!(user.agent, None);
        assert_eq!(user.model, None);

        let assistant = project_message(&inline("assistant", "hello")).expect("assistant");
        assert_eq!(assistant.role, ChatRole::Assistant);

        let system = project_message(&inline("system", "rules")).expect("system");
        assert_eq!(system.role, ChatRole::System);

        let tool = project_message(&inline("tool", "output")).expect("tool");
        assert_eq!(tool.role, ChatRole::Tool);
    }

    #[test]
    fn hydrated_typed_tool_parts_preserve_order_and_backend_status() {
        let mut value = inline("assistant", "answer");
        value["parts"] = serde_json::json!([
            {"type":"tool_call", "call_id":"c1", "name":"bash", "arguments":{"command":"printf hi"}},
            {"type":"tool_output", "call_id":"c1", "name":"bash", "output":"hi", "error":null}
        ]);
        let projected = project_message(&value).expect("typed message");
        assert_eq!(
            projected.parts.len(),
            2,
            "hydrated typed parts must be authoritative"
        );
        assert!(
            matches!(&projected.parts[0], ChatPart::ToolState { name, id, args, .. } if name == "bash" && id == "c1" && args.contains("printf hi"))
        );
        assert!(
            matches!(&projected.parts[1], ChatPart::ToolState { name, id, output, is_error, .. } if name == "bash" && id == "c1" && output == "hi" && !is_error)
        );
    }

    #[test]
    fn roles_are_never_parsed_from_text() {
        // Explicit role wins; a text body shaped like a role prefix is text.
        let message = project_message(&inline("assistant", "user: actually assistant")).unwrap();
        assert_eq!(message.role, ChatRole::Assistant);
        assert_eq!(
            message.parts,
            vec![ChatPart::Text("user: actually assistant".to_owned())]
        );
        // Unknown and missing roles project to None — never a fake System.
        assert!(
            project_message(&json!({
                "role": "wizard",
                "body": {"storage": "inline", "text": "x"}
            }))
            .is_none()
        );
        assert!(
            project_message(&json!({
                "body": {"storage": "inline", "text": "x"}
            }))
            .is_none()
        );
        assert!(project_message(&json!("user: hi")).is_none());
        assert!(project_message(&json!(null)).is_none());
        assert!(project_message(&json!({"role": 7, "body": {}})).is_none());
    }

    #[test]
    fn blob_body_without_typed_parts_projects_to_none_not_a_placeholder() {
        let blob = json!({
            "role": "user",
            "body": {"storage": "blob", "hash": "aaaaaaaa", "bytes": 4096}
        });
        assert!(project_message(&blob).is_none(), "no fabricated text");
        // Unknown storage tag is equally unpresentable here.
        assert!(
            project_message(&json!({
                "role": "user",
                "body": {"storage": "future", "text": "x"}
            }))
            .is_none()
        );
        // Empty inline text carries no part either.
        assert!(project_message(&inline("assistant", "")).is_none());
    }

    #[test]
    fn typed_tool_and_reasoning_metadata_project_from_real_fields() {
        let call = json!({"name": "bash", "call_id": "call-1", "arguments": "{}"});
        let output = json!({"name": "bash", "output": "exit 0", "call_id": "call-1"});
        let message = json!({
            "role": "assistant",
            "body": {"storage": "inline", "text": "running"},
            "reasoning_summary": "weigh options",
            "tool_call": call,
            "tool_output": output,
        });
        let projected = project_message(&message).expect("typed parts project");
        assert_eq!(
            projected.parts,
            vec![
                ChatPart::Text("running".to_owned()),
                ChatPart::Thinking("weigh options".to_owned()),
                ChatPart::ToolCall {
                    name: "bash".to_owned(),
                    id: "call-1".to_owned()
                },
                ChatPart::ToolResult {
                    name: "bash".to_owned(),
                    output: "exit 0".to_owned(),
                    is_error: false
                },
            ]
        );
        // Caller-known agent/model facts survive; absent ones stay None.
        let with_facts = project_message(&json!({
            "role": "assistant",
            "body": {"storage": "inline", "text": "x"},
            "agent": "build",
            "model": "provider/model-1"
        }))
        .unwrap();
        assert_eq!(with_facts.agent.as_deref(), Some("build"));
        assert_eq!(with_facts.model.as_deref(), Some("provider/model-1"));
    }

    #[test]
    fn is_error_comes_only_from_the_backend_flag() {
        let error_looking = project_message(&json!({
            "role": "tool",
            "tool_output": {"name": "bash", "output": "error: permission denied"}
        }))
        .expect("tool output projects");
        assert_eq!(
            error_looking.parts,
            vec![ChatPart::ToolResult {
                name: "bash".to_owned(),
                output: "error: permission denied".to_owned(),
                is_error: false, // text shape is never a flag
            }]
        );
        let flagged = project_message(&json!({
            "role": "tool",
            "tool_output": {"name": "bash", "output": "denied", "is_error": true}
        }))
        .unwrap();
        assert_eq!(
            flagged.parts,
            vec![ChatPart::ToolResult {
                name: "bash".to_owned(),
                output: "denied".to_owned(),
                is_error: true
            }]
        );
        // Stored ToolHistoryItem shape: `payload` is the real output field.
        let stored = project_message(&json!({
            "role": "tool",
            "tool_output": {
                "kind": "output",
                "call_id": "c-9",
                "name": "read",
                "payload": "file body"
            }
        }))
        .unwrap();
        assert_eq!(
            stored.parts,
            vec![ChatPart::ToolResult {
                name: "read".to_owned(),
                output: "file body".to_owned(),
                is_error: false
            }]
        );
        // `id` is accepted as the call id when `call_id` is absent.
        let by_id = project_message(&json!({
            "role": "assistant",
            "tool_call": {"name": "grep", "id": "fc-2"}
        }))
        .unwrap();
        assert_eq!(
            by_id.parts,
            vec![ChatPart::ToolCall {
                name: "grep".to_owned(),
                id: "fc-2".to_owned()
            }]
        );
        // A tool part without a name is refused, never guessed.
        assert!(
            project_message(&json!({
                "role": "tool",
                "tool_output": {"output": "x"}
            }))
            .is_none()
        );
    }

    #[test]
    fn parts_are_bounded_and_clip_on_char_boundaries() {
        let big = "漢".repeat(20 * 1024); // 60 KiB of 3-byte scalars
        let projected = project_message(&json!({
            "role": "assistant",
            "body": {"storage": "inline", "text": big}
        }))
        .expect("big text projects");
        let ChatPart::Text(text) = &projected.parts[0] else {
            panic!("expected text part");
        };
        assert!(text.len() <= MAX_MESSAGE_BYTES, "message byte budget");
        assert!(
            text.len() >= MAX_MESSAGE_BYTES - 2,
            "clip is a char boundary"
        );
        assert!(text.chars().all(|c| c == '漢'), "no split scalar");
        // Repeated large parts share the one message budget.
        let crowded = project_message(&json!({
            "role": "assistant",
            "body": {"storage": "inline", "text": "x".repeat(1024 * 1024)},
            "reasoning_summary": "y".repeat(1024 * 1024),
            "tool_call": {"name": "z".repeat(1024 * 1024), "call_id": "c"},
            "tool_output": {"name": "w", "output": "q".repeat(1024 * 1024)}
        }))
        .unwrap();
        let retained: usize = crowded
            .parts
            .iter()
            .map(|part| match part {
                ChatPart::Text(text) | ChatPart::Thinking(text) => text.len(),
                ChatPart::ToolCall { name, id } => name.len() + id.len(),
                ChatPart::ToolState {
                    name,
                    id,
                    status,
                    args,
                    output,
                    ..
                } => name.len() + id.len() + status.len() + args.len() + output.len(),
                ChatPart::File { filename, mime } => filename.len() + mime.len(),
                ChatPart::Subtask {
                    agent, description, ..
                } => agent.len() + description.len(),
                ChatPart::Retry { message, .. } => message.len(),
                ChatPart::Step { kind, detail } => kind.len() + detail.len(),
                ChatPart::Compaction { reason } => reason.len(),
                ChatPart::Patch { path, diff } => path.len() + diff.len(),
                ChatPart::ToolResult { name, output, .. } => name.len() + output.len(),
            })
            .sum();
        assert!(
            retained <= MAX_MESSAGE_BYTES,
            "one message retains a bounded byte budget, got {retained}"
        );
        assert_eq!(
            crowded.parts.len(),
            1,
            "the spent budget refuses later parts instead of overshooting"
        );
    }

    #[test]
    fn history_returns_the_bounded_fetch_and_skips_malformed_entries() {
        let mut messages: Vec<Value> = Vec::new();
        for index in 0..240 {
            let role = if index % 2 == 0 { "user" } else { "assistant" };
            messages.push(inline(role, &format!("entry {index}")));
        }
        messages.push(json!({"role": "wizard", "body": {"storage": "inline", "text": "no"}}));
        messages.push(json!("not a message"));
        let projected = project_history(&messages);
        // The raw scan contains 200 entries: 198 valid records plus the two
        // malformed records at its end. Projection skips malformed input; it
        // must not synthesize two messages merely to fill the output cap.
        assert_eq!(projected.len(), 198, "bounded raw scan with malformed entries");
        assert_eq!(
            projected.first().map(|m| m.parts.clone()),
            Some(vec![ChatPart::Text("entry 42".to_owned())]),
            "oldest valid record in the bounded raw window"
        );
        assert_eq!(
            projected.last().map(|m| m.parts.clone()),
            Some(vec![ChatPart::Text("entry 239".to_owned())]),
            "malformed entries are skipped, not replaced"
        );
        // Restart-stable: the same input projects to an equal message list.
        assert_eq!(project_history(&messages), projected);
    }

    #[test]
    fn public_projection_of_sixty_one_records_retains_the_oldest_record() {
        let messages: Vec<Value> = (0..61)
            .map(|index| inline("user", &format!("entry{index:03}")))
            .collect();
        let projected = project_history(&messages);
        assert_eq!(projected.len(), 61);
        assert_eq!(projected[0].parts, vec![ChatPart::Text("entry000".to_owned())]);
        assert_eq!(projected[60].parts, vec![ChatPart::Text("entry060".to_owned())]);
    }

    #[test]
    fn projection_cases_63_201_and_501_preserve_bounded_page_semantics() {
        for count in [63, 201, 501] {
            let messages: Vec<Value> = (0..count)
                .map(|index| inline("user", &format!("entry{index:03}")))
                .collect();
            let projected = project_history(&messages);
            assert_eq!(projected.len(), count.min(MAX_HISTORY_MESSAGES));
            let expected_first = count.saturating_sub(MAX_HISTORY_MESSAGES);
            assert_eq!(
                projected.first().unwrap().parts,
                vec![ChatPart::Text(format!("entry{expected_first:03}"))]
            );
            assert_eq!(
                projected.last().unwrap().parts,
                vec![ChatPart::Text(format!("entry{:03}", count - 1))]
            );
        }
    }

    #[test]
    fn projected_sixty_one_records_reach_oldest_after_repeated_native_scroll() {
        let messages: Vec<Value> = (0..61)
            .map(|index| inline("user", &format!("entry{index:03}")))
            .collect();
        let projected = project_history(&messages);
        let mut view = crate::native_view::NativeView::new(100, 32);
        for _ in 0..80 {
            view.scroll_up(3);
        }
        let frame = view.render_typed("", &projected, false, &[]).join("\n");
        assert!(frame.contains("entry000"), "oldest projected record must be reachable");
        assert!(frame.contains("entry006"), "oldest viewport must contain adjacent records");
    }

    #[test]
    fn history_scan_is_bounded_for_long_hostile_arrays() {
        let filler = json!({"role": "wizard"});
        let mut messages = vec![filler.clone(); 10_000];
        messages.push(inline("user", "tail"));
        let projected = project_history(&messages);
        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].parts, vec![ChatPart::Text("tail".to_owned())]);
    }
}
