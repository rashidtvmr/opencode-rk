//! Keyboard/mouse tool-rule editor plus the pinned upstream permission prompt
//! overlay (P17 / FORM-001, FORM-002). All rule mutations require a daemon ACK.
//!
//! Upstream evidence, pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`:
//! * option set/order and the `save` gate — `packages/tui/src/routes/session/permission.tsx:232-240`;
//! * option labels — `packages/tui/src/util/permission.ts:162-168`;
//! * `always` body replacement (`permissionAlwaysLines`) —
//!   `packages/tui/src/util/permission.ts:154-160`,
//!   `packages/tui/src/routes/session/permission.tsx:223-231`;
//! * left/right (h/l) option cycling — `permission.tsx:465-485`;
//! * Escape: minimize fullscreen first, then the `escapeKey="reject"` selection —
//!   `permission.tsx:432-438,493-495`; ctrl-f toggles fullscreen —
//!   `permission.tsx:454-464`, binding `config/keybind.ts:281`;
//! * nested rejection stage only for child sessions — `permission.tsx:144-155,248-254`;
//! * rejection textarea: ctrl-c clears a non-empty reason else cancels, Esc cancels,
//!   Enter confirms — `permission.tsx:284-307`; confirmed reason is the raw
//!   textarea text, empty ⇒ omitted — `permission.tsx:148-150`;
//! * narrow footer (`width < 80`) stacks column instead of row —
//!   `permission.tsx:428,541-552`;
//! * normal/fullscreen geometry (`maxHeight 15` vs absolute inset 2/1) —
//!   `permission.tsx:514-523`;
//! * edit body (unified/split at width > 120, "No diff provided") —
//!   `permission.tsx:22-111`;
//! * per-tool presentation (title/icon/lines) —
//!   `packages/tui/src/util/permission.ts:23-152`.
#![forbid(unsafe_code)]
use crate::native_ui_style::{Line, Role};
const TOOLS: [&str; 10] = ["bash", "shell", "read", "write", "edit", "grep", "glob", "websearch", "webfetch", "task"];
const ACTIONS: [(&str, &str); 3] = [("allow", "1 Allow"), ("ask", "2 Ask"), ("deny", "3 Deny")];
const ACTION_GAP: &str = "    ";
pub struct Editor {
    pub open: bool,
    pub selected: usize,
    pub persistent: bool,
    pub error: Option<String>,
    rules: serde_json::Value,
}
impl Default for Editor {
    fn default() -> Self { Self {open:false,selected:0,persistent:false,error:None,rules:serde_json::json!({"rules":[]})} }
}
struct Layout {
    lines: Vec<Line>,
    hits: Vec<Option<usize>>,
    action_row: usize,
    action_spans: [(usize, usize); 3],
}
impl Editor {
    pub fn apply(&mut self, rules: serde_json::Value) { self.rules = rules; self.error = None; }
    pub fn navigate(&mut self, down: bool) {
        self.selected = if down { (self.selected + 1) % TOOLS.len() } else { (self.selected + TOOLS.len() - 1) % TOOLS.len() };
    }
    pub fn payload(&self, decision: &str, session: &str) -> serde_json::Value {
        serde_json::json!({"tool":TOOLS[self.selected],"decision":decision,"scope":if self.persistent {"always"} else {"session"},"session_id":session})
    }
    fn action_line() -> (String, [(usize, usize); 3]) {
        let mut text = String::new();
        let mut spans = [(0, 0); 3];
        for (index, (_, label)) in ACTIONS.iter().enumerate() {
            if index > 0 { text.push_str(ACTION_GAP); }
            spans[index].0 = text.len();
            text.push_str(label);
            spans[index].1 = text.len();
        }
        (text, spans)
    }
    fn layout(&self, height: usize) -> Layout {
        let mut lines = Vec::<Line>::new();
        let mut hits = Vec::new();
        {
            let mut push = |text:String,role,hit| { lines.push(Line {text,role,detail_from:None}); hits.push(hit); };
            push("Tool permissions".into(),Role::Heading,None);
            push(format!("Scope: {} · Tab change",if self.persistent {"Always (saved)"} else {"This session"}),Role::Description,None);
            let capacity = height.saturating_sub(5);
            let start = self.selected.saturating_sub(capacity.saturating_sub(1));
            for (index, tool) in TOOLS.iter().enumerate().skip(start).take(capacity) {
                let rule = self.rules.get("rules").and_then(serde_json::Value::as_array)
                    .and_then(|rules| rules.iter().rev().find(|rule| rule["tool"] == *tool
                        && rule["scope"].as_str().unwrap_or("always") == if self.persistent {"always"} else {"session"}));
                let decision = rule.and_then(|r| r["decision"].as_str()).unwrap_or("ask");
                push(format!("{} {:<10}  {}",if self.selected == index {">"} else {" "},tool,decision),
                    if self.selected == index {Role::Selected} else {Role::Item},Some(index));
            }
            push(self.error.clone().unwrap_or_else(|| "Rules never override protected operations.".into()),
                if self.error.is_some() {Role::Error} else {Role::Description},None);
        }
        let action_row = lines.len();
        let (action, action_spans) = Self::action_line();
        lines.push(Line {text:action,role:Role::Item,detail_from:None});
        hits.push(None);
        lines.push(Line {text:"↑↓/click tool · 1/2/3 set · Tab scope · Esc close".into(),role:Role::Hint,detail_from:None});
        hits.push(None);
        Layout {lines,hits,action_row,action_spans}
    }
    pub fn rows(&self, width:u16, height:usize) -> (Vec<String>, Vec<Role>, Vec<Option<usize>>) {
        let layout = self.layout(height);
        let limit = height.min(layout.lines.len());
        let rows = layout.lines.iter().take(limit).map(|line|crate::native_view_layout::fit(&line.text,width)).collect();
        let roles = layout.lines.iter().take(limit).map(|line|line.role).collect();
        let mut hits = layout.hits;
        hits.truncate(limit);
        (rows,roles,hits)
    }
    pub fn click_action(&mut self, x:u16, row:usize, width:u16,height:usize) -> Option<&'static str> {
        let layout = self.layout(height);
        if row >= height.min(layout.lines.len()) || x >= width { return None; }
        if let Some(index) = layout.hits.get(row).copied().flatten() { self.selected = index; return None; }
        if row == 1 {self.persistent = !self.persistent; return None;}
        if row != layout.action_row { return None; }
        let x = usize::from(x);
        ACTIONS.iter().enumerate().find_map(|(index,(action,_))| {
            let (start,end) = layout.action_spans[index];
            (x >= start && x < end && end <= usize::from(width)).then_some(*action)
        })
    }
}

// ---------------------------------------------------------------------------
// Upstream permission prompt overlay (FORM-001, FORM-002)
// ---------------------------------------------------------------------------

/// One selectable permission reply. The set and its order are fixed by
/// `permission.tsx:232-240`: `once`, `always` (only when the request carries
/// `save` rules), `reject`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionOption {
    Once,
    Always,
    Reject,
}

impl PermissionOption {
    /// Stable reply key sent to the backend (`PermissionReply`).
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Once => "once",
            Self::Always => "always",
            Self::Reject => "reject",
        }
    }

    /// Visible label, exact upstream text (`util/permission.ts:162-168`).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Once => "Allow once",
            Self::Always => "Always allow",
            Self::Reject => "Reject",
        }
    }
}

/// Ordered options for a request. A request without `save` rules omits
/// `always` entirely (`permission.tsx:232-240`); the remaining options keep
/// their upstream order — never reordered, never synthesized.
#[must_use]
pub fn permission_options(save_available: bool) -> Vec<PermissionOption> {
    if save_available {
        vec![PermissionOption::Once, PermissionOption::Always, PermissionOption::Reject]
    } else {
        vec![PermissionOption::Once, PermissionOption::Reject]
    }
}

/// Body shown while `always` is the selected option
/// (`util/permission.ts:154-160`). A lone `*` save rule is worded as a
/// project-wide allowance instead of a pattern list.
#[must_use]
pub fn permission_always_lines(action: &str, save: &[String]) -> Vec<String> {
    if save.len() == 1 && save[0] == "*" {
        return vec![format!("This will always allow {action} for this project.")];
    }
    let mut lines = vec!["This will always allow the following patterns for this project.".to_string()];
    lines.extend(save.iter().map(|item| format!("- {item}")));
    lines
}

/// Diff presentation of the edit permission body. `config.diffs.view`
/// overrides; otherwise split only above 120 columns
/// (`permission.tsx:32-37`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffView {
    Unified,
    Split,
}

#[must_use]
pub fn diff_view(width: usize, configured: Option<&str>) -> DiffView {
    match configured {
        Some("unified") => DiffView::Unified,
        Some("split") => DiffView::Split,
        _ if width > 120 => DiffView::Split,
        _ => DiffView::Unified,
    }
}

/// Resolved presentation for a permission request
/// (`util/permission.ts:23-144`). `diff`/`patch` are only set for edits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PermissionPresentation {
    pub icon: String,
    pub title: String,
    pub lines: Vec<String>,
    pub diff: Option<String>,
    pub patch: Option<String>,
    pub file: Option<String>,
}

fn as_dict(value: &serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    match value {
        serde_json::Value::Object(map) => map.clone(),
        _ => serde_json::Map::new(),
    }
}

fn field_text(value: &serde_json::Value, key: &str) -> String {
    value.get(key).and_then(serde_json::Value::as_str).unwrap_or("").to_string()
}

fn finite(value: &serde_json::Value, key: &str) -> Option<f64> {
    value.get(key).and_then(serde_json::Value::as_f64).filter(|n| n.is_finite())
}

fn titlecase(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut boundary = true;
    for ch in value.chars() {
        if boundary && ch.is_alphanumeric() {
            out.extend(ch.to_uppercase());
        } else {
            out.push(ch);
        }
        boundary = !ch.is_alphanumeric() && ch != '_';
    }
    out
}

fn wildcard_directory(value: &str) -> String {
    let Some(index) = value.find('*') else {
        return value.to_string();
    };
    let prefix = &value[..index];
    let root = !prefix.is_empty() && prefix.chars().all(|c| c == '/' || c == '\\');
    let drive = prefix.len() == 3
        && prefix.as_bytes()[0].is_ascii_alphabetic()
        && prefix.as_bytes()[1] == b':'
        && (prefix.as_bytes()[2] == b'/' || prefix.as_bytes()[2] == b'\\');
    if root || drive {
        return prefix.to_string();
    }
    prefix.trim_end_matches(['/', '\\']).to_string()
}

fn web_search_label(provider: &serde_json::Value) -> String {
    let name = match provider.as_str() {
        Some("") | None => String::new(),
        Some("opencode") => "OpenCode".to_string(),
        Some(other) => {
            let mut chars = other.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        }
    };
    if name.is_empty() { "Web Search".to_string() } else { format!("Web Search via {name}") }
}

/// Port of `permissionPresentation` (`util/permission.ts:23-144`). Input is
/// canonicalized first (`bash` → `shell`, `task` → `subagent`); `metadata`
/// overrides `tool_metadata`, and path-like inputs are normalized per tool.
#[must_use]
pub fn permission_presentation(
    action: &str,
    resources: &[String],
    input: &serde_json::Value,
    metadata: &serde_json::Value,
    tool_metadata: &serde_json::Value,
    format_path: &dyn Fn(&str) -> String,
) -> PermissionPresentation {
    let action = match action {
        "bash" => "shell",
        "task" => "subagent",
        "apply_patch" => "patch",
        other => other,
    };
    let mut merged = as_dict(tool_metadata);
    merged.extend(as_dict(metadata));
    let metadata = serde_json::Value::Object(merged);
    let path = {
        let direct = field_text(input, "path");
        if direct.is_empty() {
            let file_path = field_text(input, "filePath");
            if file_path.is_empty() { field_text(input, "filepath") } else { file_path }
        } else {
            direct
        }
    };
    let first_resource = resources.first().cloned().unwrap_or_default();
    let line = |text: String| text;
    match action {
        "edit" => {
            let file = if path.is_empty() { first_resource.clone() } else { path.clone() };
            let first = metadata
                .get("files")
                .and_then(serde_json::Value::as_array)
                .and_then(|files| files.first())
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let diff = {
                let from_first = {
                    let patch = field_text(&first, "patch");
                    if patch.is_empty() { field_text(&first, "diff") } else { patch }
                };
                if from_first.is_empty() {
                    let direct = field_text(&metadata, "diff");
                    if direct.is_empty() { None } else { Some(direct) }
                } else {
                    Some(from_first)
                }
            };
            let patch = if diff.is_none() {
                let text = field_text(input, "patchText");
                if text.is_empty() { None } else { Some(text) }
            } else {
                None
            };
            PermissionPresentation {
                icon: "→".into(),
                title: format!("Edit {}", format_path(&file)),
                lines: Vec::new(),
                diff,
                patch,
                file: Some(file),
            }
        }
        "read" | "list" => {
            let value = if path.is_empty() { first_resource } else { path };
            let title = if action == "read" { "Read" } else { "List" };
            PermissionPresentation {
                icon: "→".into(),
                title: format!("{title} {}", format_path(&value)),
                lines: if value.is_empty() { Vec::new() } else { vec![format!("Path: {}", format_path(&value))] },
                ..PermissionPresentation::default()
            }
        }
        "glob" | "grep" => {
            let pattern = {
                let direct = field_text(input, "pattern");
                if direct.is_empty() { first_resource } else { direct }
            };
            let title = if action == "glob" { "Glob" } else { "Grep" };
            PermissionPresentation {
                icon: "✱".into(),
                title: format!("{title} \"{pattern}\""),
                lines: if pattern.is_empty() { Vec::new() } else { vec![line(format!("Pattern: {pattern}"))] },
                ..PermissionPresentation::default()
            }
        }
        "shell" => {
            let command = field_text(input, "command");
            PermissionPresentation {
                icon: "#".into(),
                title: "Shell command".into(),
                lines: if command.is_empty() {
                    resources.iter().map(|item| format!("- {item}")).collect()
                } else {
                    vec![format!("$ {command}")]
                },
                ..PermissionPresentation::default()
            }
        }
        "subagent" => {
            let agent = {
                let direct = field_text(input, "agent");
                if direct.is_empty() {
                    let fallback = field_text(input, "subagent_type");
                    if fallback.is_empty() { "general".to_string() } else { fallback }
                } else {
                    direct
                }
            };
            let description = field_text(input, "description");
            PermissionPresentation {
                icon: "#".into(),
                title: format!("{} Subagent", titlecase(&agent)),
                lines: if description.is_empty() { Vec::new() } else { vec![format!("◉ {description}")] },
                ..PermissionPresentation::default()
            }
        }
        "webfetch" => {
            let url = {
                let direct = field_text(input, "url");
                if direct.is_empty() { field_text(&metadata, "url") } else { direct }
            };
            PermissionPresentation {
                icon: "%".into(),
                title: format!("WebFetch {url}"),
                lines: if url.is_empty() { Vec::new() } else { vec![format!("URL: {url}")] },
                ..PermissionPresentation::default()
            }
        }
        "websearch" => {
            let query = {
                let direct = field_text(input, "query");
                if direct.is_empty() { field_text(&metadata, "query") } else { direct }
            };
            let title = web_search_label(metadata.get("provider").unwrap_or(&serde_json::Value::Null));
            PermissionPresentation {
                icon: "◈".into(),
                title: if query.is_empty() { title } else { format!("{title} \"{query}\"") },
                lines: if query.is_empty() { Vec::new() } else { vec![format!("Query: {query}")] },
                ..PermissionPresentation::default()
            }
        }
        "lsp" => {
            let file = field_text(input, "path");
            let operation = {
                let direct = field_text(input, "operation");
                if direct.is_empty() { "request".to_string() } else { direct }
            };
            let position = match (finite(input, "line"), finite(input, "character")) {
                (Some(line), Some(character)) => Some(format!("{}:{}", line as i64, character as i64)),
                _ => None,
            };
            let mut lines = Vec::new();
            if !field_text(input, "operation").is_empty() {
                lines.push(format!("Operation: {operation}"));
            }
            if !file.is_empty() {
                lines.push(format!("Path: {}", format_path(&file)));
            }
            if let Some(position) = &position {
                lines.push(format!("Position: {position}"));
            }
            let suffix = if file.is_empty() {
                String::new()
            } else {
                let position = position.as_ref().map(|value| format!(":{value}")).unwrap_or_default();
                format!(" {}{position}", format_path(&file))
            };
            PermissionPresentation {
                icon: "→".into(),
                title: format!("LSP {operation}{suffix}"),
                lines,
                ..PermissionPresentation::default()
            }
        }
        "external_directory" => {
            let raw = {
                let parent = field_text(&metadata, "parentDir");
                if parent.is_empty() {
                    let filepath = field_text(&metadata, "filepath");
                    if filepath.is_empty() { first_resource.clone() } else { filepath }
                } else {
                    parent
                }
            };
            PermissionPresentation {
                icon: "←".into(),
                title: format!("Access external directory {}", format_path(&wildcard_directory(&raw))),
                lines: resources.iter().map(|item| format!("- {item}")).collect(),
                ..PermissionPresentation::default()
            }
        }
        "doom_loop" => PermissionPresentation {
            icon: "⟳".into(),
            title: "Continue after repeated failures".into(),
            lines: vec!["This keeps the session running despite repeated failures.".into()],
            ..PermissionPresentation::default()
        },
        _ => PermissionPresentation {
            icon: "⚙".into(),
            title: format!("Call tool {action}"),
            lines: vec![format!("Tool: {action}")],
            ..PermissionPresentation::default()
        },
    }
}

/// One rendered body line with its upstream emphasis: the first `always`
/// line is muted, all following lines are base text
/// (`permission.tsx:226-228`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptLine {
    pub text: String,
    pub muted: bool,
}

impl PromptLine {
    fn muted(text: String) -> Self {
        Self { text, muted: true }
    }
    fn base(text: String) -> Self {
        Self { text, muted: false }
    }
}

/// Which prompt the overlay is showing (`permission.tsx:20,144-156`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionStage {
    Permission,
    Reject,
}

/// Keyboard owner; the rejection textarea is focused only in the nested
/// rejection stage (`permission.tsx:343-361`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptFocus {
    Options,
    RejectMessage,
}

/// Result of selecting a permission option.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PermissionOutcome {
    /// Send this `once`/`always`/`reject` reply.
    Reply(&'static str),
    /// Nested rejection stage opened (child session only); no reply yet.
    NestedReject,
}

/// Result of Escape (`permission.tsx:432-438,493-495`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EscapeOutcome {
    Minimized,
    /// Escape inside the rejection textarea cancels that stage and returns
    /// to the option prompt, issuing no reply (`permission.tsx:302`).
    RejectionCancelled,
    Rejected(PermissionOutcome),
}

/// Result of Enter in the nested rejection stage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RejectReply {
    /// Raw rejection reason; empty text is omitted (`message || undefined`,
    /// `permission.tsx:148-150`).
    pub message: Option<String>,
}

/// Overlay geometry from `permission.tsx:501-523`. Normal mode is a relative
/// box capped at 15 rows; fullscreen is absolute with the 2/1 insets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PromptGeometry {
    pub expanded: bool,
    pub max_height: Option<usize>,
    pub top_offset: isize,
    pub bottom: usize,
    pub left: usize,
    pub right: usize,
}

/// Permission prompt state machine. Pure: no IO, no clock, no threads. The
/// caller renders [`Self::body`]/[`Self::footer_rows`] and dispatches the
/// returned reply strings.
#[derive(Clone, Debug)]
pub struct PermissionPrompt {
    action: String,
    save: Vec<String>,
    session_has_parent: bool,
    selected: PermissionOption,
    expanded: bool,
    stage: PermissionStage,
    focus: PromptFocus,
    reject_message: String,
}

impl PermissionPrompt {
    #[must_use]
    pub fn new(action: impl Into<String>, save: Vec<String>, session_has_parent: bool) -> Self {
        let selected = permission_options(!save.is_empty())[0];
        Self {
            action: action.into(),
            save,
            session_has_parent,
            selected,
            expanded: false,
            stage: PermissionStage::Permission,
            focus: PromptFocus::Options,
            reject_message: String::new(),
        }
    }

    #[must_use]
    pub fn options(&self) -> Vec<PermissionOption> {
        permission_options(!self.save.is_empty())
    }
    #[must_use]
    pub fn selected(&self) -> PermissionOption {
        self.selected
    }
    #[must_use]
    pub fn stage(&self) -> PermissionStage {
        self.stage
    }
    #[must_use]
    pub fn focus(&self) -> PromptFocus {
        self.focus
    }
    #[must_use]
    pub fn expanded(&self) -> bool {
        self.expanded
    }
    #[must_use]
    pub fn reject_message(&self) -> &str {
        &self.reject_message
    }

    /// left/h and right/l cycle the options (`permission.tsx:465-485`).
    /// Ignored outside the permission stage.
    pub fn move_option(&mut self, forward: bool) -> bool {
        if self.stage != PermissionStage::Permission {
            return false;
        }
        let options = self.options();
        let index = options.iter().position(|o| *o == self.selected).unwrap_or(0);
        let next = if forward {
            (index + 1) % options.len()
        } else {
            (index + options.len() - 1) % options.len()
        };
        self.selected = options[next];
        true
    }

    /// ctrl-f fullscreen toggle (`permission.tsx:454-464`).
    pub fn toggle_fullscreen(&mut self) -> bool {
        if self.stage != PermissionStage::Permission {
            return false;
        }
        self.expanded = !self.expanded;
        true
    }

    /// Option activation (`permission.tsx:243-257`). `always` and `once`
    /// reply immediately; `reject` opens the nested reason only for a child
    /// session, otherwise it replies immediately.
    pub fn choose(&mut self, option: PermissionOption) -> PermissionOutcome {
        self.selected = option;
        match option {
            PermissionOption::Once => PermissionOutcome::Reply("once"),
            PermissionOption::Always => PermissionOutcome::Reply("always"),
            PermissionOption::Reject => {
                if self.session_has_parent {
                    self.stage = PermissionStage::Reject;
                    self.focus = PromptFocus::RejectMessage;
                    PermissionOutcome::NestedReject
                } else {
                    PermissionOutcome::Reply("reject")
                }
            }
        }
    }

    /// Enter selects the focused option (`permission.tsx:487-492`).
    pub fn enter(&mut self) -> Option<PermissionOutcome> {
        match self.stage {
            PermissionStage::Permission => Some(self.choose(self.selected)),
            // Enter in the rejection textarea confirms it (`permission.tsx:300-306`).
            PermissionStage::Reject => None,
        }
    }

    /// Escape minimizes fullscreen before it rejects
    /// (`permission.tsx:432-438`). In the rejection stage Escape cancels that
    /// stage (`permission.tsx:299`).
    pub fn escape(&mut self) -> EscapeOutcome {
        if self.stage == PermissionStage::Reject {
            self.reject_cancel();
            return EscapeOutcome::RejectionCancelled;
        }
        if self.expanded {
            self.expanded = false;
            return EscapeOutcome::Minimized;
        }
        EscapeOutcome::Rejected(self.choose(PermissionOption::Reject))
    }

    /// ctrl-c inside the rejection textarea: clear a non-empty reason, else
    /// cancel the rejection (`permission.tsx:288-298`).
    pub fn reject_ctrl_c(&mut self) {
        if self.stage != PermissionStage::Reject {
            return;
        }
        if self.reject_message.is_empty() {
            self.reject_cancel();
        } else {
            self.reject_message.clear();
        }
    }

    pub fn type_reject(&mut self, text: &str) {
        if self.stage == PermissionStage::Reject {
            self.reject_message.push_str(text);
        }
    }

    /// Esc in the rejection stage returns to the permission options and
    /// discards the reason editor (the upstream `RejectPrompt` unmounts, so a
    /// later rejection starts empty — `permission.tsx:302`).
    pub fn reject_cancel(&mut self) {
        self.stage = PermissionStage::Permission;
        self.focus = PromptFocus::Options;
        self.reject_message.clear();
    }

    /// Confirm the rejection. The raw text is forwarded; an empty reason is
    /// omitted rather than sent as an empty string.
    pub fn reject_confirm(&mut self) -> RejectReply {
        self.stage = PermissionStage::Permission;
        self.focus = PromptFocus::Options;
        let message = if self.reject_message.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.reject_message))
        };
        RejectReply { message }
    }

    /// Body for the currently selected option
    /// (`permission.tsx:217-231`): selecting `always` swaps the body for the
    /// save-rule summary; every other option shows the request presentation.
    #[must_use]
    pub fn body(&self, presentation: &PermissionPresentation) -> Vec<PromptLine> {
        if self.selected == PermissionOption::Always {
            return permission_always_lines(&self.action, &self.save)
                .into_iter()
                .enumerate()
                .map(|(index, text)| if index == 0 { PromptLine::muted(text) } else { PromptLine::base(text) })
                .collect();
        }
        // Upstream selects `EditBody` only for `action === "edit"` (`permission.tsx:169`).
        if self.action == "edit" {
            return edit_body_lines(presentation);
        }
        presentation.lines.iter().cloned().map(PromptLine::base).collect()
    }

    /// Narrow (`width < 80`) footers stack as a column; wider footers are a
    /// single row with one-cell gaps (`permission.tsx:541-552`).
    #[must_use]
    pub fn footer_rows(&self, width: usize) -> Vec<String> {
        let mut items = vec![format!("ctrl+f {}", if self.expanded { "minimize" } else { "fullscreen" })];
        if self.options().len() > 1 {
            items.push("⇆ select".to_string());
        }
        items.push("enter confirm".to_string());
        if width < 80 {
            items
        } else {
            vec![items.join(" ")]
        }
    }

    /// Rejection prompt geometry: `narrow` stacks the textarea above the
    /// actions (`permission.tsx:331-341`), and the box is a raised panel with
    /// a left error border (`permission.tsx:309-330`).
    #[must_use]
    pub fn reject_row_layout(&self, width: usize) -> &'static str {
        if width < 80 { "column" } else { "row" }
    }

    /// Dialog box geometry (`permission.tsx:514-523`).
    #[must_use]
    pub fn geometry(&self, terminal_height: usize) -> PromptGeometry {
        if self.expanded {
            PromptGeometry {
                expanded: true,
                max_height: None,
                top_offset: 1 - terminal_height as isize,
                bottom: 1,
                left: 2,
                right: 2,
            }
        } else {
            PromptGeometry { expanded: false, max_height: Some(15), top_offset: 0, bottom: 0, left: 0, right: 0 }
        }
    }

    /// Header rows (`permission.tsx:200-215`): warning marker plus
    /// "Permission required", then the per-tool icon/title unless the action
    /// is `shell` (shell commands are shown only in the body).
    #[must_use]
    pub fn header(&self, presentation: &PermissionPresentation) -> Vec<String> {
        let mut rows = vec!["△ Permission required".to_string()];
        if self.action != "shell" && !presentation.title.is_empty() {
            rows.push(format!("  {} {}", presentation.icon, presentation.title));
        }
        rows
    }
}

/// Edit body rows (`permission.tsx:42-111`): a scrollable diff when the
/// request carries one, otherwise the raw patch, otherwise a muted
/// "No diff provided" line inset by one cell.
#[must_use]
pub fn edit_body_lines(presentation: &PermissionPresentation) -> Vec<PromptLine> {
    if let Some(diff) = presentation.diff.as_deref().filter(|diff| !diff.is_empty()) {
        return diff.lines().map(|line| PromptLine::base(line.to_string())).collect();
    }
    if let Some(patch) = presentation.patch.as_deref().filter(|patch| !patch.is_empty()) {
        return patch.lines().map(|line| PromptLine::muted(line.to_string())).collect();
    }
    vec![PromptLine::muted(" No diff provided".to_string())]
}

#[cfg(test)]
mod tests {
    use super::*;

    // FORM-001 — once/always/reject variants, `save` gate, body swap,
    // nested rejection scope.
    // Upstream: permission.tsx:232-240 (options + save gate),
    // permission.tsx:217-257 (body + selection semantics),
    // util/permission.ts:154-175 (always lines + labels),
    // permission.tsx:144-155,248-254 (nested rejection stage).
    #[test]
    fn form_001_non_edit_action_never_renders_edit_body_from_diff_or_patch() {
        // Upstream branches on action, not on optional diff/patch data.
        let prompt = PermissionPrompt::new("shell", Vec::new(), false);
        let presentation = PermissionPresentation {
            icon: "#".into(),
            title: "Shell command".into(),
            lines: vec!["$ echo ok".into()],
            diff: Some("-old\n+new".into()),
            patch: Some("*** Begin Patch".into()),
            file: None,
        };

        assert_eq!(
            prompt.body(&presentation),
            vec![PromptLine::base("$ echo ok".into())],
        );
    }

    #[test]
    fn form_001_permission_variants_save_gate_body_and_reject_scope() {
        // Option order and labels are fixed.
        let mut prompt = PermissionPrompt::new("edit", vec!["src/**".into()], true);
        assert_eq!(prompt.options(), vec![PermissionOption::Once, PermissionOption::Always, PermissionOption::Reject]);
        assert_eq!(prompt.selected(), PermissionOption::Once);
        assert_eq!(PermissionOption::Once.label(), "Allow once");
        assert_eq!(PermissionOption::Always.label(), "Always allow");
        assert_eq!(PermissionOption::Reject.label(), "Reject");
        assert_eq!(
            prompt.options().iter().map(|o| o.key()).collect::<Vec<_>>(),
            vec!["once", "always", "reject"]
        );

        // Missing save rules omit `always`; the rest keep their order.
        let mut no_save = PermissionPrompt::new("edit", Vec::new(), false);
        assert_eq!(no_save.options(), vec![PermissionOption::Once, PermissionOption::Reject]);
        assert_eq!(no_save.selected(), PermissionOption::Once);

        // Changing the selected option changes the body.
        let request = serde_json::json!({"path": "src/main.rs"});
        let empty = serde_json::json!({});
        let presentation = permission_presentation(
            "edit",
            &[],
            &request,
            &empty,
            &empty,
            &|value| value.to_string(),
        );
        assert_eq!(presentation.title, "Edit src/main.rs");
        let once_body = prompt.body(&presentation);
        assert_eq!(once_body, vec![PromptLine::muted(" No diff provided".into())]);
        assert!(prompt.move_option(true));
        assert_eq!(prompt.selected(), PermissionOption::Always);
        let always_body = prompt.body(&presentation);
        assert_eq!(
            always_body,
            vec![
                PromptLine::muted("This will always allow the following patterns for this project.".into()),
                PromptLine::base("- src/**".into()),
            ]
        );
        assert_ne!(once_body, always_body);
        // A lone `*` save rule uses the project-wide wording.
        assert_eq!(
            permission_always_lines("edit", &["*".into()]),
            vec!["This will always allow edit for this project.".to_string()]
        );

        // Nested rejection is scoped to a child session only.
        assert!(prompt.move_option(true)); // reject
        assert_eq!(prompt.selected(), PermissionOption::Reject);
        assert_eq!(prompt.choose(PermissionOption::Reject), PermissionOutcome::NestedReject);
        assert_eq!(prompt.stage(), PermissionStage::Reject);
        assert_eq!(prompt.focus(), PromptFocus::RejectMessage);
        // Cancelling returns to the same prompt, no reply issued.
        prompt.reject_cancel();
        assert_eq!(prompt.stage(), PermissionStage::Permission);
        assert_eq!(prompt.focus(), PromptFocus::Options);

        // A root session rejects immediately instead of nesting a reason.
        let mut root = PermissionPrompt::new("edit", vec!["*".into()], false);
        assert_eq!(root.choose(PermissionOption::Reject), PermissionOutcome::Reply("reject"));
        assert_eq!(root.stage(), PermissionStage::Permission);
        assert_eq!(root.choose(PermissionOption::Once), PermissionOutcome::Reply("once"));
        assert_eq!(root.choose(PermissionOption::Always), PermissionOutcome::Reply("always"));
    }

    // FORM-002 — 79/80 width footer, normal/fullscreen, Escape minimize then
    // reject, ctrl-c clear/cancel, edit-diff body geometry and focus.
    // Upstream: permission.tsx:428,541-552 (narrow stacking),
    // permission.tsx:432-438,493-495 (escape semantics),
    // permission.tsx:454-464,514-523 (fullscreen geometry),
    // permission.tsx:284-307 (ctrl-c), permission.tsx:32-37,42-111 (edit body).
    #[test]
    fn form_002_permission_width_fullscreen_escape_and_edit_body() {
        let prompt = PermissionPrompt::new("edit", vec!["src/**".into()], false);
        // 79 columns stack the footer; 80 columns keep one row.
        let narrow = prompt.footer_rows(79);
        assert_eq!(narrow.len(), 3, "{narrow:?}");
        assert_eq!(narrow[0], "ctrl+f fullscreen");
        assert_eq!(narrow[1], "⇆ select");
        assert_eq!(narrow[2], "enter confirm");
        let wide = prompt.footer_rows(80);
        assert_eq!(wide, vec!["ctrl+f fullscreen ⇆ select enter confirm".to_string()]);
        // The rejection prompt stacks textarea above actions below 80.
        assert_eq!(prompt.reject_row_layout(79), "column");
        assert_eq!(prompt.reject_row_layout(80), "row");

        // Normal geometry is capped at 15 rows; fullscreen insets 2/1.
        let normal = prompt.geometry(40);
        assert_eq!(normal.max_height, Some(15));
        assert_eq!((normal.left, normal.right, normal.bottom, normal.top_offset), (0, 0, 0, 0));
        let mut fullscreen = PermissionPrompt::new("edit", vec!["src/**".into()], false);
        assert!(fullscreen.toggle_fullscreen());
        let expanded = fullscreen.geometry(40);
        assert_eq!(expanded.max_height, None);
        assert_eq!((expanded.left, expanded.right, expanded.bottom), (2, 2, 1));
        assert_eq!(expanded.top_offset, 1 - 40);
        assert_eq!(fullscreen.footer_rows(80), vec!["ctrl+f minimize ⇆ select enter confirm".to_string()]);

        // Escape minimizes fullscreen first, issuing no reply...
        assert_eq!(fullscreen.escape(), EscapeOutcome::Minimized);
        assert!(!fullscreen.expanded());
        // ... and only the next Escape activates `reject`.
        assert_eq!(
            fullscreen.escape(),
            EscapeOutcome::Rejected(PermissionOutcome::Reply("reject"))
        );

        // ctrl-c clears a non-empty rejection reason, else cancels the stage.
        let mut reject = PermissionPrompt::new("edit", vec!["src/**".into()], true);
        reject.choose(PermissionOption::Reject);
        reject.type_reject("use a safer tool");
        reject.reject_ctrl_c();
        assert_eq!(reject.reject_message(), "");
        assert_eq!(reject.stage(), PermissionStage::Reject, "clear keeps the reason editor");
        reject.reject_ctrl_c();
        assert_eq!(reject.stage(), PermissionStage::Permission, "empty ctrl-c cancels");
        reject.choose(PermissionOption::Reject);
        reject.type_reject("use a safer tool");
        assert_eq!(reject.reject_confirm().message.as_deref(), Some("use a safer tool"));
        reject.choose(PermissionOption::Reject);
        assert_eq!(reject.reject_confirm().message, None, "empty reason is omitted");
        // Escape inside the rejection stage cancels it with no reply and
        // discards the editor text (`permission.tsx:302`).
        reject.choose(PermissionOption::Reject);
        reject.type_reject("discard me");
        assert_eq!(reject.escape(), EscapeOutcome::RejectionCancelled);
        assert_eq!(reject.stage(), PermissionStage::Permission);
        assert_eq!(reject.reject_message(), "", "unmounted editor text is gone");

        // Edit-diff body: unified vs split at the terminal width boundary.
        assert_eq!(diff_view(120, None), DiffView::Unified);
        assert_eq!(diff_view(121, None), DiffView::Split);
        assert_eq!(diff_view(121, Some("unified")), DiffView::Unified);
        let diff_presentation = PermissionPresentation {
            icon: "→".into(),
            title: "Edit a.rs".into(),
            lines: Vec::new(),
            diff: Some("-a\n+b".into()),
            patch: None,
            file: Some("a.rs".into()),
        };
        assert_eq!(
            edit_body_lines(&diff_presentation),
            vec![PromptLine::base("-a".into()), PromptLine::base("+b".into())]
        );
        // Upstream's Show treats empty diff/patch strings as absent.
        let empty_diff_with_patch = PermissionPresentation {
            diff: Some(String::new()),
            patch: Some("*** Begin Patch".into()),
            ..diff_presentation.clone()
        };
        assert_eq!(
            edit_body_lines(&empty_diff_with_patch),
            vec![PromptLine::muted("*** Begin Patch".into())]
        );
        let empty_patch = PermissionPresentation {
            diff: None,
            patch: Some(String::new()),
            ..diff_presentation.clone()
        };
        assert_eq!(edit_body_lines(&empty_patch), vec![PromptLine::muted(" No diff provided".into())]);
        let mut keep_selection = PermissionPrompt::new("edit", vec!["*".into()], false);
        keep_selection.choose(PermissionOption::Always);
        keep_selection.toggle_fullscreen();
        // Fullscreen toggling never moves the selection or the focus owner.
        assert_eq!(keep_selection.selected(), PermissionOption::Always);
        assert_eq!(keep_selection.focus(), PromptFocus::Options);
        // Header keeps the warning row first; shell commands omit the title row.
        let shell = permission_presentation(
            "shell",
            &[],
            &serde_json::json!({"command": "cargo test"}),
            &serde_json::json!({}),
            &serde_json::json!({}),
            &|value| value.to_string(),
        );
        assert_eq!(shell.lines, vec!["$ cargo test".to_string()]);
        let shell_prompt = PermissionPrompt::new("shell", vec!["*".into()], false);
        assert_eq!(shell_prompt.header(&shell), vec!["△ Permission required".to_string()]);
    }

    // Presentation mapping for the remaining upstream tool branches.
    #[test]
    fn permission_presentation_matches_upstream_tool_branches() {
        let empty = serde_json::json!({});
        let format = |value: &str| format!("/fmt/{value}");
        let external = permission_presentation(
            "external_directory",
            &["/tmp/a/**".into(), "/tmp/b".into()],
            &empty,
            &serde_json::json!({"parentDir": "/tmp/*/"}),
            &empty,
            &format,
        );
        assert_eq!(external.icon, "←");
        assert_eq!(external.title, "Access external directory /fmt//tmp");
        assert_eq!(external.lines, vec!["- /tmp/a/**".to_string(), "- /tmp/b".to_string()]);
        let search = permission_presentation(
            "websearch",
            &[],
            &serde_json::json!({"query": "rust"}),
            &serde_json::json!({"provider": "opencode"}),
            &empty,
            &format,
        );
        assert_eq!(search.title, "Web Search via OpenCode \"rust\"");
        assert_eq!(search.lines, vec!["Query: rust".to_string()]);
        let task = permission_presentation(
            "task",
            &[],
            &serde_json::json!({"subagent_type": "explore", "description": "find it"}),
            &empty,
            &empty,
            &format,
        );
        assert_eq!(task.title, "Explore Subagent");
        assert_eq!(task.lines, vec!["◉ find it".to_string()]);
        let unknown = permission_presentation("mystery", &[], &empty, &empty, &empty, &format);
        assert_eq!(unknown.title, "Call tool mystery");
        let looped = permission_presentation("doom_loop", &[], &empty, &empty, &empty, &format);
        assert_eq!(looped.icon, "⟳");
        assert_eq!(looped.title, "Continue after repeated failures");
    }

    #[test]
    fn rule_edits_are_explicit_and_scope_is_bound_to_current_session() {
        let mut editor=Editor::default();
        assert_eq!(editor.payload("deny","s")["scope"],"session");
        editor.persistent=true;editor.navigate(true);
        assert_eq!(editor.payload("allow","s")["tool"],"shell");
        assert_eq!(editor.payload("allow","s")["session_id"],"s");
        assert_eq!(editor.payload("allow","s")["scope"],"always");
    }
    #[test]
    fn compact_editor_keeps_selection_and_mouse_actions_visible() {
        let mut editor=Editor::default();
        for _ in 0..9 {editor.navigate(true);}
        let (rows,roles,hits)=editor.rows(52,12);
        assert!(rows.len()<=12);
        let row=roles.iter().position(|role|*role==Role::Selected).unwrap();
        assert_eq!(hits[row],Some(9));
        assert_eq!(editor.click_action(1,rows.len()-2,52,12),Some("allow"));
        assert_eq!(editor.click_action(13,rows.len()-2,52,12),Some("ask"));
        assert_eq!(editor.click_action(23,rows.len()-2,52,12),Some("deny"));
    }
    #[test]
    fn rendered_action_hit_map_matches_numeric_keyboard_labels() {
        let mut editor=Editor::default();
        let (rows,_,_)=editor.rows(52,12);
        let action_row=rows.iter().position(|row|row.contains(ACTIONS[0].1)).unwrap();
        let labels=ACTIONS.iter().map(|(_,label)|*label).collect::<Vec<_>>().join(ACTION_GAP);
        assert_eq!(rows[action_row].trim_end(),labels.as_str());
        for (index,(decision,label)) in ACTIONS.iter().enumerate() {
            assert_eq!(label.chars().next(),Some(char::from(b'1'+index as u8)));
            assert_eq!(editor.payload(decision,"s")["decision"],*decision);
            let start=rows[action_row].find(*label).unwrap();
            for x in start..start+label.len() {
                assert_eq!(editor.click_action(x as u16,action_row,52,12),Some(*decision),"x={x}");
            }
        }
        assert_eq!(editor.click_action(7,action_row,52,12),None);
        assert_eq!(editor.click_action(8,action_row,52,12),None);
        assert_eq!(editor.click_action(16,action_row,52,12),None);
        assert_eq!(editor.click_action(19,action_row,52,12),None);
        assert_eq!(editor.click_action(52,action_row,52,12),None);
        assert_eq!(editor.selected,0);
    }
    #[test]
    fn truncated_or_hidden_action_rows_never_apply_rules() {
        let mut editor=Editor::default();
        let (rows,_,_)=editor.rows(13,6);
        let action_row=rows.iter().position(|row|row.contains("1 Allow")).unwrap();
        assert!(!rows[action_row].contains("2 Ask"));
        assert_eq!(editor.click_action(0,action_row,13,6),Some("allow"));
        assert_eq!(editor.click_action(11,action_row,13,6),None);
        assert_eq!(editor.click_action(12,action_row,13,6),None);

        let (narrow,_,_)=editor.rows(4,6);
        let action_row=narrow.iter().position(|row|row.starts_with("1 Al")).unwrap();
        assert_eq!(editor.click_action(0,action_row,4,6),None);

        let (short,_,_)=editor.rows(52,3);
        assert_eq!(short.len(),3);
        assert_eq!(editor.selected,0);
        assert_eq!(editor.click_action(0,2,52,3),None);
        assert_eq!(editor.selected,0);
        assert_eq!(editor.click_action(0,3,52,3),None);

        let (tiny,_,_)=editor.rows(52,1);
        assert_eq!(tiny.len(),1);
        assert!(!editor.persistent);
        assert_eq!(editor.click_action(0,1,52,1),None);
        assert!(!editor.persistent);

        for height in 0..8 {
            let (bounded,_,_)=editor.rows(52,height);
            assert!(bounded.len()<=height);
        }
    }
    #[test]
    fn scope_row_click_matches_tab_scope_toggle() {
        let mut editor=Editor::default();
        let (rows,_,_)=editor.rows(52,12);
        assert!(rows[1].starts_with("Scope: This session"));
        assert_eq!(editor.click_action(0,1,52,12),None);
        assert!(editor.persistent);
        assert!(editor.rows(52,12).0[1].starts_with("Scope: Always (saved)"));
        assert_eq!(editor.click_action(0,1,52,12),None);
        assert!(!editor.persistent);
        assert_eq!(editor.click_action(60,1,52,12),None);
        assert!(!editor.persistent);
    }
}
