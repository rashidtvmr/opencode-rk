#![forbid(unsafe_code)]
//! Per-tool transcript rows: the 14 specialized tool presentations plus the
//! generic/plugin/MCP fallback, mirroring the pinned OpenCode TUI.
//!
//! Upstream evidence (pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`):
//! - tool name canonicalization and display helpers:
//!   `packages/tui/src/util/tool-display.ts:1-64` (`canonicalToolName`,
//!   `primitiveInputSummary`, `executeCalls`, `executeCallSummary`,
//!   `webSearchProviderName`, `webSearchProviderLabel`, `toolDisplayContent`);
//! - specialization dispatch and per-tool renderers:
//!   `packages/tui/src/routes/session/message-parts.tsx:19-47`
//!   (`INLINE_TOOL_ICON_WIDTH = 2`, `toolDisplays`) and
//!   `packages/tui/src/routes/session/index.tsx:2574-2648` (`ToolPart`
//!   switch), `:2736-2791` (`GenericTool`), `:2940-3157` (`Shell`),
//!   `:3159-3251` (`Write`), `:3253-3310` (`Glob`/`Read`/`Grep`),
//!   `:3312-3390` (`WebFetch`/`WebSearch`/`Subagent`), `:3348-3430`
//!   (`ExecuteCallView`/`Execute`), `:3604-3708` (`TodoWrite`/`Question`/
//!   `Skill`);
//! - status/error styling: `InlineTool`/`InlineToolRow`/`InlineToolLabel`
//!   (`index.tsx:2813-2883`, `message-parts.tsx:202-293`) and
//!   `BackgroundToolHint`/`StatusBadge` (`index.tsx:1894-1929`, `:2884-2892`);
//! - collapsed budgets: `packages/tui/src/util/collapse-tool-output.ts`
//!   (10-line shell budget, 2-row command, `(N earlier lines)` tail label);
//! - execute dialog model: `packages/tui/src/routes/session/dialog-execute.tsx`
//!   (`executeCode`, `outputText`, `highlightedOutput`, shared `digits`
//!   gutter, `GutteredCode`).
//!
//! This module is pure display: it parses only caller-supplied tool arguments
//! and output, never invents metadata, percentages or success. Rows carry the
//! existing [`ChatRowClass`] so the paint layer keeps owning colors; no new
//! dependencies and no IO.

use crate::native_view_chat::ChatRowClass;
use crate::native_view_layout::visit_unpadded_cells;
use serde_json::{Map, Value};
use unicode_segmentation::UnicodeSegmentation;

/// Upstream `maxLines = 10` collapsed budget for command + output
/// (`routes/session/index.tsx:3094-3097`).
pub const SHELL_BUDGET_LINES: usize = 10;
/// Reduced-motion spinner sentinel (TOOL-002): the live loop passes this when
/// `animations` is off so a busy row paints the upstream `⋯` fallback
/// (`component/spinner.tsx:20`) instead of an animated Braille frame. Any
/// other `Some(tick)` selects `spinner-frames` tick `tick % len`.
pub const SPINNER_REDUCED_MOTION: u64 = u64::MAX;
/// Upstream `commandLines = Math.min(2, maxLines)` (`collapse-tool-output.ts:36`).
pub const SHELL_COMMAND_LINES: usize = 2;
/// Upstream execute runtime-error preview: 4 lines
/// (`routes/session/index.tsx:3408-3410`).
pub const EXECUTE_PREVIEW_LINES: usize = 4;
/// Fixed icon column width (`message-parts.tsx:19`).
pub const ICON_COLUMN_CELLS: usize = 2;
/// Left padding for inline tool rows (`InlineToolRow`, `paddingLeft={3}`;
/// this transcript already indents bodies by two cells, so the icon column
/// starts at two).
pub const ROW_INDENT: &str = "  ";
/// Continuation/body indent: two cells of body padding plus the icon column.
pub const BODY_INDENT: &str = "    ";
/// Bound on parsed argument JSON (bytes) before it is ignored.
pub const MAX_ARGS_BYTES: usize = 64 * 1024;
/// Bound on one logical text field read from arguments.
pub const MAX_FIELD_BYTES: usize = 8 * 1024;
/// Bound on rows produced for one tool part.
pub const MAX_TOOL_ROWS: usize = 64;
/// Upper bound on `Read` `metadata.loaded` instruction-file rows
/// (`routes/session/index.tsx:3231-3243`), so a hostile payload cannot grow the
/// transcript unbounded.
pub const MAX_LOADED_ROWS: usize = 32;
/// Upper bound on one loaded-file path's characters.
pub const MAX_LOADED_PATH_CHARS: usize = 512;

const MAX_CELLS: usize = 64 * 1024;

/// `canonicalToolName` (`util/tool-display.ts:1-6`): legacy wire names map to
/// their current specialization; every other name (plugin/MCP/unknown) is
/// returned unchanged.
#[must_use]
pub fn canonical_tool_name(name: &str) -> &str {
    match name {
        "bash" => "shell",
        "task" => "subagent",
        "apply_patch" => "patch",
        other => other,
    }
}

/// The 14 specialized tool presentations dispatched by upstream `ToolPart`
/// (`routes/session/index.tsx:2597-2646`); anything else renders through the
/// generic fallback (`GenericTool`, `:2736`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Shell,
    Glob,
    Read,
    Grep,
    WebFetch,
    WebSearch,
    Write,
    Edit,
    Subagent,
    Execute,
    Patch,
    TodoWrite,
    Question,
    Skill,
    Generic,
}

impl ToolKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shell => "shell",
            Self::Glob => "glob",
            Self::Read => "read",
            Self::Grep => "grep",
            Self::WebFetch => "webfetch",
            Self::WebSearch => "websearch",
            Self::Write => "write",
            Self::Edit => "edit",
            Self::Subagent => "subagent",
            Self::Execute => "execute",
            Self::Patch => "patch",
            Self::TodoWrite => "todowrite",
            Self::Question => "question",
            Self::Skill => "skill",
            Self::Generic => "generic",
        }
    }
}

/// Canonical specialization for a raw tool name (legacy aliases first).
#[must_use]
pub fn tool_kind(name: &str) -> ToolKind {
    match canonical_tool_name(name) {
        "shell" => ToolKind::Shell,
        "glob" => ToolKind::Glob,
        "read" => ToolKind::Read,
        "grep" => ToolKind::Grep,
        "webfetch" => ToolKind::WebFetch,
        "websearch" => ToolKind::WebSearch,
        "write" => ToolKind::Write,
        "edit" => ToolKind::Edit,
        "subagent" => ToolKind::Subagent,
        "execute" => ToolKind::Execute,
        "patch" => ToolKind::Patch,
        "todowrite" => ToolKind::TodoWrite,
        "question" => ToolKind::Question,
        "skill" => ToolKind::Skill,
        _ => ToolKind::Generic,
    }
}

/// Display status (`state.status` plus the real `is_error` flag).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolStatus {
    Streaming,
    Running,
    Pending,
    Completed,
    Error,
    /// Permission/question rejection: upstream strikes the label and keeps the
    /// muted (not error) color (`index.tsx:2826-2837`).
    Denied,
}

impl ToolStatus {
    #[must_use]
    pub fn is_running(self) -> bool {
        matches!(self, Self::Streaming | Self::Running)
    }

    /// Row class used for the header/body. `ToolCall`/`ToolResult` are both
    /// painted muted; `ToolError` is the red feedback color
    /// (`tui_entry.rs:1968-1985`).
    #[must_use]
    pub fn row_class(self) -> ChatRowClass {
        match self {
            Self::Streaming | Self::Running | Self::Pending => ChatRowClass::ToolCall,
            Self::Completed => ChatRowClass::ToolResult,
            Self::Denied => ChatRowClass::ToolDenied,
            Self::Error => ChatRowClass::ToolError,
        }
    }
}

/// Classify a wire status string. `is_error` always wins over a completed
/// status; unknown statuses stay pending (never claimed as success).
#[must_use]
pub fn classify_status(status: &str, is_error: bool) -> ToolStatus {
    if is_error {
        return ToolStatus::Error;
    }
    match status {
        "streaming" => ToolStatus::Streaming,
        "running" => ToolStatus::Running,
        "completed" | "complete" | "ok" | "success" | "done" => ToolStatus::Completed,
        "error" | "failed" | "failure" | "denied" => ToolStatus::Error,
        _ => ToolStatus::Pending,
    }
}

/// Upstream `denied` classification (`index.tsx:2826-2833`): rejection wording
/// only; ordinary errors are never re-labeled as denials.
#[must_use]
pub fn is_denied_error(message: &str) -> bool {
    message.contains("QuestionRejectedError")
        || message.contains("rejected permission")
        || message.contains("specified a rule")
        || message.contains("user dismissed")
}

/// `webSearchProviderName` (`util/tool-display.ts:40-45`).
#[must_use]
pub fn web_search_provider_name(provider: Option<&str>) -> Option<String> {
    let provider = provider.filter(|value| !value.is_empty())?;
    if provider == "opencode" {
        return Some("OpenCode".to_owned());
    }
    let mut chars = provider.chars();
    let first = chars.next()?;
    Some(format!("{}{}", first.to_uppercase(), chars.as_str()))
}

/// `webSearchProviderLabel` (`util/tool-display.ts:47-50`).
#[must_use]
pub fn web_search_provider_label(provider: Option<&str>) -> String {
    web_search_provider_name(provider).map_or_else(
        || "Web Search".to_owned(),
        |name| format!("Web Search via {name}"),
    )
}

/// `primitiveInputSummary` (`util/tool-display.ts:12-22`): only top-level
/// string/number/boolean entries, one flattened line. Nested JSON stays out of
/// the inline row (expanded view only).
#[must_use]
pub fn primitive_input_summary(input: &Map<String, Value>, omit: &[&str]) -> String {
    let entries: Vec<String> = input
        .iter()
        .filter(|(key, _)| !omit.contains(&key.as_str()))
        .filter_map(|(key, value)| primitive_value(value).map(|value| format!("{key}={value}")))
        .collect();
    if entries.is_empty() {
        String::new()
    } else {
        format!("[{}]", entries.join(", "))
    }
}

fn primitive_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

/// Upstream `ExecuteCall` (`util/tool-display.ts:24-33`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecuteCall {
    pub tool: String,
    pub status: ExecuteCallStatus,
    pub input: Option<Map<String, Value>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecuteCallStatus {
    Running,
    Completed,
    Error,
}

/// `executeCalls` (`util/tool-display.ts:26-34`): malformed entries are
/// dropped, never displayed.
#[must_use]
pub fn execute_calls(value: &Value) -> Vec<ExecuteCall> {
    let Value::Array(items) = value else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|call| {
            let call = call.as_object()?;
            let tool = call.get("tool").and_then(Value::as_str)?;
            let status = match call.get("status").and_then(Value::as_str)? {
                "running" => ExecuteCallStatus::Running,
                "completed" => ExecuteCallStatus::Completed,
                "error" => ExecuteCallStatus::Error,
                _ => return None,
            };
            Some(ExecuteCall {
                tool: tool.to_owned(),
                status,
                input: call.get("input").and_then(Value::as_object).cloned(),
            })
        })
        .collect()
}

/// `executeCallSummary` (`util/tool-display.ts:37-40`).
#[must_use]
pub fn execute_call_summary(call: &ExecuteCall) -> String {
    let args = call
        .input
        .as_ref()
        .map(|input| primitive_input_summary(input, &[]))
        .unwrap_or_default();
    let args = args.split_whitespace().collect::<Vec<_>>().join(" ");
    if args.is_empty() {
        call.tool.clone()
    } else {
        format!("{} {args}", call.tool)
    }
}

/// `toolDisplayContent` (`util/tool-display.ts:59-62`): no output is shown
/// while the tool is still streaming or running.
#[must_use]
pub fn tool_display_content<'a>(status: &str, content: Option<&'a str>) -> Option<&'a str> {
    match classify_status(status, false) {
        ToolStatus::Streaming | ToolStatus::Running => None,
        _ => content,
    }
}

/// `isBackgroundSubagent` (`index.tsx:3339-3341`): the part completed but the
/// detached process is still running.
#[must_use]
pub fn is_background_subagent(metadata_status: Option<&str>, status: &str) -> bool {
    matches!(classify_status(status, false), ToolStatus::Completed)
        && metadata_status == Some("running")
}

/// One fully resolved tool presentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolDisplay {
    /// Canonical display name (`bash` -> `shell`, plugin/MCP names unchanged).
    pub name: String,
    pub kind: ToolKind,
    pub status: ToolStatus,
    /// Fixed icon-column glyph (status-dependent, as upstream).
    pub icon: &'static str,
    /// Primary label shown after the icon column.
    pub label: String,
    /// Body rows (already collapsed and budgeted), rendered after the label.
    pub body: Vec<ToolRow>,
    /// Error text for the `✖ <name> failed: …` row; `None` unless errored.
    pub error: Option<String>,
    /// Detached `Background` badge.
    pub background: bool,
    /// Execute output preview, retained until metadata can mark a completed
    /// transport state as a runtime error.
    pub execute_error_preview: Vec<ToolRow>,
    /// A permission decision is pending for THIS tool call
    /// (`routes/session/index.tsx:2834`): the row's foreground becomes
    /// `theme.text.feedback.warning.base` (`:2837`). Upstream adds NO badge
    /// text for this state — only the colour changes.
    pub permission_needed: bool,
}

/// A rendered row: glyph text plus its paint class.
pub type ToolRow = (String, ChatRowClass);

impl ToolDisplay {
    /// Mark this call as awaiting a permission decision
    /// (`routes/session/index.tsx:2793-2799`): upstream resolves the pending
    /// request whose `source.type == "tool"` and `source.id` equals this
    /// call's id, then paints the whole row
    /// `theme.text.feedback.warning.base` (`:2834-2847`). The caller joins the
    /// pending request to the tool id; this only records the fact.
    pub fn mark_permission_pending(&mut self) {
        self.permission_needed = true;
    }
}

/// Render a tool call whose result has already been paired with it
/// (`ChatPart::ToolState`).
#[must_use]
pub fn tool_state_rows(
    width: usize,
    name: &str,
    status: &str,
    args_json: &str,
    output: &str,
    is_error: bool,
) -> Vec<ToolRow> {
    tool_state_rows_spin(width, name, status, args_json, output, is_error, None)
}

/// [`tool_state_rows`] with the live spinner frame for busy rows (TOOL-002).
/// `spin` is the `FrameClock` animation tick (`now_ms / 80`, the upstream
/// spinner `interval={80}`, `component/spinner.tsx:34`); the reduced-motion
/// sentinel [`SPINNER_REDUCED_MOTION`] paints `⋯`; `None` keeps the static
/// icon (the non-live default). Terminal rows ignore it.
#[must_use]
pub fn tool_state_rows_spin(
    width: usize,
    name: &str,
    status: &str,
    args_json: &str,
    output: &str,
    is_error: bool,
    spin: Option<u64>,
) -> Vec<ToolRow> {
    rows(width, &tool_display(width, name, status, args_json, output, is_error), spin)
}

/// Render a call event that carries no result yet (`ChatPart::ToolCall`).
#[must_use]
pub fn tool_call_rows(width: usize, name: &str) -> Vec<ToolRow> {
    tool_call_rows_spin(width, name, None)
}

/// [`tool_call_rows`] with the live spinner frame (TOOL-002).
#[must_use]
pub fn tool_call_rows_spin(width: usize, name: &str, spin: Option<u64>) -> Vec<ToolRow> {
    tool_state_rows_spin(width, name, "running", "", "", false, spin)
}

/// Render a result event that arrived without a paired call
/// (`ChatPart::ToolResult`). The result body is shown under the header; the
/// shell specialization already budgets its own command/output pair.
#[must_use]
pub fn tool_result_rows(width: usize, name: &str, output: &str, is_error: bool) -> Vec<ToolRow> {
    let status = if is_error { "error" } else { "completed" };
    let mut display = tool_display(width, name, status, "", output, is_error);
    if display.status == ToolStatus::Completed && display.body.is_empty() {
        let clean = strip(output);
        if !clean.trim().is_empty() {
            let preview = collapse_tool_output(
                &clean,
                SHELL_BUDGET_LINES,
                SHELL_BUDGET_LINES * width.saturating_sub(6).max(20),
            )
            .0;
            for line in preview.split('\n') {
                display
                    .body
                    .push((line.to_owned(), ChatRowClass::ToolResult));
            }
        }
    }
    rows(width, &display, None)
}

/// Build the per-tool display model from caller facts only.
#[must_use]
pub fn tool_display(
    width: usize,
    name: &str,
    status: &str,
    args_json: &str,
    output: &str,
    is_error: bool,
) -> ToolDisplay {
    let canonical = canonical_tool_name(name);
    let kind = tool_kind(name);
    let args = parse_args(args_json);
    let output = strip(output);
    let mut state = classify_status(status, is_error);
    if state == ToolStatus::Error && is_denied_error(&output) {
        state = ToolStatus::Denied;
    }
    let visible_output = tool_display_content(status, Some(output.as_str())).unwrap_or("");
    let mut body: Vec<ToolRow> = Vec::new();
    let execute_error_preview = if kind == ToolKind::Execute {
        collapse_tool_output(
            output.trim(),
            EXECUTE_PREVIEW_LINES,
            EXECUTE_PREVIEW_LINES * width.saturating_sub(6).max(20),
        )
        .0
        .split('\n')
        .enumerate()
        .map(|(index, line)| {
            let prefix = if index == 0 { "↳ " } else { "  " };
            (format!("{prefix}{line}"), ChatRowClass::ToolError)
        })
        .collect()
    } else {
        Vec::new()
    };
    let mut error = None;
    if state == ToolStatus::Error {
        let message = output.trim();
        error = Some(if message.is_empty() {
            "failed".to_owned()
        } else {
            message.to_owned()
        });
    }

    let (icon, label) = match kind {
        ToolKind::Shell => shell_display(width, &args, state, visible_output, &mut body),
        ToolKind::Glob => ("✱", glob_label(&args, "Glob", "count", "match", state)),
        ToolKind::Grep => ("✱", glob_label(&args, "Grep", "matches", "match", state)),
        ToolKind::Read => ("→", read_label(&args, state)),
        ToolKind::WebFetch => {
            let url = str_field(&args, "url").unwrap_or_default();
            let label = if url.is_empty() {
                if state.is_running() { "Fetching from the web…".to_owned() } else { "WebFetch".to_owned() }
            } else {
                format!("WebFetch {url}")
            };
            ("%", label)
        }
        ToolKind::WebSearch => {
            let provider = str_field(&args, "provider");
            let title = web_search_provider_label(provider.as_deref());
            let query = str_field(&args, "query").unwrap_or_default();
            let label = if query.is_empty() {
                if state.is_running() { "Searching web…".to_owned() } else { title }
            } else {
                format!("{title} \"{query}\"")
            };
            ("◈", label)
        }
        ToolKind::Write => {
            let path = path_of(&args);
            let label = if path.is_empty() && state.is_running() {
                "Preparing write…".to_owned()
            } else {
                format!("Write {path}")
            };
            ("←", label)
        }
        ToolKind::Edit => {
            let path = path_of(&args);
            let label = if path.is_empty() && state.is_running() {
                "Preparing edit…".to_owned()
            } else {
                format!("Edit {path}")
            };
            ("←", label)
        }
        ToolKind::Subagent => subagent_display(&args, state),
        ToolKind::Execute => execute_display(state),
        ToolKind::Patch => {
            let label = patch_label(&args);
            let label = if label == "Patch" && state.is_running() {
                "Patching".to_owned()
            } else {
                label
            };
            ("%", label)
        }
        ToolKind::TodoWrite => todo_display(&args, state, &mut body),
        ToolKind::Question => question_display(&args, state, &output, &mut body),
        ToolKind::Skill => {
            let skill = str_field(&args, "name")
                .or_else(|| str_field(&args, "id"))
                .unwrap_or_default();
            let label = if skill.is_empty() && state.is_running() {
                "Loading skill…".to_owned()
            } else {
                format!("Skill \"{skill}\"")
            };
            ("→", label)
        }
        ToolKind::Generic => generic_display(canonical, &args, state),
    };

    let icon = match state {
        ToolStatus::Error => "✗",
        ToolStatus::Denied => "⊘",
        _ => icon,
    };

    ToolDisplay {
        name: canonical.to_owned(),
        kind,
        status: state,
        icon,
        label,
        body,
        error,
        background: false,
        execute_error_preview,
        permission_needed: false,
    }
}

/// Overlay the upstream `metadata` facts that the wire `status` string cannot
/// carry, matching the TUI's metadata-driven branches (TOOL-006/007/008):
///
/// - The `execute` root turns red when `metadata.error === true` **even if**
///   the transport state is `completed`
///   (`routes/session/index.tsx:3401-3411`,
///   `hasRuntimeError = metadata.error === true || state.status === "error"`);
///   upstream keys this on `toolCalls`, which only `execute` carries.
/// - A shell whose part completed while the detached process is still running
///   shows the `Background` badge (`index.tsx:2984-2990`,
///   `background = state.status === "completed" && metadata.status === "running"`).
/// - A subagent in the same detached state uses
///   `isBackgroundSubagent(metadata, status)` (`index.tsx:3339-3341`) and paints
///   the `Background` `StatusBadge` (`:3320-3325`).
///
/// Facts absent from `metadata_json` never change the display, so an ordinary
/// completed row keeps its existing rendering.
#[must_use]
pub fn apply_tool_metadata(mut display: ToolDisplay, metadata_json: &str) -> ToolDisplay {
    let Ok(Value::Object(meta)) = serde_json::from_str::<Value>(metadata_json) else {
        return display;
    };
    // `Read` prints one `↳ Loaded <path>` row per loaded instruction file, but
    // only once the part completed (`routes/session/index.tsx:3211-3243`:
    // `if state.status !== "completed" return []`), keeping only string entries.
    if display.kind == ToolKind::Read && display.status == ToolStatus::Completed {
        if let Some(loaded) = meta.get("loaded").and_then(Value::as_array) {
            let mut extra: Vec<ToolRow> = Vec::new();
            for entry in loaded.iter().filter_map(Value::as_str) {
                // Upstream nests the row under `paddingLeft={3}` + a text
                // `paddingLeft={3}`, i.e. 6 leading cells; `rows()` already adds
                // `BODY_INDENT` (4), so bake the remaining two into the text.
                let path: String = entry.chars().take(MAX_LOADED_PATH_CHARS).collect();
                extra.push((format!("  ↳ Loaded {path}"), ChatRowClass::Metadata));
                if extra.len() >= MAX_LOADED_ROWS {
                    break;
                }
            }
            display.body.extend(extra);
        }
    }
    if display.kind == ToolKind::Execute {
        if let Some(value) = meta.get("toolCalls") {
            let rows: Vec<ToolRow> = execute_calls(value)
                .iter()
                .map(|call| {
                    let (marker, class) = match call.status {
                        ExecuteCallStatus::Error => ("✗", ChatRowClass::ToolError),
                        _ => ("›", ChatRowClass::ToolResult),
                    };
                    (format!("{marker} {}", execute_call_summary(call)), class)
                })
                .collect();
            display.body.splice(0..0, rows);
        }
    }
    if display.kind == ToolKind::Execute
        && meta.get("error").and_then(Value::as_bool) == Some(true)
        && display.status != ToolStatus::Error
    {
        display.status = ToolStatus::Error;
        display.icon = "✗";
    }
    let detached = matches!(display.kind, ToolKind::Shell | ToolKind::Subagent)
        && display.status == ToolStatus::Completed
        && meta.get("status").and_then(Value::as_str) == Some("running");
    if detached {
        display.background = true;
    }
    display
}

fn parse_args(args_json: &str) -> Option<Map<String, Value>> {
    if args_json.is_empty() || args_json.len() > MAX_ARGS_BYTES {
        return None;
    }
    match serde_json::from_str::<Value>(args_json).ok()? {
        Value::Object(map) => Some(map),
        Value::String(text) => Some(Map::from_iter([("__raw".to_owned(), Value::String(text))])),
        _ => None,
    }
}

fn str_field(args: &Option<Map<String, Value>>, key: &str) -> Option<String> {
    let value = args.as_ref()?.get(key)?.as_str()?;
    if value.is_empty() {
        return None;
    }
    Some(clip(value, MAX_FIELD_BYTES))
}

fn number_field(args: &Option<Map<String, Value>>, key: &str) -> Option<u64> {
    args.as_ref()?.get(key)?.as_u64()
}

fn array_field<'a>(args: &'a Option<Map<String, Value>>, key: &str) -> Option<&'a Vec<Value>> {
    args.as_ref()?.get(key)?.as_array()
}

fn path_of(args: &Option<Map<String, Value>>) -> String {
    str_field(args, "path")
        .or_else(|| str_field(args, "filePath"))
        .unwrap_or_default()
}

fn glob_label(
    args: &Option<Map<String, Value>>,
    verb: &str,
    count_key: &str,
    noun: &str,
    state: ToolStatus,
) -> String {
    let pattern = str_field(args, "pattern").unwrap_or_default();
    if pattern.is_empty() && state.is_running() {
        return if verb == "Glob" { "Finding files…" } else { "Searching content…" }.to_owned();
    }
    let mut label = format!("{verb} \"{pattern}\"");
    if let Some(path) = str_field(args, "path") {
        label.push_str(&format!(" in {path}"));
    }
    if let Some(count) = number_field(args, count_key) {
        label.push_str(&format!(" ({count} {noun}{})", if count == 1 { "" } else { "es" }));
    }
    label
}

fn read_label(args: &Option<Map<String, Value>>, state: ToolStatus) -> String {
    let path = path_of(args);
    if path.is_empty() && state.is_running() {
        return "Reading file…".to_owned();
    }
    let mut label = format!("Read {path}");
    let offset = number_field(args, "offset");
    let limit = number_field(args, "limit");
    if offset.is_some() || limit.is_some() {
        let start = offset.unwrap_or(1);
        let end = limit.map_or_else(String::new, |limit| (start + limit - 1).to_string());
        label.push_str(&format!(":{start}-{end}"));
    }
    label
}

fn titlecase(text: &str) -> String {
    // Locale.titlecase: uppercase the first character of every word.
    let mut out = String::with_capacity(text.len());
    let mut start = true;
    for ch in text.chars() {
        if start && ch.is_alphanumeric() {
            out.extend(ch.to_uppercase());
            start = false;
        } else {
            out.push(ch);
            start = !ch.is_alphanumeric();
        }
    }
    out
}

fn subagent_display(
    args: &Option<Map<String, Value>>,
    state: ToolStatus,
) -> (&'static str, String) {
    let continuation = str_field(args, "sessionID").is_some();
    let agent = str_field(args, "agent")
        .or_else(|| str_field(args, "subagent_type"))
        .unwrap_or_else(|| "General".to_owned());
    let description = str_field(args, "description");
    if state.is_running()
        && !continuation
        && description.is_none()
        && str_field(args, "agent").is_none()
        && str_field(args, "subagent_type").is_none()
    {
        return ("│", "Delegating…".to_owned());
    }
    let icon = if continuation {
        "↳"
    } else if state == ToolStatus::Completed {
        "✓"
    } else {
        "│"
    };
    let mut label = if continuation {
        "Continue subagent".to_owned()
    } else {
        format!("{} Subagent", titlecase(&agent))
    };
    label.push_str(" — ");
    label.push_str(description.as_deref().unwrap_or("Subagent"));
    if let Some(model) = str_field(args, "model") {
        label.push_str(&format!(" · {}", subagent_model_label(&model)));
    }
    (icon, label)
}

/// `subagentModelLabel` without the model catalog: when the display name is
/// unknown upstream falls back to the raw `provider/model` reference.
#[must_use]
pub fn subagent_model_label(value: &str) -> String {
    let (reference, variant) = value.split_once('#').unwrap_or((value, ""));
    if variant.is_empty() {
        reference.to_owned()
    } else {
        format!("{reference} ({variant})")
    }
}

fn patch_label(args: &Option<Map<String, Value>>) -> String {
    let declared = array_field(args, "files").map_or(0, Vec::len);
    let targets = str_field(args, "patchText").map_or(0, |text| patch_targets(&text).len());
    let files = declared.max(targets);
    if files == 0 {
        "Patch".to_owned()
    } else {
        format!("Patch {files} file{}", if files == 1 { "" } else { "s" })
    }
}

/// `*** Add|Update|Delete File:` targets from an apply-patch payload
/// (`ApplyPatch` target scan, `index.tsx:3503-3507`).
#[must_use]
pub fn patch_targets(patch: &str) -> Vec<String> {
    patch
        .lines()
        .filter_map(|line| {
            ["*** Add File: ", "*** Update File: ", "*** Delete File: "]
                .iter()
                .find_map(|prefix| line.strip_prefix(prefix))
                .map(|target| target.trim().to_owned())
        })
        .filter(|target| !target.is_empty())
        .collect()
}

fn todo_display(
    args: &Option<Map<String, Value>>,
    state: ToolStatus,
    body: &mut Vec<ToolRow>,
) -> (&'static str, String) {
    if state.is_running() {
        return ("⚙", "Updating todos…".to_owned());
    }
    let todos = parse_todos(args);
    if state == ToolStatus::Error {
        return ("⚙", "Todo update failed".to_owned());
    }
    if todos.is_empty() {
        return ("⚙", "Cleared todos".to_owned());
    }
    for (marker, content) in &todos {
        body.push((format!("{marker} {content}"), ChatRowClass::ToolResult));
    }
    ("#", "# Todos".to_owned())
}

fn parse_todos(args: &Option<Map<String, Value>>) -> Vec<(&'static str, String)> {
    array_field(args, "todos")
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let item = item.as_object()?;
                    let content = item.get("content")?.as_str().filter(|text| !text.is_empty())?;
                    let marker = match item.get("status").and_then(Value::as_str) {
                        Some("completed") => "[✓]",
                        Some("in_progress") => "[•]",
                        Some("cancelled") => "[-]",
                        _ => "[ ]",
                    };
                    Some((marker, clip(content, MAX_FIELD_BYTES)))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn question_display(
    args: &Option<Map<String, Value>>,
    state: ToolStatus,
    output: &str,
    body: &mut Vec<ToolRow>,
) -> (&'static str, String) {
    let questions: Vec<String> = array_field(args, "questions")
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_object()?.get("question")?.as_str())
                .map(|question| clip(question, MAX_FIELD_BYTES))
                .collect()
        })
        .unwrap_or_default();
    let answers = question_answers(output);
    for (index, question) in questions.iter().enumerate() {
        body.push((question.clone(), ChatRowClass::ToolResult));
        let answer = answers
            .as_ref()
            .and_then(|answers| answers.get(index))
            .filter(|entries| !entries.is_empty())
            .map_or_else(|| "(no answer)".to_owned(), |entries| entries.join(", "));
        body.push((format!("  {answer}"), ChatRowClass::ToolResult));
    }
    let count = questions.len();
    let label = if count == 0 && state.is_running() {
        "Asking questions…".to_owned()
    } else {
        format!("Asked {count} question{}", if count == 1 { "" } else { "s" })
    };
    ("→", label)
}

/// Answers from the completed question tool payload (`metadata.answers`,
/// `index.tsx:3662-3675`); absent/foreign payloads yield no answer block.
fn question_answers(output: &str) -> Option<Vec<Vec<String>>> {
    let value: Value = serde_json::from_str(output).ok()?;
    let answers = value.get("answers")?.as_array()?;
    Some(
        answers
            .iter()
            .map(|answer| {
                answer
                    .as_array()
                    .map(|entries| {
                        entries
                            .iter()
                            .filter_map(Value::as_str)
                            .map(|entry| clip(entry, MAX_FIELD_BYTES))
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect(),
    )
}

fn generic_display(
    canonical: &str,
    args: &Option<Map<String, Value>>,
    state: ToolStatus,
) -> (&'static str, String) {
    let icon = match state {
        ToolStatus::Error => "✗",
        ToolStatus::Completed => "✓",
        _ => "⚙",
    };
    let raw = args
        .as_ref()
        .and_then(|args| args.get("__raw"))
        .and_then(Value::as_str)
        .map(|text| clip(text, MAX_FIELD_BYTES))
        .or_else(|| {
            args.as_ref()
                .map(|args| primitive_input_summary(args, &[]))
                .filter(|summary| !summary.is_empty())
        })
        .unwrap_or_default();
    let raw = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if raw.is_empty() {
        (icon, canonical.to_owned())
    } else {
        (icon, format!("{canonical} {raw}"))
    }
}

fn execute_display(state: ToolStatus) -> (&'static str, String) {
    let icon = match state {
        ToolStatus::Completed => "✓",
        ToolStatus::Error => "✗",
        _ => "│",
    };
    (icon, "execute".to_owned())
}

fn shell_display(
    width: usize,
    args: &Option<Map<String, Value>>,
    state: ToolStatus,
    output: &str,
    body: &mut Vec<ToolRow>,
) -> (&'static str, String) {
    let command = str_field(args, "command").unwrap_or_default();
    let workdir = str_field(args, "workdir").unwrap_or_default();
    if command.is_empty() {
        return ("$", "Writing command…".to_owned());
    }
    let prefix = if workdir.is_empty() || workdir == "." {
        String::new()
    } else {
        format!("cd {workdir} && ")
    };
    let input = format!("{prefix}{command}");
    let running_extra = usize::from(state.is_running()) * 2;
    let max_chars = SHELL_BUDGET_LINES * width.saturating_sub(6 + running_extra).max(20);
    let (label, collapsed_output, _) =
        collapse_shell_output(&input, output, SHELL_BUDGET_LINES, max_chars);
    if state != ToolStatus::Error && !collapsed_output.is_empty() {
        for line in collapsed_output.split('\n') {
            body.push((line.to_owned(), ChatRowClass::ToolResult));
        }
    }
    ("$", label)
}

/// Port of `collapseToolOutput` (`collapse-tool-output.ts:4-24`). Returns the
/// head preview and whether anything was dropped.
///
/// The budget unit is the **code point** (`Array.from(output).length`,
/// `Array.from(preview).slice(0, maxChars - 1)` at
/// `collapse-tool-output.ts:6,13,16-18`) — never display cells or grapheme
/// clusters — so the preview is exactly `max_chars - 1` code points plus `…`.
#[must_use]
pub fn collapse_tool_output(output: &str, max_lines: usize, max_chars: usize) -> (String, bool) {
    let lines: Vec<&str> = output.split('\n').collect();
    if lines.len() <= max_lines && output.chars().count() <= max_chars {
        return (output.to_owned(), false);
    }
    let mut visible: Vec<String> = lines.iter().take(max_lines).map(|line| (*line).to_owned()).collect();
    if lines.len() > max_lines {
        if let Some(last) = visible.last_mut() {
            last.push('…');
        }
    }
    let preview = visible.join("\n");
    if preview.chars().count() > max_chars {
        let head: String = preview.chars().take(max_chars.saturating_sub(1)).collect();
        return (format!("{head}…"), true);
    }
    (preview, true)
}

/// Port of `collapseTail` (`collapse-tool-output.ts:89-103`): the last lines
/// with a `(N earlier lines)` label as the truncation indicator.
#[must_use]
pub fn collapse_tail(output: &str, max_lines: usize, max_chars: usize) -> String {
    let lines: Vec<&str> = output.split('\n').collect();
    if lines.len() <= max_lines && cells(output) <= max_chars {
        return output.to_owned();
    }
    let count = lines.len().saturating_sub(max_lines.saturating_sub(1)).max(1);
    let label = format!("({count} earlier line{})", if count == 1 { "" } else { "s" });
    if max_lines <= 1 {
        return label;
    }
    let preview = {
        let keep = (max_lines - 1).min(lines.len());
        lines[lines.len() - keep..].join("\n")
    };
    let available = max_chars.saturating_sub(cells(&label) + 1);
    if available == 0 {
        return label;
    }
    format!("{label}\n{}", take_last_cells(&preview, available))
}

/// Port of `collapseShellCommand` (`collapse-tool-output.ts:51-87`): at most
/// `max_lines` visual rows within `line_width` cells, `…` when clipped.
///
/// The unit of iteration is the **grapheme cluster** (`Locale.graphemes(input)`
/// with `stringWidth(segment)` at `collapse-tool-output.ts:55,64`) — never the
/// code point — so combining sequences, flag pairs and joined emoji stay whole
/// across row wraps, and the overflow pop removes exactly one cluster
/// (`visible.pop()` at `collapse-tool-output.ts:76-79`).
#[must_use]
pub fn collapse_shell_command(input: &str, max_lines: usize, line_width: usize) -> (String, bool, usize) {
    let mut visible: Vec<&str> = Vec::new();
    let mut lines = 1usize;
    let mut width = 0usize;
    let mut overflow = false;
    for segment in input.graphemes(true) {
        if segment == "\n" {
            if lines >= max_lines {
                overflow = true;
                break;
            }
            visible.push(segment);
            lines += 1;
            width = 0;
            continue;
        }
        let next = cells(segment);
        if width + next > line_width {
            if lines >= max_lines {
                overflow = true;
                break;
            }
            lines += 1;
            width = 0;
        }
        visible.push(segment);
        width += next;
    }
    if !overflow {
        return (input.to_owned(), false, lines);
    }
    if width >= line_width {
        // Drop the cluster that filled the row so the ellipsis replaces it.
        // Upstream then does `width -= stringWidth(removed)`; `width` is not
        // returned, so that subtraction is inoperative and omitted here.
        visible.pop();
    }
    (format!("{}…", visible.concat()), true, lines)
}

/// Port of `collapseShellOutput` (`collapse-tool-output.ts:26-51`): command and
/// output share one 10-row budget; an overflowing output keeps its tail behind
/// the earlier-line label.
#[must_use]
pub fn collapse_shell_output(
    input: &str,
    output: &str,
    max_lines: usize,
    max_chars: usize,
) -> (String, String, bool) {
    if input.is_empty() {
        let (_, overflow) = collapse_tool_output(output, max_lines, max_chars);
        let tail = if overflow {
            collapse_tail(output, max_lines, max_chars)
        } else {
            output.to_owned()
        };
        return (String::new(), tail, overflow);
    }
    let command_lines = SHELL_COMMAND_LINES.min(max_lines);
    let line_chars = (max_chars / max_lines.max(1)).max(1);
    let (command, command_overflow, used_lines) = collapse_shell_command(input, command_lines, line_chars);
    if output.is_empty() {
        return (command, String::new(), command_overflow);
    }
    let lines = max_lines.saturating_sub(used_lines).saturating_sub(1).max(1);
    // Upstream subtracts `Array.from(command.output).length` code points
    // (`collapse-tool-output.ts:42`) — the shared budget is counted in code
    // points, not display cells.
    let chars = max_chars.saturating_sub(command.chars().count() + 2).max(1);
    let (_, overflow) = collapse_tool_output(output, lines, chars);
    let body = if overflow {
        collapse_tail(output, lines, chars)
    } else {
        output.to_owned()
    };
    (command, body, command_overflow || overflow)
}

// ---------------------------------------------------------------------------
// Execute dialog model (`dialog-execute.tsx`)
// ---------------------------------------------------------------------------

/// Pure model of the execute dialog (`DialogExecute`), split into the shared
/// gutter, code block, and output block so a dialog renderer only lays it out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecuteDialog {
    /// `Receiving code…` / `Running` / `Completed · 1.2s` / `Failed · 1.2s`.
    pub status: String,
    pub failed: bool,
    /// `state.input.code` when present.
    pub code: String,
    /// Output text (error message on failure, content when completed).
    pub output: Option<String>,
    /// Leading JSON object/array from the output (syntax highlighted as JSON).
    pub output_json: Option<String>,
    /// Trailing `Warnings:`/`Logs:` block after the JSON.
    pub output_rest: Option<String>,
    /// Shared gutter width across code and output blocks.
    pub digits: usize,
}

/// Build the execute dialog model. `elapsed_ms` is the caller's real duration
/// when known; no duration is invented.
#[must_use]
pub fn execute_dialog(
    status: &str,
    args_json: &str,
    output: &str,
    is_error: bool,
    elapsed_ms: Option<u64>,
) -> ExecuteDialog {
    let args = parse_args(args_json);
    let state = classify_status(status, is_error);
    let code = str_field(&args, "code").unwrap_or_default();
    let text = if state == ToolStatus::Error {
        let message = strip(output).trim().to_owned();
        (!message.is_empty()).then_some(message)
    } else if state == ToolStatus::Completed {
        let message = strip(output).trim().to_owned();
        (!message.is_empty()).then_some(message)
    } else {
        None
    };
    let (output_json, output_rest) = text
        .as_deref()
        .and_then(highlighted_output)
        .map_or((None, None), |(json, rest)| (Some(json), rest));
    let digits = digits_for(line_count(Some(code.as_str())).max(line_count(output_json.as_deref())).max(1));
    let failed = state == ToolStatus::Error;
    let status = match state {
        ToolStatus::Streaming => "Receiving code…".to_owned(),
        ToolStatus::Running | ToolStatus::Pending => "Running".to_owned(),
        _ => {
            let duration = elapsed_ms
                .filter(|ms| *ms > 0)
                .map(|ms| format!(" · {}", duration_text(ms)))
                .unwrap_or_default();
            format!("{}{duration}", if failed { "Failed" } else { "Completed" })
        }
    };
    ExecuteDialog {
        status,
        failed,
        code,
        output: text,
        output_json,
        output_rest,
        digits,
    }
}

/// Split the tool's JSON result from an optional `Warnings:`/`Logs:` tail
/// (`highlightedOutput`, `dialog-execute.tsx:222-230`).
#[must_use]
pub fn highlighted_output(text: &str) -> Option<(String, Option<String>)> {
    if !text.starts_with('{') && !text.starts_with('[') {
        return None;
    }
    let end = ["\n\nWarnings:\n", "\n\nLogs:\n"]
        .iter()
        .filter_map(|marker| text.find(marker))
        .min();
    let json = end.map_or(text, |end| &text[..end]);
    serde_json::from_str::<Value>(json).ok()?;
    let rest = text[json.len()..].trim();
    Some((json.to_owned(), (!rest.is_empty()).then(|| rest.to_owned())))
}

/// Gutter column (`GutteredCode`, `dialog-execute.tsx:251-282`): one number per
/// source line, continuation rows of a wrapped line stay blank, every row is
/// exactly `digits` cells wide.
#[must_use]
pub fn execute_dialog_gutter(text: &str, width: usize, digits: usize) -> Vec<String> {
    let digits = digits.max(1);
    let width = width.max(1);
    let mut rows = Vec::new();
    for (index, line) in text.split('\n').enumerate() {
        let wrapped = cells(line).div_ceil(width).max(1);
        rows.push(format!("{:>digits$}", index + 1));
        for _ in 1..wrapped {
            rows.push(" ".repeat(digits));
        }
    }
    rows
}

fn line_count(text: Option<&str>) -> usize {
    text.map_or(0, |text| text.split('\n').count())
}

fn digits_for(value: usize) -> usize {
    value.max(1).to_string().len()
}

fn duration_text(ms: u64) -> String {
    if ms < 1_000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else if ms < 3_600_000 {
        format!("{}m {}s", ms / 60_000, (ms % 60_000) / 1000)
    } else {
        format!("{}h {}m", ms / 3_600_000, (ms % 3_600_000) / 60_000)
    }
}

// ---------------------------------------------------------------------------
// Row emission
// ---------------------------------------------------------------------------

/// Render a fully-resolved [`ToolDisplay`] (used by the live transcript once
/// caller metadata has been folded in via [`apply_tool_metadata`]).
#[must_use]
pub fn rows_for_display(width: usize, display: &ToolDisplay) -> Vec<ToolRow> {
    rows_for_display_spin(width, display, None)
}

/// [`rows_for_display`] with the live spinner frame for busy rows (TOOL-002).
#[must_use]
pub fn rows_for_display_spin(width: usize, display: &ToolDisplay, spin: Option<u64>) -> Vec<ToolRow> {
    rows(width, display, spin)
}

fn rows(width: usize, display: &ToolDisplay, spin: Option<u64>) -> Vec<ToolRow> {
    // Upstream `fg` memo (`routes/session/index.tsx:2837-2847`) checks the
    // pending-permission state BEFORE the runtime-failure state, so a
    // permission-pending call paints its whole row `text.feedback.warning`
    // even while an inner error is present.
    let class = if display.permission_needed {
        ChatRowClass::ToolPermission
    } else {
        display.status.row_class()
    };
    // TOOL-002 (`message-parts.tsx:223-234`): while the row streams or runs
    // upstream swaps the icon column for the animated `spinner-frames.ts`
    // glyph (`Match when={props.spinner}` renders `<Spinner>` instead of the
    // static icon). `spin` is the live `FrameClock` tick (`now_ms / 80`, the
    // upstream spinner `interval={80}`, `component/spinner.tsx:34`). Non-live
    // callers pass `None` and keep the static icon byte-identical; the
    // reduced-motion tick [`SPINNER_REDUCED_MOTION`] paints the `⋯` fallback
    // (`component/spinner.tsx:20`). Terminal rows always keep the icon.
    let glyph: String = if display.status.is_running() {
        match spin {
            Some(SPINNER_REDUCED_MOTION) => "\u{22ef}".to_owned(),
            Some(tick) => crate::native_box_style::BRAILLE_FRAMES
                [(tick % crate::native_box_style::BRAILLE_FRAMES.len() as u64) as usize]
                .to_string(),
            None => display.icon.to_owned(),
        }
    } else {
        display.icon.to_owned()
    };
    let header_prefix = header_prefix(&glyph);
    let mut out = Vec::new();
    push_wrapped(&mut out, width, &display.label, &header_prefix, class);
    // `StatusBadge` (`routes/session/index.tsx:2875-2883`) is a separate,
    // non-shrinking flex item in the inline tool row: `" Background "` stays a
    // single padded token (one gap + the badge's own `" " + text + " "`), so it
    // wraps to its own row instead of splitting mid-word. Upstream paints it
    // for a detached shell block (`:3149-3151`) and a detached subagent
    // (`:3317`).
    if display.background {
        push_badge(&mut out, width, &header_prefix, "Background", class);
    }
    if display.status == ToolStatus::Error {
        if display.kind == ToolKind::Execute {
            // Child calls retain their own lifecycle class. Only output uses
            // the bounded runtime-error preview; never synthesize a failure.
            for (line, body_class) in display.body.iter().chain(&display.execute_error_preview) {
                push_wrapped(&mut out, width, line, BODY_INDENT, *body_class);
                if out.len() >= MAX_TOOL_ROWS {
                    break;
                }
            }
        } else if let Some(error) = &display.error {
            let line = format!("✖ {} failed: {error}", display.name);
            push_wrapped(&mut out, width, &line, BODY_INDENT, ChatRowClass::ToolError);
        }
    } else {
        for (line, body_class) in &display.body {
            push_wrapped(&mut out, width, line, BODY_INDENT, *body_class);
            if out.len() >= MAX_TOOL_ROWS {
                break;
            }
        }
    }
    out.truncate(MAX_TOOL_ROWS);
    out
}

/// `  ` + the icon in a fixed two-cell column (`INLINE_TOOL_ICON_WIDTH = 2`).
fn header_prefix(icon: &str) -> String {
    let used = cells(icon);
    let pad = ICON_COLUMN_CELLS.saturating_sub(used);
    format!("{ROW_INDENT}{icon}{}", " ".repeat(pad))
}

/// Append one `StatusBadge` as an atomic padded token (`"  Background "`): the
/// one-cell flex gap plus the badge's own leading/trailing spaces
/// (`routes/session/index.tsx:2875-2883`). It joins the previous row when it
/// fits there, otherwise it starts a new row — never split across cells.
fn push_badge(rows_out: &mut Vec<ToolRow>, width: usize, prefix: &str, label: &str, class: ChatRowClass) {
    if rows_out.len() >= MAX_TOOL_ROWS {
        return;
    }
    let token = format!("  {label} ");
    if let Some((last, _)) = rows_out.last_mut() {
        if width > 0 && cells(last).saturating_add(cells(&token)) <= width {
            last.push_str(&token);
            return;
        }
    }
    rows_out.push((truncate_cells(&format!("{prefix}{token}"), width), class));
}

fn push_wrapped(rows_out: &mut Vec<ToolRow>, width: usize, text: &str, prefix: &str, class: ChatRowClass) {
    if rows_out.len() >= MAX_TOOL_ROWS {
        return;
    }
    let clean = strip(text);
    let prefix_cells = cells(prefix);
    let budget = if width == 0 {
        0
    } else {
        width.saturating_sub(prefix_cells).max(1)
    };
    for raw in clean.split('\n') {
        let raw = raw.replace('\t', " ");
        if budget == 0 {
            rows_out.push((format!("{prefix}{raw}"), class));
        } else {
            for chunk in wrap_cells(&raw, budget) {
                rows_out.push((truncate_cells(&format!("{prefix}{chunk}"), width), class));
            }
        }
        if rows_out.len() >= MAX_TOOL_ROWS {
            return;
        }
    }
}

fn wrap_cells(text: &str, budget: usize) -> Vec<String> {
    if budget == 0 || text.is_empty() {
        return vec![text.to_owned()];
    }
    let mut out = Vec::new();
    let mut chunk = String::new();
    let mut used = 0usize;
    let mut visit = |glyph: &str, width: usize| {
        if used > 0 && used + width > budget {
            out.push(std::mem::take(&mut chunk));
            used = 0;
        }
        chunk.push_str(glyph);
        used += width;
    };
    visit_unpadded_cells(text, MAX_CELLS, &mut visit);
    if !chunk.is_empty() {
        out.push(chunk);
    } else if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn truncate_cells(text: &str, width: usize) -> String {
    if width == 0 {
        return text.to_owned();
    }
    let mut out = String::new();
    visit_unpadded_cells(text, width, |glyph, _| out.push_str(glyph));
    out
}

fn take_last_cells(text: &str, budget: usize) -> String {
    let mut clusters: Vec<(String, usize)> = Vec::new();
    visit_unpadded_cells(text, MAX_CELLS, |glyph, width| clusters.push((glyph.to_owned(), width)));
    let mut used = 0usize;
    let mut keep_from = clusters.len();
    for (index, (_, width)) in clusters.iter().enumerate().rev() {
        if used + width > budget {
            break;
        }
        used += width;
        keep_from = index;
    }
    clusters[keep_from..].iter().map(|(glyph, _)| glyph.as_str()).collect()
}

fn cells(text: &str) -> usize {
    visit_unpadded_cells(text, MAX_CELLS, |_, _| {})
}

fn strip(text: &str) -> String {
    crate::native_transcript::strip_ansi(&clip(text, MAX_ARGS_BYTES))
}

fn clip(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn label(name: &str, status: &str, args: Value) -> ToolDisplay {
        tool_display(80, name, status, &args.to_string(), "", false)
    }

    #[test]
    fn canonical_names_map_legacy_aliases() {
        assert_eq!(canonical_tool_name("bash"), "shell");
        assert_eq!(canonical_tool_name("task"), "subagent");
        assert_eq!(canonical_tool_name("apply_patch"), "patch");
        assert_eq!(canonical_tool_name("shell"), "shell");
        assert_eq!(canonical_tool_name("mcp__fixture"), "mcp__fixture");
        assert_eq!(tool_kind("bash"), ToolKind::Shell);
        assert_eq!(tool_kind("task"), ToolKind::Subagent);
        assert_eq!(tool_kind("apply_patch"), ToolKind::Patch);
    }

    #[test]
    fn all_fourteen_specializations_dispatch() {
        let cases = [
            ("shell", ToolKind::Shell, "$"),
            ("glob", ToolKind::Glob, "✱"),
            ("read", ToolKind::Read, "→"),
            ("grep", ToolKind::Grep, "✱"),
            ("webfetch", ToolKind::WebFetch, "%"),
            ("websearch", ToolKind::WebSearch, "◈"),
            ("write", ToolKind::Write, "←"),
            ("edit", ToolKind::Edit, "←"),
            ("subagent", ToolKind::Subagent, "✓"),
            ("execute", ToolKind::Execute, "✓"),
            ("patch", ToolKind::Patch, "%"),
            ("todowrite", ToolKind::TodoWrite, "⚙"),
            ("question", ToolKind::Question, "→"),
            ("skill", ToolKind::Skill, "→"),
        ];
        for (name, kind, icon) in cases {
            let display = label(name, "completed", json!({}));
            assert_eq!(display.kind, kind, "{name}");
            assert_eq!(display.icon, icon, "{name}");
            assert!(!display.label.is_empty(), "{name}");
        }
    }

    #[test]
    fn per_tool_labels_use_real_arguments() {
        let glob = label("glob", "completed", json!({"pattern": "*.rs", "path": "src", "count": 2}));
        assert_eq!(glob.label, "Glob \"*.rs\" in src (2 matches)");
        let grep = label("grep", "completed", json!({"pattern": "fn main", "matches": 1}));
        assert_eq!(grep.label, "Grep \"fn main\" (1 match)");
        let read = label("read", "completed", json!({"path": "src/main.rs", "offset": 5, "limit": 5}));
        assert_eq!(read.label, "Read src/main.rs:5-9");
        let read_open = label("read", "completed", json!({"path": "a", "offset": 7}));
        assert_eq!(read_open.label, "Read a:7-");
        let fetch = label("webfetch", "completed", json!({"url": "https://example.com"}));
        assert_eq!(fetch.label, "WebFetch https://example.com");
        let search = label("websearch", "completed", json!({"query": "rust", "provider": "opencode"}));
        assert_eq!(search.label, "Web Search via OpenCode \"rust\"");
        let search_default = label("websearch", "completed", json!({"query": "rust"}));
        assert_eq!(search_default.label, "Web Search \"rust\"");
        let write = label("write", "completed", json!({"filePath": "out.txt"}));
        assert_eq!(write.label, "Write out.txt");
        let edit = label("edit", "completed", json!({"path": "a.rs"}));
        assert_eq!(edit.label, "Edit a.rs");
        let skill = label("skill", "completed", json!({"name": "pdf"}));
        assert_eq!(skill.label, "Skill \"pdf\"");
        let question = label("question", "completed", json!({"questions": [{"question": "q1"}, {"question": "q2"}]}));
        assert_eq!(question.label, "Asked 2 questions");
    }

    #[test]
    fn shell_command_and_workdir() {
        let display = label("bash", "completed", json!({"command": "ls", "workdir": "/tmp"}));
        assert_eq!(display.name, "shell");
        assert_eq!(display.label, "cd /tmp && ls");
        let dot = label("shell", "completed", json!({"command": "ls", "workdir": "."}));
        assert_eq!(dot.label, "ls");
        let pending = label("shell", "running", json!({}));
        assert_eq!(pending.label, "Writing command…");
        assert_eq!(pending.status, ToolStatus::Running);
    }

    #[test]
    fn shell_budget_is_ten_lines_with_truncation_indicator() {
        let output: String = (1..=30).map(|n| format!("line {n}\n")).collect();
        let display = tool_display(80, "bash", "completed", r#"{"command":"echo hi"}"#, &output, false);
        let rows = rows(80, &display, None);
        assert!(rows.len() <= SHELL_BUDGET_LINES, "rows: {}", rows.len());
        assert!(rows[0].0.contains("echo hi"));
        let body = &rows[1..];
        assert!(body.iter().any(|(row, _)| row.contains("earlier line")), "{body:?}");
        assert!(body.iter().any(|(row, _)| row.trim_end().ends_with("line 30")));
        assert!(body.iter().all(|(_, class)| *class == ChatRowClass::ToolResult));
    }

    #[test]
    fn shell_command_collapses_to_two_rows() {
        let long = "x".repeat(500);
        let (command, overflow, lines) = collapse_shell_command(&long, SHELL_COMMAND_LINES, 20);
        assert!(overflow);
        assert_eq!(lines, 2);
        assert!(command.ends_with('…'));
        assert!(cells(&command) <= 2 * 20 + 1);
    }

    #[test]
    fn shell_output_tail_keeps_label_and_last_lines() {
        let output = "a\nb\nc\nd\ne\nf";
        let tail = collapse_tail(output, 4, 200);
        assert!(tail.starts_with("(3 earlier lines)"));
        assert!(tail.ends_with("f"));
        assert!(!tail.contains("\nd\n"));
    }

    #[test]
    fn wide_glyph_budget_counts_code_points_like_upstream() {
        // Contract supersession of the prior `wide_glyph_lines_stay_within_cell_budget`
        // (git history preserves it): upstream `collapse-tool-output.ts:13-20`
        // slices `Array.from(preview)` — code points — so 20 budgeted units of
        // wide glyphs become 19 code points + `…` (39 display cells). The old
        // `<= 21 cells` assertion pinned a display-cell budget that upstream
        // does not implement.
        let text = "日本語のテキスト".repeat(10);
        let (preview, overflow) = collapse_tool_output(&text, 2, 20);
        assert!(overflow);
        assert!(preview.ends_with('…'));
        assert_eq!(preview.chars().count(), 20);
        assert_eq!(preview.chars().take(3).collect::<String>(), "日本語");
    }

    // TOOL-005: `collapseToolOutput` budgets on code points (`Array.from`,
    // `collapse-tool-output.ts:6,13,16-18`). The pre-fix port measured display
    // `cells()`: `中中中` counted 6 > 5 and collapsed to `中中…`, and
    // `中中中中` truncated to `中…`; upstream sees 3 <= 5 code points (no
    // overflow) and slices exactly `maxChars - 1` = 2 code points + `…`.
    // These assertions FAIL on the cell-budget source and PASS on the fix.
    #[test]
    fn tool_output_budget_counts_code_points_not_display_cells() {
        assert_eq!(collapse_tool_output("中中中", 1, 5), ("中中中".to_owned(), false));
        assert_eq!(collapse_tool_output("中中中中", 1, 3), ("中中…".to_owned(), true));
    }

    // TOOL-005: `collapseShellCommand` iterates grapheme clusters
    // (`Locale.graphemes`, `collapse-tool-output.ts:55-73`) and pops exactly
    // one whole cluster when the row is full (`collapse-tool-output.ts:76-79`).
    // With `lineWidth = 2` the third `e + U+0301` cluster overflows; upstream
    // pops that cluster and renders `é…`. The pre-fix code-point loop popped
    // the trailing combining mark instead and kept its base, rendering
    // `e + U+0301 + e + …` (two base letters, cluster split). This assertion
    // FAILS on the code-point source and PASSES on the fix.
    #[test]
    fn shell_command_pops_grapheme_clusters_not_code_points() {
        let (command, overflow, lines) = collapse_shell_command("e\u{301}e\u{301}e\u{301}", 1, 2);
        assert!(overflow);
        assert_eq!(lines, 1);
        assert_eq!(command, "e\u{301}…");
    }

    #[test]
    fn unknown_tools_fall_back_to_generic_summary() {
        let display = tool_display(
            80,
            "mcp__fixture",
            "completed",
            r#"{"alpha":"beta","count":3,"nested":{"deep":true}}"#,
            "",
            false,
        );
        assert_eq!(display.kind, ToolKind::Generic);
        assert_eq!(display.label, "mcp__fixture [alpha=beta, count=3]");
        assert!(!display.label.contains("nested"));
        assert!(!display.label.contains('{'));
        let plugin = label("unknown_plugin", "running", json!({"flag": true}));
        assert_eq!(plugin.kind, ToolKind::Generic);
        assert_eq!(plugin.icon, "⚙");
        assert_eq!(plugin.label, "unknown_plugin [flag=true]");
    }

    #[test]
    fn error_rows_are_error_classed() {
        let rows = tool_state_rows(80, "bash", "completed", r#"{"command":"false"}"#, "boom", true);
        assert_eq!(rows[0].1, ChatRowClass::ToolError);
        assert!(rows[0].0.contains('✗'));
        assert!(rows.iter().any(|(row, class)| *class == ChatRowClass::ToolError
            && row.contains("✖ shell failed: boom")));
        assert_eq!(rows[0].0.trim_start().chars().next(), Some('✗'));
    }

    #[test]
    fn denied_errors_are_muted_not_red() {
        let rows = tool_state_rows(
            80,
            "write",
            "error",
            "{}",
            "rejected permission for this path",
            true,
        );
        assert!(rows.iter().all(|(_, class)| *class != ChatRowClass::ToolError));
        assert_eq!(rows[0].1, ChatRowClass::ToolDenied);
        assert!(rows[0].0.contains('⊘'));
        assert!(!rows.iter().any(|(row, _)| row.contains("failed")));
    }

    #[test]
    fn subagent_labels_cover_continuation_model_and_background() {
        let new = label(
            "task",
            "running",
            json!({"agent": "general", "description": "inspect logs"}),
        );
        assert_eq!(new.label, "General Subagent — inspect logs");
        let continued = label("subagent", "running", json!({"sessionID": "s1", "description": "keep going"}));
        assert_eq!(continued.label, "Continue subagent — keep going");
        assert_eq!(continued.icon, "↳");
        let model = label(
            "subagent",
            "completed",
            json!({"agent": "build", "description": "x", "model": "openai/gpt-5#high"}),
        );
        assert_eq!(model.label, "Build Subagent — x · openai/gpt-5 (high)");
        let background = label(
            "subagent",
            "completed",
            json!({"agent": "build", "description": "x", "background": true}),
        );
        // The label itself never carries the badge; `rows()` emits the
        // atomic `StatusBadge` token instead (`index.tsx:2875-2883,3317`).
        // TOOL-011: the detached badge comes from `metadata.status === "running"`
        // (`index.tsx:3343`), never the shell/subagent argument `background` flag.
        assert!(!background.label.ends_with(" Background "));
        assert!(!background.background, "argument flag must not select detached mode");
        let detached = apply_tool_metadata(background, r#"{"status":"running"}"#);
        assert!(detached.background, "metadata.status=running selects the Background badge");
        assert!(is_background_subagent(Some("running"), "completed"));
        assert!(!is_background_subagent(Some("running"), "running"));
        assert!(!is_background_subagent(None, "completed"));
    }

    /// TOOL-003: the `Background` `StatusBadge` is a non-shrinking sibling of
    /// the inline tool row (`routes/session/index.tsx:2875-2883,3149-3151,
    /// 3317`): it paints as one padded token (`"  Background "`), joining the
    /// label row when it fits and otherwise wrapping whole to its own row —
    /// never split mid-word.
    #[test]
    fn background_badge_wraps_atomically_never_split_mid_word() {
        let mut background = label(
            "subagent",
            "completed",
            json!({"agent": "build", "description": "inspect logs"}),
        );
        background.background = true;

        // Wide: the badge joins the label row.
        let wide = rows_for_display(80, &background);
        assert_eq!(wide.len(), 1, "wide badge joins the label row: {wide:?}");
        assert!(wide[0].0.contains("  Background "), "intact token: {wide:?}");

        // Narrower than label+badge but wider than prefix+badge: the token
        // wraps whole to its own row, still intact.
        let narrow = rows_for_display(26, &background);
        let badge_rows: Vec<&String> = narrow
            .iter()
            .map(|(row, _)| row)
            .filter(|row| row.contains("Background"))
            .collect();
        assert_eq!(badge_rows.len(), 1, "badge painted exactly once: {narrow:?}");
        assert!(
            badge_rows[0].contains("  Background "),
            "token stays intact on its own row: {narrow:?}"
        );
        assert!(
            narrow
                .iter()
                .all(|(row, _)| !row.ends_with("Backgrou") && !row.ends_with(" Backgroun")),
            "no row splits the badge mid-word: {narrow:?}"
        );
    }

    #[test]
    fn execute_children_and_error_preview() {
        let args = json!({"code": "return 1"}).to_string();
        let tool_calls = json!([
            {"tool": "read", "status": "running", "input": {"path": "a.rs"}},
            {"tool": "grep", "status": "error", "input": {"pattern": "x"}},
            {"tool": "bad"}
        ]);
        let calls = execute_calls(&tool_calls);
        assert_eq!(calls.len(), 2);
        assert_eq!(execute_call_summary(&calls[0]), "read [path=a.rs]");
        let display = apply_tool_metadata(
            tool_display(80, "execute", "running", &args, "", false),
            &json!({"toolCalls": tool_calls}).to_string(),
        );
        let rows = rows_for_display(80, &display);
        assert!(rows.iter().any(|(row, class)| *class == ChatRowClass::ToolResult
            && row.contains("› read [path=a.rs]")));
        assert!(rows.iter().any(|(row, class)| *class == ChatRowClass::ToolError
            && row.contains("✗ grep [pattern=x]")));
        let output: String = (1..=10).map(|n| format!("error {n}\n")).collect();
        let failed = tool_state_rows(80, "execute", "error", "{}", &output, true);
        let preview: Vec<&String> = failed
            .iter()
            .filter(|(row, _)| row.contains("error "))
            .map(|(row, _)| row)
            .collect();
        assert!(preview.len() <= EXECUTE_PREVIEW_LINES, "{preview:?}");
        assert!(preview[0].contains("↳ error 1"));
    }

    #[test]
    fn execute_success_output_stays_out_of_preview() {
        let rows = tool_state_rows(80, "execute", "completed", r#"{"code":"1"}"#, "printed output", false);
        assert!(!rows.iter().any(|(row, _)| row.contains("printed output")));
    }

    // TOOL-007: error/cancelled subagent states classify correctly and no
    // lifecycle state invents percentage progress
    // (`routes/session/index.tsx:2826-2867`; subagent rows never carry a %).
    #[test]
    fn subagent_error_cancelled_states_and_no_invented_percentage() {
        assert_eq!(classify_status("failed", false), ToolStatus::Error);
        assert_eq!(ToolStatus::Error.row_class(), ChatRowClass::ToolError);
        // `cancelled` is never claimed as success or running.
        assert_eq!(classify_status("cancelled", false), ToolStatus::Pending);
        assert!(!ToolStatus::Pending.is_running());
        let failed = tool_state_rows(
            80,
            "subagent",
            "error",
            &json!({"agent": "build", "description": "x"}).to_string(),
            "child failed",
            true,
        );
        assert!(
            failed.iter().any(|(_, class)| *class == ChatRowClass::ToolError),
            "{failed:?}"
        );
        for state in ["running", "completed", "error", "cancelled"] {
            let display = label(
                "subagent",
                state,
                json!({"agent": "build", "description": "x", "sessionID": "s1"}),
            );
            assert!(!display.label.contains('%'), "{}", display.label);
            let rows = rows_for_display(80, &display);
            assert!(
                rows.iter().all(|(row, _)| !row.contains('%')),
                "no invented percentage in {state}: {rows:?}"
            );
        }
    }

    // TOOL-008: a missing model override adds NO model label; an explicit
    // override shows the provider/model reference with its variant suffix
    // (`subagentModelLabel`, `index.tsx` subagent tool rows).
    #[test]
    fn subagent_missing_model_override_adds_no_model_label() {
        let plain = label("subagent", "completed", json!({"agent": "build", "description": "x"}));
        assert!(!plain.label.contains('·'), "{}", plain.label);
        assert_eq!(subagent_model_label("openai/gpt-5"), "openai/gpt-5");
        let explicit = label(
            "subagent",
            "completed",
            json!({"agent": "build", "description": "x", "model": "openai/gpt-5#high"}),
        );
        assert!(
            explicit.label.ends_with("\u{b7} openai/gpt-5 (high)"),
            "{}",
            explicit.label
        );
    }

    // TOOL-009: `metadata.error` recolors the ROOT row to error even when the
    // transport state is completed (`index.tsx:3408-3427`), independent of the
    // metadata child rows updating from `metadata.toolCalls`.
    #[test]
    fn execute_metadata_error_recolors_the_completed_root() {
        let root = tool_display(80, "execute", "completed", r#"{"code":"1"}"#, "ok", false);
        assert_eq!(root.status, ToolStatus::Completed);
        assert_eq!(root.icon, "\u{2713}");
        let flipped = apply_tool_metadata(root, r#"{"error":true}"#);
        assert_eq!(flipped.status, ToolStatus::Error);
        assert_eq!(flipped.icon, "\u{2717}");
    }

    #[test]
    fn todo_question_and_patch_bodies() {
        let todos = tool_state_rows(
            80,
            "todowrite",
            "completed",
            r#"{"todos":[{"content":"one","status":"completed"},{"content":"two","status":"in_progress"},{"content":"three","status":"pending"}]}"#,
            "",
            false,
        );
        assert!(todos[0].0.contains("# Todos"));
        assert!(todos.iter().any(|(row, _)| row.contains("[✓] one")));
        assert!(todos.iter().any(|(row, _)| row.contains("[•] two")));
        assert!(todos.iter().any(|(row, _)| row.contains("[ ] three")));
        let cleared = tool_state_rows(80, "todowrite", "completed", "{}", "", false);
        assert!(cleared[0].0.contains("Cleared todos"));

        // Upstream renders `(no answer)` only when the answer array is absent;
        // a present-but-empty string is a real (blank) answer.
        let answers = r#"{"answers":[["blue"]]}"#;
        let questions = tool_state_rows(
            80,
            "question",
            "completed",
            r#"{"questions":[{"question":"colour?"},{"question":"size?"}]}"#,
            answers,
            false,
        );
        assert!(questions.iter().any(|(row, _)| row.contains("colour?")));
        assert!(questions.iter().any(|(row, _)| row.contains("blue")));
        assert!(questions.iter().any(|(row, _)| row.contains("(no answer)")));

        let patch = tool_state_rows(
            80,
            "apply_patch",
            "completed",
            r#"{"patchText":"*** Begin Patch\n*** Update File: a.rs\n*** Add File: b.rs\n*** End Patch"}"#,
            "",
            false,
        );
        assert!(patch[0].0.contains("Patch 2 files"));
        assert_eq!(patch_targets("*** Delete File: c\n"), vec!["c".to_owned()]);
    }

    #[test]
    fn ansi_sequences_never_leak() {
        let rows = tool_state_rows(
            80,
            "bash",
            "completed",
            r#"{"command":"printf hi"}"#,
            "\u{1b}[31mred\u{1b}[0m\n\u{1b}]0;title\u{7}after",
            false,
        );
        for (row, _) in &rows {
            assert!(!row.contains('\u{1b}'), "{row:?}");
            assert!(!row.contains("0;title"));
        }
    }

    #[test]
    fn execute_dialog_models_status_gutter_and_json() {
        let dialog = execute_dialog(
            "completed",
            r#"{"code":"const a = 1\nreturn a"}"#,
            "{\"ok\":true}\n\nLogs:\nline",
            false,
            Some(1_250),
        );
        assert_eq!(dialog.status, "Completed · 1.2s");
        assert!(!dialog.failed);
        assert_eq!(dialog.code, "const a = 1\nreturn a");
        assert_eq!(dialog.output_json.as_deref(), Some("{\"ok\":true}"));
        assert_eq!(dialog.output_rest.as_deref(), Some("Logs:\nline"));
        assert_eq!(dialog.digits, 1);
        let gutter = execute_dialog_gutter(&dialog.code, 4, dialog.digits);
        assert_eq!(gutter[0], "1");
        assert_eq!(gutter[1], " ");
        assert_eq!(gutter[2], " ");
        assert_eq!(gutter[3], "2");
        assert_eq!(gutter[4], " ");
        assert!(gutter.iter().all(|row| cells(row) == dialog.digits));

        let running = execute_dialog("running", "{}", "", false, None);
        assert_eq!(running.status, "Running");
        assert!(running.code.is_empty());
        let streaming = execute_dialog("streaming", "{}", "", false, None);
        assert_eq!(streaming.status, "Receiving code…");
        let failed = execute_dialog("error", "{}", "boom", true, None);
        assert_eq!(failed.status, "Failed");
        assert_eq!(failed.output.as_deref(), Some("boom"));
        assert_eq!(highlighted_output("plain text"), None);
        assert!(highlighted_output("{not json").is_none());
    }

    #[test]
    fn status_classification_is_conservative() {
        assert_eq!(classify_status("running", false), ToolStatus::Running);
        assert_eq!(classify_status("completed", false), ToolStatus::Completed);
        assert_eq!(classify_status("completed", true), ToolStatus::Error);
        assert_eq!(classify_status("ambiguous", false), ToolStatus::Pending);
        assert_eq!(classify_status("something-new", false), ToolStatus::Pending);
        assert_eq!(classify_status("completed", false).row_class(), ChatRowClass::ToolResult);
        assert_eq!(classify_status("running", false).row_class(), ChatRowClass::ToolCall);
        assert_eq!(classify_status("error", true).row_class(), ChatRowClass::ToolError);
    }

    /// TOOL-005: a denied tool keeps the muted (not error) color but moves to
    /// the dedicated class that carries the upstream strikethrough
    /// (`routes/session/message-parts.tsx:242,252,288`).
    #[test]
    fn denied_status_maps_to_the_strikethrough_class() {
        let display = tool_display(
            80,
            "shell",
            "completed",
            "",
            "PermissionRejectedError: user dismissed the request",
            true,
        );
        assert_eq!(display.status, ToolStatus::Denied);
        assert_eq!(display.status.row_class(), ChatRowClass::ToolDenied);
        assert_ne!(display.status.row_class(), ChatRowClass::ToolError);
    }

    #[test]
    fn wrapped_labels_keep_the_two_cell_icon_column() {
        let args = json!({"pattern": "a-very-long-pattern-name-that-wraps", "path": "deeply/nested/directory"});
        let rows = tool_state_rows(30, "glob", "completed", &args.to_string(), "", false);
        assert!(rows.len() > 1, "{rows:?}");
        for (row, _) in &rows {
            let cells_after_indent = cells(row) - cells(ROW_INDENT);
            assert!(cells_after_indent >= ICON_COLUMN_CELLS, "{row:?}");
            assert!(row.starts_with(ROW_INDENT), "{row:?}");
        }
        // Continuations still start after the fixed icon column.
        assert!(rows[1].0.starts_with(ROW_INDENT));
    }

    #[test]
    fn busy_rows_paint_the_live_spinner_frame() {
        // TOOL-002 (`message-parts.tsx:223-234`): a streaming/running row
        // replaces the icon column with the animated `spinner-frames.ts`
        // glyph at upstream `interval={80}` (`component/spinner.tsx:34`);
        // the frame index is the live `FrameClock` tick (`now_ms / 80`).
        let args = json!({"command": "make"});
        let frame0 = tool_state_rows_spin(40, "shell", "streaming", &args.to_string(), "", false, Some(0));
        assert!(frame0[0].0.contains('\u{280b}'), "tick 0 paints frame 0: {frame0:?}");
        assert!(!frame0[0].0.contains('\u{2839}'), "{frame0:?}");
        let frame2 = tool_state_rows_spin(40, "shell", "streaming", &args.to_string(), "", false, Some(2));
        assert!(frame2[0].0.contains('\u{2839}'), "tick 2 paints frame 2: {frame2:?}");
        // Reduced motion (the live loop's sentinel) paints the upstream `⋯`
        // fallback (`spinner.tsx:20`).
        let reduced =
            tool_state_rows_spin(40, "shell", "streaming", &args.to_string(), "", false, Some(SPINNER_REDUCED_MOTION));
        assert!(reduced[0].0.contains('\u{22ef}'), "{reduced:?}");
        // The non-live default (`None`) keeps the static icon byte-identical,
        // and terminal rows keep the status icon regardless of the live frame.
        let static_row = tool_state_rows(40, "shell", "streaming", &args.to_string(), "", false);
        assert!(static_row[0].0.contains('$'), "{static_row:?}");
        assert!(!static_row[0].0.contains('\u{280b}'), "{static_row:?}");
        let done = tool_state_rows_spin(40, "shell", "completed", &args.to_string(), "", false, Some(7));
        assert!(done[0].0.contains('$'), "{done:?}");
    }

    #[test]
    fn tool_call_and_result_entry_points_match_display() {
        let call = tool_call_rows(80, "bash");
        assert!(call[0].0.contains("Writing command…"));
        assert!(call.iter().all(|(_, class)| *class == ChatRowClass::ToolCall));
        let result = tool_result_rows(80, "grep", "src/a.rs\nsrc/b.rs", false);
        assert!(result[0].0.contains("Grep"));
        assert!(result.iter().any(|(row, _)| row.contains("src/a.rs")));
        assert_eq!(result[0].1, ChatRowClass::ToolResult);
        assert_eq!(tool_display_content("running", Some("x")), None);
        assert_eq!(tool_display_content("completed", Some("x")), Some("x"));
    }

    #[test]
    fn permission_needed_changes_tool_row_colour_not_text() {
        // TOOL-002: upstream adds NO badge for a pending permission — only the
        // row colour changes (`routes/session/index.tsx:2834-2847`). The fact
        // is joined from the pending request's `source.id`, never parsed from
        // the tool `metadata` object.
        let mut display = tool_display(80, "write", "running", r#"{"filePath":"a.rs"}"#, "", false);
        display.mark_permission_pending();
        assert!(display.permission_needed);
        let rows = rows_for_display(80, &display);
        assert_eq!(rows[0].1, ChatRowClass::ToolPermission);
        assert!(!rows.iter().any(|(row, _)| row.contains("Permission")));
    }

    /// TOOL-002 order: upstream checks the permission branch before the
    /// runtime-failure branch, so a pending permission paints the row warning
    /// even when the call also carries an error.
    #[test]
    fn permission_needed_beats_the_runtime_error_row_class() {
        let mut display = tool_display(80, "execute", "completed", r#"{"code":"1"}"#, "", false);
        display.status = ToolStatus::Error;
        display.mark_permission_pending();
        let rows = rows_for_display(80, &display);
        assert_eq!(rows[0].1, ChatRowClass::ToolPermission);
    }

    /// TOOL-006/007/008: `metadata` overlays the transport status.
    #[test]
    fn metadata_error_and_detached_background_overlay() {
        // `execute` runtime error forces the red root even when transport says
        // completed (`index.tsx:3401-3411`).
        let base = tool_display(80, "execute", "completed", r#"{"code":"1"}"#, "", false);
        assert_eq!(base.status, ToolStatus::Completed);
        let failed = apply_tool_metadata(base, r#"{"error":true}"#);
        assert_eq!(failed.status, ToolStatus::Error);
        assert_eq!(failed.icon, "✗");

        // Non-execute metadata error is not applied (upstream keys on toolCalls).
        let shell_meta_error = apply_tool_metadata(
            tool_display(80, "bash", "completed", r#"{"command":"ls"}"#, "", false),
            r#"{"error":true}"#,
        );
        assert_eq!(shell_meta_error.status, ToolStatus::Completed);

        // Detached process: part completed, process still running.
        let shell = apply_tool_metadata(
            tool_display(80, "bash", "completed", r#"{"command":"ls"}"#, "", false),
            r#"{"status":"running"}"#,
        );
        assert!(shell.background);
        let normal = apply_tool_metadata(
            tool_display(80, "bash", "completed", r#"{"command":"ls"}"#, "", false),
            r#"{"status":"completed"}"#,
        );
        assert!(!normal.background);
        let argument_flag = tool_display(
            80,
            "bash",
            "running",
            r#"{"command":"ls","background":true}"#,
            "",
            false,
        );
        assert!(!argument_flag.background, "only metadata.status selects detached mode");
        let unrelated = apply_tool_metadata(
            tool_display(80, "read", "completed", r#"{"path":"a.rs"}"#, "", false),
            r#"{"status":"running"}"#,
        );
        assert!(!unrelated.background, "metadata.status only controls shell/subagent badges");

        let execute = apply_tool_metadata(
            tool_display(80, "execute", "completed", r#"{"code":"1"}"#, "failure output", false),
            r#"{"error":true}"#,
        );
        let execute_rows = rows_for_display(80, &execute);
        assert!(execute_rows.iter().any(|(row, class)| *class == ChatRowClass::ToolError
            && row.contains("↳ failure output")), "metadata runtime error keeps bounded output: {execute_rows:?}");

        // Subagent in the same detached state paints the Background badge.
        let sub = apply_tool_metadata(
            tool_display(
                80,
                "subagent",
                "completed",
                r#"{"agent":"build","description":"x"}"#,
                "",
                false,
            ),
            r#"{"status":"running"}"#,
        );
        assert!(sub.background);
        assert!(!sub.label.contains("Background"));
        // The badge is emitted by `rows()` as one padded token, never in-label.
        let sub_rows = rows_for_display(80, &sub);
        assert!(
            sub_rows.iter().any(|(row, _)| row.contains("  Background ")),
            "badge token painted: {sub_rows:?}"
        );

        // Absent / malformed metadata changes nothing.
        let untouched = apply_tool_metadata(
            tool_display(80, "bash", "completed", r#"{"command":"ls"}"#, "", false),
            "not json",
        );
        assert!(!untouched.background);
        assert_eq!(untouched.status, ToolStatus::Completed);
    }

    /// TOOL-012: `Read` prints one `↳ Loaded <path>` row per loaded
    /// instruction file, completed parts only, strings only
    /// (`routes/session/index.tsx:3211-3243`).
    #[test]
    fn read_metadata_loaded_rows_render_only_when_completed() {
        let read = |status: &str, meta: &str| {
            apply_tool_metadata(
                tool_display(80, "read", status, r#"{"path":"src/a.rs"}"#, "", false),
                meta,
            )
        };
        // Completed: every string entry becomes a `↳ Loaded` row, and
        // non-string entries are dropped (`filter((p) => typeof p === "string")`).
        let done = read("completed", r#"{"loaded":["AGENTS.md","nested/RULES.md",7,null]}"#);
        let loaded: Vec<&String> = done
            .body
            .iter()
            .filter(|(row, _)| row.contains("↳ Loaded"))
            .map(|(row, _)| row)
            .collect();
        assert_eq!(loaded.len(), 2, "two string entries: {loaded:?}");
        assert_eq!(loaded[0], "  ↳ Loaded AGENTS.md");
        assert_eq!(loaded[1], "  ↳ Loaded nested/RULES.md");
        assert!(done.body.iter().any(|(_, class)| *class == ChatRowClass::Metadata));

        // Not completed: `loaded` is ignored entirely.
        let running = read("running", r#"{"loaded":["AGENTS.md"]}"#);
        assert!(!running.body.iter().any(|(row, _)| row.contains("↳ Loaded")));

        // Missing or non-array `loaded`: no rows.
        let none = read("completed", r#"{"loaded":"AGENTS.md"}"#);
        assert!(!none.body.iter().any(|(row, _)| row.contains("↳ Loaded")));

        // The row cap bounds a hostile payload.
        let many = format!(
            r#"{{"loaded":[{}]}}"#,
            (0..(MAX_LOADED_ROWS + 8))
                .map(|i| format!("\"f{i}\""))
                .collect::<Vec<_>>()
                .join(",")
        );
        let capped = read("completed", &many);
        assert_eq!(
            capped.body.iter().filter(|(row, _)| row.contains("↳ Loaded")).count(),
            MAX_LOADED_ROWS
        );
    }
}
