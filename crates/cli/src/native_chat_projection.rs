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
//! - `crates/cli/src/tui_entry.rs:399` renders the live snapshot as
//!   `messages.iter().rev().take(40)`: the tail-40 cap preserved here.
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
/// Max projected messages returned by [`project_history`] (the live snapshot
/// renders the last 40 history rows).
pub const MAX_HISTORY_MESSAGES: usize = 40;
/// Max raw entries examined from the tail of the history slice.
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

fn hydrated_parts(message: &Value, budget: &mut usize) -> Vec<ChatPart> {
    let Some(parts) = message.get("parts").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut output = Vec::new();
    for part in parts.iter().take(64) {
        let text = |key: &str| part.get(key).and_then(Value::as_str).unwrap_or("");
        let projected = match text("type") {
            "tool_call" => Some(ChatPart::ToolState {
                name: take(text("name"), budget),
                id: take(text("call_id"), budget),
                status: take(
                    part.get("status").and_then(Value::as_str).unwrap_or("call"),
                    budget,
                ),
                args: take(
                    &part
                        .get("arguments")
                        .map(Value::to_string)
                        .unwrap_or_default(),
                    budget,
                ),
                output: String::new(),
                is_error: false,
            }),
            "tool_output" => {
                let is_error = part
                    .get("is_error")
                    .and_then(Value::as_bool)
                    .or_else(|| {
                        part.get("success")
                            .and_then(Value::as_bool)
                            .map(|success| !success)
                    })
                    .or_else(|| part.get("error").map(|error| !error.is_null()))
                    .unwrap_or(false);
                output.extend(tool_output_parts_bounded(
                    text("name"),
                    text("call_id"),
                    text("output"),
                    is_error,
                    budget,
                ));
                None
            }
            "text" => Some(ChatPart::Text(take(text("text"), budget))),
            "reasoning" => Some(ChatPart::Thinking(take(text("text"), budget))),
            "file" => Some(ChatPart::File {
                filename: take(
                    part.get("filename")
                        .or_else(|| part.get("path"))
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                    budget,
                ),
                mime: take(text("mime"), budget),
            }),
            "subtask" | "task" => Some(ChatPart::Subtask {
                agent: take(text("agent"), budget),
                description: take(text("description"), budget),
                background: part
                    .get("background")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            }),
            "retry" => Some(ChatPart::Retry {
                attempt: part
                    .get("attempt")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
                    .min(usize::MAX as u64) as usize,
                message: take(text("message"), budget),
            }),
            "step-start" | "step-finish" | "snapshot" | "agent" | "agent-switched"
            | "model-switched" => Some(ChatPart::Step {
                kind: take(text("type"), budget),
                detail: take(&part.to_string(), budget),
            }),
            "compaction" => Some(ChatPart::Compaction {
                reason: take(text("reason"), budget),
            }),
            "patch" => Some(ChatPart::Patch {
                path: take(text("path"), budget),
                diff: take(text("diff"), budget),
            }),
            _ => None,
        };
        if let Some(part) = projected {
            output.push(part);
        }
    }
    output
}

/// Only a versioned real-tool envelope supplies diff facts. Ordinary prose
/// resembling a patch stays ordinary tool output.
pub fn tool_output_parts(name: &str, id: &str, output: &str, is_error: bool) -> Vec<ChatPart> {
    let mut budget = MAX_MESSAGE_BYTES;
    tool_output_parts_bounded(name, id, output, is_error, &mut budget)
}
fn tool_output_parts_bounded(
    name: &str,
    id: &str,
    output: &str,
    is_error: bool,
    budget: &mut usize,
) -> Vec<ChatPart> {
    let mut parts = Vec::new();
    let mut end = output.len().min(MAX_MESSAGE_BYTES);
    while !output.is_char_boundary(end) {
        end -= 1;
    }
    let output = &output[..end];
    let typed = serde_json::from_str::<Value>(output)
        .ok()
        .filter(|value| value["format"] == "opencode.file-edit.v1" && !is_error);
    let body = typed
        .as_ref()
        .and_then(|value| value["output"].as_str())
        .unwrap_or(output);
    let status = take(if is_error { "error" } else { "result" }, budget);
    parts.push(ChatPart::ToolState {
        name: take(name, budget),
        id: take(id, budget),
        status,
        args: String::new(),
        output: take(body, budget),
        is_error,
    });
    if let Some(edit) = typed {
        if let Some(diff) = edit["diff"].as_str() {
            parts.push(ChatPart::Patch {
                path: take(edit["path"].as_str().unwrap_or(""), budget),
                diff: take(diff, budget),
            });
        }
    }
    parts
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

/// Project the tail of a daemon history array: at most [`MAX_HISTORY_SCAN`]
/// raw entries are examined and at most [`MAX_HISTORY_MESSAGES`] messages are
/// returned, in original order. Malformed entries are skipped, never
/// replaced by a fabricated message.
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn actual_hydrated_call_arguments_and_diff_are_visible() {
        let record = json!({"role":"tool","parts":[
            {"type":"tool_call","call_id":"edit-real","name":"edit","status":"completed","arguments":{"path":"source","oldString":"before","newString":"after"}},
            {"type":"tool_output","call_id":"edit-real","name":"edit","success":true,"output":json!({"format":"opencode.file-edit.v1","path":"source","diff":"--- source\n+++ source\n@@ -1 +1 @@\n-before\n+after\n"}).to_string()}
        ]});
        let message = project_message(&record).unwrap();
        let rows = super::super::native_view_chat::ChatView::new().render_typed_classified(
            120,
            30,
            "workspace",
            &[message],
            "",
            false,
        );
        assert!(
            rows.iter().any(|(row, _)| row.contains("oldString")),
            "actual call arguments were discarded"
        );
        assert!(rows.iter().any(|(row, _)| row.contains("-before")));
        assert!(rows.iter().any(|(row, _)| row.contains("+after")));
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
    fn history_returns_the_bounded_tail_and_skips_malformed_entries() {
        let mut messages: Vec<Value> = Vec::new();
        for index in 0..100 {
            let role = if index % 2 == 0 { "user" } else { "assistant" };
            messages.push(inline(role, &format!("entry {index}")));
        }
        messages.push(json!({"role": "wizard", "body": {"storage": "inline", "text": "no"}}));
        messages.push(json!("not a message"));
        let projected = project_history(&messages);
        assert_eq!(projected.len(), MAX_HISTORY_MESSAGES, "tail 40 cap");
        assert_eq!(
            projected.first().map(|m| m.parts.clone()),
            Some(vec![ChatPart::Text("entry 60".to_owned())]),
            "oldest of the tail-40 window (entries 60..=99) first"
        );
        assert_eq!(
            projected.last().map(|m| m.parts.clone()),
            Some(vec![ChatPart::Text("entry 99".to_owned())]),
            "malformed entries are skipped, not replaced"
        );
        // Restart-stable: the same input projects to an equal message list.
        assert_eq!(project_history(&messages), projected);
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
