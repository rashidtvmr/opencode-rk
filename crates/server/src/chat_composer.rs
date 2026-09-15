//! WEB-010: structured WYSIWYG chat composer lowering (server-side contract).
//!
//! Pure synchronous boundary between the browser composer and the native turn
//! request contract. A structured [`ComposerDoc`] (paragraphs, code blocks,
//! mentions, slash commands, capability chips) lowers deterministically to the
//! `(provider/model, effort, text)` triple the turn pipeline consumes.
//!
//! Safety properties: arbitrary pasted HTML and unsupported nodes never lower
//! (they must pass [`sanitize_doc`] first); inactive capability chips cannot be
//! sent as if active; every bound is checked before allocation growth; error
//! values carry field/kind names only, never input text.

#![forbid(unsafe_code)]

/// Maximum lowered turn text bytes (matches 64 KiB control-plane cap).
pub const MAX_COMPOSER_TEXT_BYTES: usize = 64 * 1024;
/// Maximum document nodes lowered or drafted in one composer state.
pub const MAX_DOC_NODES: usize = 256;
/// Maximum encoded draft bytes accepted by [`decode_draft`].
pub const MAX_DRAFT_BYTES: usize = 64 * 1024;
/// Maximum queued/steered drafts retained in [`DraftQueue`].
pub const MAX_QUEUE_DRAFTS: usize = 16;

/// Supported reasoning efforts, matching the native turn contract.
const EFFORTS: [&str; 6] = ["none", "minimal", "low", "medium", "high", "xhigh"];

/// One structured composer node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposerNode {
    /// Plain multiline paragraph.
    Paragraph { text: String },
    /// Fenced code block with language tag.
    CodeBlock { lang: String, text: String },
    /// User mention (`@target`, human-readable `label`).
    Mention { target: String, label: String },
    /// Slash command (`/name args`).
    Command { name: String, args: String },
    /// Raw pasted HTML. Never lowers; [`sanitize_doc`] drops it.
    Html { html: String },
    /// Unsupported attachment/embed. Never lowers; [`sanitize_doc`] drops it.
    Embed { kind: String, label: String },
    /// Tool capability chip. Lowers only when `active`.
    ToolChip { tool: String, active: bool },
    /// Plugin capability chip. Lowers only when `active`.
    PluginChip { plugin: String, active: bool },
}

/// Structured composer state plus model/effort selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerDoc {
    pub nodes: Vec<ComposerNode>,
    /// `provider/model` selector, e.g. `openai/gpt-4o-mini`.
    pub model: String,
    /// One of `none|minimal|low|medium|high|xhigh`.
    pub effort: String,
}

/// Composer failures. Variants carry kind/field names and sizes only, never
/// input text, so `Display` output is safe to log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposerError {
    EmptyDocument,
    ModelInvalid,
    EffortInvalid,
    UnsupportedNode(&'static str),
    InactiveCapability(&'static str),
    NodeLimitExceeded { max: usize, actual: usize },
    DocTooLarge { max: usize, actual: usize },
    NodeTextTooLarge { max: usize, actual: usize },
    QueueFull { max: usize, actual: usize },
    DraftNotFound,
    MalformedDraft(&'static str),
    DraftTooLarge { max: usize, actual: usize },
}

impl std::fmt::Display for ComposerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyDocument => write!(f, "empty document"),
            Self::ModelInvalid => write!(f, "invalid model selector"),
            Self::EffortInvalid => write!(f, "invalid effort selector"),
            Self::UnsupportedNode(kind) => write!(f, "unsupported node: {kind}"),
            Self::InactiveCapability(kind) => write!(f, "inactive capability: {kind}"),
            Self::NodeLimitExceeded { max, actual } => {
                write!(f, "too many nodes: max {max}, got {actual}")
            }
            Self::DocTooLarge { max, actual } => {
                write!(f, "document too large: max {max}, got {actual}")
            }
            Self::NodeTextTooLarge { max, actual } => {
                write!(f, "node text too large: max {max}, got {actual}")
            }
            Self::QueueFull { max, actual } => {
                write!(f, "draft queue full: max {max}, got {actual}")
            }
            Self::DraftNotFound => write!(f, "draft not found"),
            Self::MalformedDraft(field) => write!(f, "malformed draft: {field}"),
            Self::DraftTooLarge { max, actual } => {
                write!(f, "draft too large: max {max}, got {actual}")
            }
        }
    }
}

impl std::error::Error for ComposerError {}

/// Lowered native turn request: plain text plus routing selectors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredTurn {
    pub text: String,
    pub provider: String,
    pub model: String,
    pub effort: String,
}

/// Result of [`sanitize_doc`]: the sendable document plus dropped-node count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedDoc {
    pub doc: ComposerDoc,
    pub dropped: usize,
}

/// Result of [`decode_draft`]: the recovered document plus dropped-node count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedDraft {
    pub doc: ComposerDoc,
    pub dropped: usize,
}

/// Keyboard commands the composer must honor (T03: no focus traps).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    InsertNewline,
    Send,
    Undo,
    Redo,
    OpenMenu,
    ShowHelp,
    MoveFocusNext,
    CloseMenu,
}

/// Bounded owner-tagged draft queue plus the live-turn flag.
///
/// `push` appends a queued draft, `steer` replaces the calling owner's queued
/// text in place (ownership is deterministic: owners only touch their own
/// slot), `begin_live`/`stop` bracket the single live turn.
#[derive(Debug, Default)]
pub struct DraftQueue {
    drafts: Vec<(String, String)>,
    live: bool,
}

impl DraftQueue {
    /// Empty queue, no live turn.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Queued draft count.
    pub fn len(&self) -> usize {
        self.drafts.len()
    }

    /// True when no drafts are queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.drafts.is_empty()
    }

    /// Append an owner-tagged draft. Bounded by [`MAX_QUEUE_DRAFTS`].
    pub fn push(&mut self, owner: &str, text: &str) -> Result<(), ComposerError> {
        if self.drafts.len() >= MAX_QUEUE_DRAFTS {
            return Err(ComposerError::QueueFull {
                max: MAX_QUEUE_DRAFTS,
                actual: self.drafts.len(),
            });
        }
        self.drafts
            .push((owner.to_string(), text.to_string()));
        Ok(())
    }

    /// Replace one owner's queued text in place; queue order is preserved.
    pub fn steer(&mut self, owner: &str, text: &str) -> Result<(), ComposerError> {
        let slot = self
            .drafts
            .iter_mut()
            .find(|(name, _)| name == owner)
            .ok_or(ComposerError::DraftNotFound)?;
        slot.1 = text.to_string();
        Ok(())
    }

    /// Mark the live turn as running for `_owner`.
    pub fn begin_live(&mut self, _owner: &str) {
        self.live = true;
    }

    /// Abort the live turn. True only when a live turn was running.
    pub fn stop(&mut self) -> bool {
        std::mem::replace(&mut self.live, false)
    }

    /// True while a live turn is running.
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.live
    }

    /// Owner tag at queue position `index`.
    #[must_use]
    pub fn owner_at(&self, index: usize) -> Option<&str> {
        self.drafts.get(index).map(|(owner, _)| owner.as_str())
    }

    /// Queued text for `owner`.
    #[must_use]
    pub fn text_of(&self, owner: &str) -> Option<&str> {
        self.drafts
            .iter()
            .find(|(name, _)| name == owner)
            .map(|(_, text)| text.as_str())
    }
}

/// Lower one node to its plain-text fragment.
fn lower_node(node: &ComposerNode) -> Result<String, ComposerError> {
    match node {
        ComposerNode::Paragraph { text } => Ok(text.clone()),
        ComposerNode::CodeBlock { lang, text } => {
            Ok(format!("```{lang}\n{text}\n```"))
        }
        ComposerNode::Mention { target, .. } => Ok(format!("@{target}")),
        ComposerNode::Command { name, args } => {
            if args.trim().is_empty() {
                Ok(format!("/{name}"))
            } else {
                Ok(format!("/{name} {args}"))
            }
        }
        ComposerNode::Html { .. } => Err(ComposerError::UnsupportedNode("html")),
        ComposerNode::Embed { .. } => Err(ComposerError::UnsupportedNode("attachment")),
        ComposerNode::ToolChip { tool, active } => {
            if *active {
                Ok(format!("[tool:{tool}]"))
            } else {
                Err(ComposerError::InactiveCapability("tool"))
            }
        }
        ComposerNode::PluginChip { plugin, active } => {
            if *active {
                Ok(format!("[plugin:{plugin}]"))
            } else {
                Err(ComposerError::InactiveCapability("plugin"))
            }
        }
    }
}

/// Split a `provider/model` selector. Both halves must be non-empty.
fn split_model(model: &str) -> Option<(&str, &str)> {
    model
        .split_once('/')
        .filter(|(provider, name)| !provider.is_empty() && !name.is_empty())
}

/// Lower a structured composer document to the native turn contract.
///
/// Deterministic: same document always yields the same turn or the same error.
/// Checks node count first, then per-node support/capability, then emptiness,
/// then selectors, then byte bounds.
pub fn lower_composer_doc(doc: &ComposerDoc) -> Result<LoweredTurn, ComposerError> {
    if doc.nodes.len() > MAX_DOC_NODES {
        return Err(ComposerError::NodeLimitExceeded {
            max: MAX_DOC_NODES,
            actual: doc.nodes.len(),
        });
    }
    let mut parts = Vec::with_capacity(doc.nodes.len());
    for node in &doc.nodes {
        let part = lower_node(node)?;
        if part.len() > MAX_COMPOSER_TEXT_BYTES {
            return Err(ComposerError::NodeTextTooLarge {
                max: MAX_COMPOSER_TEXT_BYTES,
                actual: part.len(),
            });
        }
        parts.push(part);
    }
    let text = parts.join("\n");
    if text.trim().is_empty() {
        return Err(ComposerError::EmptyDocument);
    }
    let Some((provider, model)) = split_model(&doc.model) else {
        return Err(ComposerError::ModelInvalid);
    };
    if !EFFORTS.contains(&doc.effort.as_str()) {
        return Err(ComposerError::EffortInvalid);
    }
    if text.len() > MAX_COMPOSER_TEXT_BYTES {
        return Err(ComposerError::DocTooLarge {
            max: MAX_COMPOSER_TEXT_BYTES,
            actual: text.len(),
        });
    }
    Ok(LoweredTurn {
        text,
        provider: provider.to_string(),
        model: model.to_string(),
        effort: doc.effort.clone(),
    })
}

/// Drop non-lowerable nodes (raw HTML, unsupported embeds, inactive chips) and
/// keep the sendable remainder with the same selectors. `dropped` counts the
/// removed nodes.
pub fn sanitize_doc(doc: &ComposerDoc) -> SanitizedDoc {
    let mut kept = Vec::new();
    let mut dropped = 0usize;
    for node in &doc.nodes {
        let sendable = match node {
            ComposerNode::Paragraph { .. }
            | ComposerNode::CodeBlock { .. }
            | ComposerNode::Mention { .. }
            | ComposerNode::Command { .. } => true,
            ComposerNode::Html { .. } | ComposerNode::Embed { .. } => false,
            ComposerNode::ToolChip { active, .. }
            | ComposerNode::PluginChip { active, .. } => *active,
        };
        if sendable {
            kept.push(node.clone());
        } else {
            dropped += 1;
        }
    }
    SanitizedDoc {
        doc: ComposerDoc {
            nodes: kept,
            model: doc.model.clone(),
            effort: doc.effort.clone(),
        },
        dropped,
    }
}

/// Screen-reader label for one composer node. Never empty.
pub fn accessible_label(node: &ComposerNode) -> String {
    match node {
        ComposerNode::Paragraph { text } => {
            let head: String = text.chars().take(40).collect();
            if head.trim().is_empty() {
                "Paragraph (empty)".to_string()
            } else {
                format!("Paragraph: {head}")
            }
        }
        ComposerNode::CodeBlock { lang, text } => {
            let head: String = text.chars().take(40).collect();
            format!("Code block {lang}: {head}")
        }
        ComposerNode::Mention { target, label } => format!("Mention {label} (@{target})"),
        ComposerNode::Command { name, args } => {
            if args.trim().is_empty() {
                format!("Command /{name}")
            } else {
                format!("Command /{name} {args}")
            }
        }
        ComposerNode::Html { .. } => "Pasted content (sanitized)".to_string(),
        ComposerNode::Embed { kind, label } => format!("Attachment {label} ({kind})"),
        ComposerNode::ToolChip { tool, active } => {
            format!("Tool {tool} ({})", if *active { "active" } else { "inactive" })
        }
        ComposerNode::PluginChip { plugin, active } => {
            format!(
                "Plugin {plugin} ({})",
                if *active { "active" } else { "inactive" }
            )
        }
    }
}

/// Discoverable shortcut help. Names every keyboard command from [`key_command`].
pub fn editor_help() -> &'static str {
    "Composer shortcuts: Enter inserts a newline, Ctrl+Enter sends, \
     Ctrl+Z Undo, Ctrl+Shift+Z or Ctrl+Y Redo, Ctrl+K opens the command menu, \
     Ctrl+/ shows this help, Tab moves focus to the next control, \
     Escape closes the open menu and returns focus."
}

/// Map a key press to its composer command. `ctrl`/`shift` are modifiers.
///
/// Tab always moves focus (no traps); Escape always closes the menu; plain
/// Enter inserts a newline while Ctrl+Enter sends.
pub fn key_command(key: &str, ctrl: bool, shift: bool) -> KeyAction {
    if key.eq_ignore_ascii_case("escape") {
        return KeyAction::CloseMenu;
    }
    if ctrl && key.eq_ignore_ascii_case("enter") {
        return KeyAction::Send;
    }
    if key.eq_ignore_ascii_case("enter") {
        return KeyAction::InsertNewline;
    }
    if ctrl && key.eq_ignore_ascii_case("z") && shift {
        return KeyAction::Redo;
    }
    if ctrl && key.eq_ignore_ascii_case("z") {
        return KeyAction::Undo;
    }
    if ctrl && key.eq_ignore_ascii_case("y") {
        return KeyAction::Redo;
    }
    if ctrl && key.eq_ignore_ascii_case("k") {
        return KeyAction::OpenMenu;
    }
    if ctrl && key == "/" {
        return KeyAction::ShowHelp;
    }
    if key.eq_ignore_ascii_case("tab") {
        return KeyAction::MoveFocusNext;
    }
    KeyAction::CloseMenu
}

/// Encode a composer document as a draft string for preservation.
pub fn encode_draft(doc: &ComposerDoc) -> Result<String, ComposerError> {
    let nodes: Vec<serde_json::Value> = doc.nodes.iter().map(encode_node).collect();
    let value = serde_json::json!({
        "model": doc.model,
        "effort": doc.effort,
        "nodes": nodes,
    });
    serde_json::to_string(&value).map_err(|_| ComposerError::MalformedDraft("draft"))
}

/// Recover a preserved draft. Unknown node kinds are dropped and counted so
/// newer documents degrade gracefully; known nodes with wrong shapes and
/// non-object envelopes are [`ComposerError::MalformedDraft`]. The cap is
/// checked before parsing.
pub fn decode_draft(encoded: &str) -> Result<DecodedDraft, ComposerError> {
    if encoded.len() > MAX_DRAFT_BYTES {
        return Err(ComposerError::DraftTooLarge {
            max: MAX_DRAFT_BYTES,
            actual: encoded.len(),
        });
    }
    let value: serde_json::Value =
        serde_json::from_str(encoded).map_err(|_| ComposerError::MalformedDraft("draft"))?;
    let object = value
        .as_object()
        .ok_or(ComposerError::MalformedDraft("draft"))?;
    let model = object
        .get("model")
        .and_then(serde_json::Value::as_str)
        .ok_or(ComposerError::MalformedDraft("model"))?;
    let effort = object
        .get("effort")
        .and_then(serde_json::Value::as_str)
        .ok_or(ComposerError::MalformedDraft("effort"))?;
    if split_model(model).is_none() {
        return Err(ComposerError::ModelInvalid);
    }
    if !EFFORTS.contains(&effort) {
        return Err(ComposerError::EffortInvalid);
    }
    let nodes = object
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .ok_or(ComposerError::MalformedDraft("nodes"))?;
    if nodes.len() > MAX_DOC_NODES {
        return Err(ComposerError::NodeLimitExceeded {
            max: MAX_DOC_NODES,
            actual: nodes.len(),
        });
    }
    let mut out = Vec::with_capacity(nodes.len());
    let mut dropped = 0usize;
    for node in nodes {
        match decode_node(node)? {
            Some(node) => out.push(node),
            None => dropped += 1,
        }
    }
    Ok(DecodedDraft {
        doc: ComposerDoc {
            nodes: out,
            model: model.to_string(),
            effort: effort.to_string(),
        },
        dropped,
    })
}

/// Wrap older plain-text input as a one-paragraph compatible document.
pub fn doc_from_legacy_text(
    text: &str,
    model: &str,
    effort: &str,
) -> Result<ComposerDoc, ComposerError> {
    if text.trim().is_empty() {
        return Err(ComposerError::EmptyDocument);
    }
    if split_model(model).is_none() {
        return Err(ComposerError::ModelInvalid);
    }
    if !EFFORTS.contains(&effort) {
        return Err(ComposerError::EffortInvalid);
    }
    Ok(ComposerDoc {
        nodes: vec![ComposerNode::Paragraph {
            text: text.to_string(),
        }],
        model: model.to_string(),
        effort: effort.to_string(),
    })
}

/// Encode one node with its `kind` discriminator.
fn encode_node(node: &ComposerNode) -> serde_json::Value {
    match node {
        ComposerNode::Paragraph { text } => {
            serde_json::json!({"kind": "paragraph", "text": text})
        }
        ComposerNode::CodeBlock { lang, text } => {
            serde_json::json!({"kind": "code", "lang": lang, "text": text})
        }
        ComposerNode::Mention { target, label } => {
            serde_json::json!({"kind": "mention", "target": target, "label": label})
        }
        ComposerNode::Command { name, args } => {
            serde_json::json!({"kind": "command", "name": name, "args": args})
        }
        ComposerNode::Html { html } => {
            serde_json::json!({"kind": "html", "html": html})
        }
        ComposerNode::Embed { kind, label } => {
            serde_json::json!({"kind": "embed", "mime": kind, "label": label})
        }
        ComposerNode::ToolChip { tool, active } => {
            serde_json::json!({"kind": "tool", "tool": tool, "active": active})
        }
        ComposerNode::PluginChip { plugin, active } => {
            serde_json::json!({"kind": "plugin", "plugin": plugin, "active": active})
        }
    }
}

/// Required string field of a draft node object.
fn draft_string(
    object: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Result<String, ComposerError> {
    object
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or(ComposerError::MalformedDraft(field))
}

/// Decode one draft node. `Ok(None)` means "unknown kind: drop and count".
fn decode_node(
    value: &serde_json::Value,
) -> Result<Option<ComposerNode>, ComposerError> {
    let object = value
        .as_object()
        .ok_or(ComposerError::MalformedDraft("nodes"))?;
    let kind = object
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .ok_or(ComposerError::MalformedDraft("nodes"))?;
    let node = match kind {
        "paragraph" => ComposerNode::Paragraph {
            text: draft_string(object, "text")?,
        },
        "code" => ComposerNode::CodeBlock {
            lang: draft_string(object, "lang")?,
            text: draft_string(object, "text")?,
        },
        "mention" => ComposerNode::Mention {
            target: draft_string(object, "target")?,
            label: draft_string(object, "label")?,
        },
        "command" => ComposerNode::Command {
            name: draft_string(object, "name")?,
            args: draft_string(object, "args")?,
        },
        "tool" => ComposerNode::ToolChip {
            tool: draft_string(object, "tool")?,
            active: object
                .get("active")
                .and_then(serde_json::Value::as_bool)
                .ok_or(ComposerError::MalformedDraft("tool"))?,
        },
        "plugin" => ComposerNode::PluginChip {
            plugin: draft_string(object, "plugin")?,
            active: object
                .get("active")
                .and_then(serde_json::Value::as_bool)
                .ok_or(ComposerError::MalformedDraft("plugin"))?,
        },
        // Raw HTML and embeds never round-trip as sendable nodes; drafts that
        // still carry them degrade by dropping, matching reload-after-sanitize.
        "html" | "embed" => return Ok(None),
        _ => return Ok(None),
    };
    Ok(Some(node))
}
