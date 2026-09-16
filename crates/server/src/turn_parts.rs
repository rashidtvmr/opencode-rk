//! WEB-009: structured assistant-turn parts (reasoning summary, tool
//! activity, final answer, references).
//!
//! Pure projection over provider-supplied reasoning summaries and auditable
//! tool lifecycle records. Raw hidden chain-of-thought is never requested,
//! persisted, or shown: only caller-supplied summary deltas accumulate.
//! Events project into distinct parts reconciled to one persisted turn.
//! Bounded: event counts, payload sizes, summaries, references all capped.
//! Disconnect runs one cancel hook (upstream/provider cancel + turn-permit
//! release) exactly once.

/// Max provider/turn events projected into one turn.
pub const MAX_EVENTS: usize = 512;
/// Max bytes retained per text part (reasoning summary, answer).
pub const MAX_PART_BYTES: usize = 64 * 1024;
/// Max bytes retained across reasoning-summary accumulation.
pub const MAX_SUMMARY_BYTES: usize = 16 * 1024;
/// Max references retained per turn.
pub const MAX_REFERENCES: usize = 64;
/// Max bytes per reference label/target.
pub const MAX_REFERENCE_BYTES: usize = 1024;
/// Max tool-call display name bytes.
pub const MAX_TOOL_NAME_BYTES: usize = 128;
/// Max tool call-id bytes.
pub const MAX_CALL_ID_BYTES: usize = 128;

const SNAPSHOT_VERSION: u32 = 1;

/// Distinct rendered section of one assistant turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartKind {
    Reasoning,
    ToolActivity,
    Answer,
    References,
}

/// One projected section with owned text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnPart {
    /// Section identity.
    pub kind: PartKind,
    /// Rendered text (references part joins labels; never hidden CoT).
    pub text: String,
}

/// Bounded reference target. Labels are navigable source names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefTarget {
    Url(String),
    Path(String),
}

/// Provider/tool lifecycle records projected into parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityEvent {
    ToolStart { name: String, call_id: String },
    ToolEnd { call_id: String, ok: bool },
}

/// Stream events accepted by one turn. Unknown provider variants must map to
/// an explicit error at the call site, never silently into answer text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnEvent {
    ReasoningDelta { text: String },
    Activity(ActivityEvent),
    AnswerDelta { text: String },
    Reference { label: String, target: RefTarget },
}

/// Explicit bounded failures. Never fabricate answer content on error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityError {
    Malformed(&'static str),
    UnknownTool(String),
    TooLarge,
    TooManyEvents,
    WrongTurn,
    Unfinished,
    Snapshot(&'static str),
}

impl std::fmt::Display for ActivityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(name) => write!(f, "malformed turn event: {name}"),
            Self::UnknownTool(id) => write!(f, "unknown tool call: {id}"),
            Self::TooLarge => write!(f, "turn event exceeds byte bound"),
            Self::TooManyEvents => write!(f, "turn exceeds event bound"),
            Self::WrongTurn => write!(f, "event for wrong turn"),
            Self::Unfinished => write!(f, "turn not finished"),
            Self::Snapshot(reason) => write!(f, "invalid turn snapshot: {reason}"),
        }
    }
}

impl std::error::Error for ActivityError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolCall {
    name: String,
    call_id: String,
    done: bool,
    ok: bool,
}

/// Screen-reader projection: collapsed sections, live politeness, labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessibilityView {
    /// Reasoning `<details>` starts collapsed.
    pub reasoning_expanded: bool,
    /// Tool activity `<details>` starts collapsed.
    pub tools_expanded: bool,
    /// Final answer is the primary content, expanded.
    pub answer_expanded: bool,
    /// Streaming deltas never announce per-delta (`aria-live="off"`).
    pub live_politeness: &'static str,
    /// Meaningful control names.
    pub reasoning_label: &'static str,
    /// Meaningful control names.
    pub tools_label: &'static str,
    /// Meaningful section names.
    pub references_label: &'static str,
    /// Navigable references with labels + hrefs.
    pub references: Vec<AccessibleRef>,
}

/// One navigable reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessibleRef {
    /// Meaningful source label.
    pub label: String,
    /// Navigable href (url or path preserved verbatim, bounded).
    pub href: String,
}

/// One structured assistant turn. Owns only bounded caller bytes.
pub struct TurnParts {
    turn_id: String,
    reasoning: String,
    answer: String,
    tools: Vec<ToolCall>,
    references: Vec<(String, RefTarget)>,
    events: usize,
    finished: bool,
    cancelled: bool,
    cancel_hook: Option<Box<dyn FnOnce() + Send>>,
    hook_fired: bool,
}

impl TurnParts {
    /// Start projecting one persisted turn.
    #[must_use]
    pub fn new(turn_id: impl Into<String>) -> Self {
        Self {
            turn_id: turn_id.into(),
            reasoning: String::new(),
            answer: String::new(),
            tools: Vec::new(),
            references: Vec::new(),
            events: 0,
            finished: false,
            cancelled: false,
            cancel_hook: None,
            hook_fired: false,
        }
    }

    /// Register disconnect hook (upstream cancel + permit release). Fires once.
    pub fn set_cancel_hook(&mut self, hook: impl FnOnce() + Send + 'static) {
        self.cancel_hook = Some(Box::new(hook));
    }

    /// Browser disconnect: cancel upstream/provider work exactly once.
    pub fn disconnect(&mut self) {
        self.cancelled = true;
        if !self.hook_fired {
            self.hook_fired = true;
            if let Some(hook) = self.cancel_hook.take() {
                hook();
            }
        }
    }

    /// Whether disconnect was observed.
    #[must_use]
    pub fn cancelled(&self) -> bool {
        self.cancelled
    }

    /// Project one event into its distinct part. Errors never alter text.
    pub fn push(&mut self, event: TurnEvent) -> Result<(), ActivityError> {
        if self.events >= MAX_EVENTS {
            return Err(ActivityError::TooManyEvents);
        }
        match event {
            TurnEvent::ReasoningDelta { text } => {
                if text.is_empty() || text.len() > MAX_PART_BYTES {
                    return Err(if text.is_empty() {
                        ActivityError::Malformed("reasoning_delta")
                    } else {
                        ActivityError::TooLarge
                    });
                }
                if self.reasoning.len().saturating_add(text.len()) > MAX_SUMMARY_BYTES {
                    return Err(ActivityError::TooLarge);
                }
                self.reasoning.push_str(&text);
                self.events += 1;
                Ok(())
            }
            TurnEvent::Activity(ActivityEvent::ToolStart { name, call_id }) => {
                check_tool_id(&name, MAX_TOOL_NAME_BYTES, "tool name")?;
                check_tool_id(&call_id, MAX_CALL_ID_BYTES, "call_id")?;
                if self.tools.iter().any(|t| t.call_id == call_id) {
                    return Err(ActivityError::Malformed("duplicate call_id"));
                }
                if self.tools.len() >= MAX_EVENTS {
                    return Err(ActivityError::TooManyEvents);
                }
                self.tools.push(ToolCall {
                    name,
                    call_id,
                    done: false,
                    ok: false,
                });
                self.events += 1;
                Ok(())
            }
            TurnEvent::Activity(ActivityEvent::ToolEnd { call_id, ok }) => {
                check_tool_id(&call_id, MAX_CALL_ID_BYTES, "call_id")?;
                let Some(tool) = self.tools.iter_mut().find(|t| t.call_id == call_id) else {
                    return Err(ActivityError::UnknownTool(call_id));
                };
                tool.done = true;
                tool.ok = ok;
                self.events += 1;
                Ok(())
            }
            TurnEvent::AnswerDelta { text } => {
                if text.is_empty() {
                    return Err(ActivityError::Malformed("answer_delta"));
                }
                if self.answer.len().saturating_add(text.len()) > MAX_PART_BYTES {
                    return Err(ActivityError::TooLarge);
                }
                self.answer.push_str(&text);
                self.events += 1;
                Ok(())
            }
            TurnEvent::Reference { label, target } => {
                check_tool_id(&label, MAX_REFERENCE_BYTES, "reference label")?;
                let target_text = match &target {
                    RefTarget::Url(u) | RefTarget::Path(u) => u,
                };
                check_tool_id(target_text, MAX_REFERENCE_BYTES, "reference target")?;
                if label.trim().is_empty() || target_text.trim().is_empty() {
                    return Err(ActivityError::Malformed("reference"));
                }
                if self.references.len() >= MAX_REFERENCES {
                    return Err(ActivityError::TooManyEvents);
                }
                self.references.push((label, target));
                self.events += 1;
                Ok(())
            }
        }
    }

    /// Seal the turn for persistence.
    pub fn finish(&mut self, turn_id: &str) -> Result<(), ActivityError> {
        if turn_id != self.turn_id {
            return Err(ActivityError::WrongTurn);
        }
        self.finished = true;
        Ok(())
    }

    /// Provider-supplied reasoning summary (never hidden CoT).
    #[must_use]
    pub fn reasoning_summary(&self) -> &str {
        &self.reasoning
    }

    /// Final answer text, separate from reasoning/tool parts.
    #[must_use]
    pub fn answer(&self) -> &str {
        &self.answer
    }

    /// Navigable references with meaningful source labels.
    #[must_use]
    pub fn references(&self) -> &[(String, RefTarget)] {
        &self.references
    }

    /// Tool lifecycle states `(name, call_id, ok)`.
    #[must_use]
    pub fn tool_states(&self) -> Vec<(String, String, bool)> {
        self.tools
            .iter()
            .map(|t| (t.name.clone(), t.call_id.clone(), t.done && t.ok))
            .collect()
    }

    /// Distinct rendered parts in stable order.
    #[must_use]
    pub fn parts(&self) -> Vec<TurnPart> {
        let mut parts = Vec::with_capacity(4);
        parts.push(TurnPart {
            kind: PartKind::Reasoning,
            text: self.reasoning.clone(),
        });
        let mut activity = String::new();
        for tool in &self.tools {
            let state = if tool.done {
                if tool.ok {
                    "ok"
                } else {
                    "error"
                }
            } else {
                "running"
            };
            activity.push_str(&tool.name);
            activity.push(' ');
            activity.push_str(&tool.call_id);
            activity.push(' ');
            activity.push_str(state);
            activity.push('\n');
        }
        parts.push(TurnPart {
            kind: PartKind::ToolActivity,
            text: activity,
        });
        parts.push(TurnPart {
            kind: PartKind::Answer,
            text: self.answer.clone(),
        });
        let mut refs = String::new();
        for (label, target) in &self.references {
            refs.push_str(label);
            refs.push_str(": ");
            match target {
                RefTarget::Url(u) | RefTarget::Path(u) => refs.push_str(u),
            }
            refs.push('\n');
        }
        parts.push(TurnPart {
            kind: PartKind::References,
            text: refs,
        });
        parts
    }

    /// Collapsed-control a11y projection. Deltas never live-announced.
    #[must_use]
    pub fn accessibility(&self) -> AccessibilityView {
        AccessibilityView {
            reasoning_expanded: false,
            tools_expanded: false,
            answer_expanded: true,
            live_politeness: "off",
            reasoning_label: "Reasoning",
            tools_label: "Tool activity",
            references_label: "References",
            references: self
                .references
                .iter()
                .map(|(label, target)| AccessibleRef {
                    label: label.clone(),
                    href: match target {
                        RefTarget::Url(u) | RefTarget::Path(u) => u.clone(),
                    },
                })
                .collect(),
        }
    }

    /// Persist the turn: final answer, summary, tool states, references.
    /// No hidden CoT field exists, so none can be stored.
    pub fn snapshot(&self) -> Result<String, ActivityError> {
        if !self.finished {
            return Err(ActivityError::Unfinished);
        }
        let mut out = String::from("{\"version\":");
        out.push_str(&SNAPSHOT_VERSION.to_string());
        out.push_str(",\"turn_id\":");
        out.push_str(&json_string(&self.turn_id));
        out.push_str(",\"reasoning_summary\":");
        out.push_str(&json_string(&self.reasoning));
        out.push_str(",\"answer\":");
        out.push_str(&json_string(&self.answer));
        out.push_str(",\"tools\":[");
        for (i, tool) in self.tools.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str("{\"name\":");
            out.push_str(&json_string(&tool.name));
            out.push_str(",\"call_id\":");
            out.push_str(&json_string(&tool.call_id));
            out.push_str(",\"done\":");
            out.push_str(if tool.done { "true" } else { "false" });
            out.push_str(",\"ok\":");
            out.push_str(if tool.ok { "true" } else { "false" });
            out.push('}');
        }
        out.push_str("],\"references\":[");
        for (i, (label, target)) in self.references.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str("{\"label\":");
            out.push_str(&json_string(label));
            match target {
                RefTarget::Url(u) => {
                    out.push_str(",\"url\":");
                    out.push_str(&json_string(u));
                }
                RefTarget::Path(p) => {
                    out.push_str(",\"path\":");
                    out.push_str(&json_string(p));
                }
            }
            out.push('}');
        }
        out.push_str("]}");
        Ok(out)
    }

    /// Reload reproduces answer, summary, tool states, references.
    pub fn from_snapshot(snapshot: &str) -> Result<Self, ActivityError> {
        if snapshot.len() > MAX_EVENTS * 256 {
            return Err(ActivityError::Snapshot("too large"));
        }
        let value: serde_json::Value =
            serde_json::from_str(snapshot).map_err(|_| ActivityError::Snapshot("invalid json"))?;
        let object = value.as_object().ok_or(ActivityError::Snapshot("object"))?;
        if object.get("version").and_then(serde_json::Value::as_u64)
            != Some(u64::from(SNAPSHOT_VERSION))
        {
            return Err(ActivityError::Snapshot("version"));
        }
        // Reject any hidden-CoT shaped payload outright.
        if object.contains_key("chain_of_thought") || object.contains_key("hidden") {
            return Err(ActivityError::Snapshot("hidden reasoning"));
        }
        let turn_id = object
            .get("turn_id")
            .and_then(serde_json::Value::as_str)
            .ok_or(ActivityError::Snapshot("turn_id"))?;
        let reasoning = object
            .get("reasoning_summary")
            .and_then(serde_json::Value::as_str)
            .ok_or(ActivityError::Snapshot("reasoning_summary"))?;
        let answer = object
            .get("answer")
            .and_then(serde_json::Value::as_str)
            .ok_or(ActivityError::Snapshot("answer"))?;
        if reasoning.len() > MAX_SUMMARY_BYTES || answer.len() > MAX_PART_BYTES {
            return Err(ActivityError::Snapshot("too large"));
        }
        let mut turn = TurnParts::new(turn_id);
        turn.reasoning.push_str(reasoning);
        turn.answer.push_str(answer);
        for tool in object
            .get("tools")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default()
        {
            let name = tool
                .get("name")
                .and_then(serde_json::Value::as_str)
                .ok_or(ActivityError::Snapshot("tool name"))?;
            let call_id = tool
                .get("call_id")
                .and_then(serde_json::Value::as_str)
                .ok_or(ActivityError::Snapshot("call_id"))?;
            check_tool_id(name, MAX_TOOL_NAME_BYTES, "tool name")?;
            check_tool_id(call_id, MAX_CALL_ID_BYTES, "call_id")?;
            let done = tool
                .get("done")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let ok = tool
                .get("ok")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            turn.tools.push(ToolCall {
                name: name.to_owned(),
                call_id: call_id.to_owned(),
                done,
                ok,
            });
        }
        if turn.tools.len() > MAX_EVENTS {
            return Err(ActivityError::Snapshot("too many tools"));
        }
        for reference in object
            .get("references")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default()
        {
            let label = reference
                .get("label")
                .and_then(serde_json::Value::as_str)
                .ok_or(ActivityError::Snapshot("label"))?;
            check_tool_id(label, MAX_REFERENCE_BYTES, "reference label")?;
            if label.trim().is_empty() {
                return Err(ActivityError::Snapshot("label"));
            }
            if let Some(url) = reference.get("url").and_then(serde_json::Value::as_str) {
                check_tool_id(url, MAX_REFERENCE_BYTES, "reference target")?;
                turn.references
                    .push((label.to_owned(), RefTarget::Url(url.to_owned())));
            } else if let Some(path) = reference.get("path").and_then(serde_json::Value::as_str) {
                check_tool_id(path, MAX_REFERENCE_BYTES, "reference target")?;
                turn.references
                    .push((label.to_owned(), RefTarget::Path(path.to_owned())));
            } else {
                return Err(ActivityError::Snapshot("target"));
            }
        }
        if turn.references.len() > MAX_REFERENCES {
            return Err(ActivityError::Snapshot("too many references"));
        }
        turn.events = turn
            .tools
            .len()
            .saturating_add(turn.references.len())
            .min(MAX_EVENTS);
        turn.finished = true;
        Ok(turn)
    }
}

fn check_tool_id(value: &str, max: usize, what: &'static str) -> Result<(), ActivityError> {
    if value.is_empty() || value.len() > max {
        return Err(if value.is_empty() {
            ActivityError::Malformed(what)
        } else {
            ActivityError::TooLarge
        });
    }
    Ok(())
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
