//! Pure TUI interaction state machines (UI-014..UI-018).
//!
//! No rendering, no IO, no clock, no threads. Caller supplies all inputs.

use thiserror::Error;

// ---------- shared bounds ----------

/// Max drafts waiting while the composer is busy (UI-014).
pub const MAX_QUEUED: usize = 8;
/// Max draft bytes retained (UI-014).
pub const MAX_DRAFT_BYTES: usize = 32_768;
/// Max model entries shown in the switcher (UI-015).
pub const MAX_SWITCHER_ENTRIES: usize = 64;
/// Max per-source rows shown in context detail (UI-016).
pub const MAX_SOURCES: usize = 64;
/// Max memory files retained in the viewer (UI-017).
pub const MAX_MEMORY_FILES: usize = 64;
/// Max footer hints rendered (UI-018).
pub const MAX_HINTS: usize = 8;

// ---------- UI-014 composer ----------

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComposerKey {
    Enter,
    ShiftEnter,
    CtrlJ,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitKeymap {
    Enter,
    CtrlJ,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComposerAction {
    Submit,
    Newline,
}

#[must_use]
pub const fn decide_composer_key(key: ComposerKey, keymap: SubmitKeymap) -> ComposerAction {
    match (keymap, key) {
        (SubmitKeymap::Enter, ComposerKey::Enter) => ComposerAction::Submit,
        (SubmitKeymap::CtrlJ, ComposerKey::CtrlJ) => ComposerAction::Submit,
        _ => ComposerAction::Newline,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubmitOutcome {
    Sent(String),
    Queued,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ComposerError {
    #[error("draft is empty; nothing to send")]
    EmptyDraft,
    #[error("draft exceeds {MAX_DRAFT_BYTES} bytes")]
    DraftTooLong,
    #[error("queued-submit limit reached ({limit})")]
    QueueFull { limit: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Composer {
    draft: String,
    busy: bool,
    queue: Vec<String>,
}

impl Composer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            draft: String::new(),
            busy: false,
            queue: Vec::new(),
        }
    }

    pub fn set_draft(&mut self, text: impl Into<String>) -> Result<(), ComposerError> {
        let text = text.into();
        if text.len() > MAX_DRAFT_BYTES {
            return Err(ComposerError::DraftTooLong);
        }
        self.draft = text;
        Ok(())
    }

    #[must_use]
    pub fn draft(&self) -> &str {
        &self.draft
    }

    #[must_use]
    pub fn can_send(&self) -> bool {
        !self.draft.trim().is_empty()
    }

    #[must_use]
    pub const fn is_busy(&self) -> bool {
        self.busy
    }

    #[must_use]
    pub fn queued(&self) -> &[String] {
        &self.queue
    }

    /// Idle + non-empty draft sends immediately and marks busy.
    /// Busy + non-empty draft queues (bounded). Draft text is preserved
    /// in both cases for the interrupt path.
    pub fn submit(&mut self) -> Result<SubmitOutcome, ComposerError> {
        if !self.can_send() {
            return Err(ComposerError::EmptyDraft);
        }
        if !self.busy {
            let sent = std::mem::take(&mut self.draft);
            self.busy = true;
            return Ok(SubmitOutcome::Sent(sent));
        }
        if self.queue.len() >= MAX_QUEUED {
            return Err(ComposerError::QueueFull { limit: MAX_QUEUED });
        }
        self.queue.push(self.draft.clone());
        Ok(SubmitOutcome::Queued)
    }

    /// Abort the in-flight turn. Draft and queue survive.
    pub fn interrupt(&mut self) {
        self.busy = false;
    }

    /// Pop the next queued draft after a turn finishes. Returns `None`
    /// when idle or the queue is empty.
    pub fn finish_turn(&mut self) -> Option<String> {
        if self.queue.is_empty() {
            self.busy = false;
            return None;
        }
        let next = self.queue.remove(0);
        self.busy = true;
        Some(next)
    }
}

impl Default for Composer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------- UI-015 status bar ----------

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusItem {
    Model,
    Context,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusAction {
    OpenModelSwitcher,
    OpenContextDetail,
}

#[must_use]
pub const fn status_click(item: StatusItem) -> StatusAction {
    match item {
        StatusItem::Model => StatusAction::OpenModelSwitcher,
        StatusItem::Context => StatusAction::OpenContextDetail,
    }
}

/// Keyboard equivalents for pointer-only status-bar clicks.
#[must_use]
pub fn keyboard_fallback(key: &str) -> Option<StatusAction> {
    match key {
        "ctrl-p" => Some(StatusAction::OpenModelSwitcher),
        "ctrl-t" => Some(StatusAction::OpenContextDetail),
        _ => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelEntry {
    pub name: String,
    pub provider: String,
    pub effort: String,
}

impl ModelEntry {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        provider: impl Into<String>,
        effort: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            provider: provider.into(),
            effort: effort.into(),
        }
    }
}

/// Provider-aware switcher list. Returns entries plus an explicit
/// truncation flag (never silent).
#[must_use]
pub fn filter_models(models: &[ModelEntry], provider: &str) -> (Vec<ModelEntry>, bool) {
    let mut out: Vec<ModelEntry> = models
        .iter()
        .filter(|m| m.provider == provider)
        .cloned()
        .collect();
    let truncated = out.len() > MAX_SWITCHER_ENTRIES;
    out.truncate(MAX_SWITCHER_ENTRIES);
    (out, truncated)
}

// ---------- UI-016 context detail ----------

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceUsage {
    pub source: String,
    pub tokens: u64,
    pub cached: bool,
}

impl SourceUsage {
    #[must_use]
    pub fn new(source: impl Into<String>, tokens: u64, cached: bool) -> Self {
        Self {
            source: source.into(),
            tokens,
            cached,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextBreakdown {
    pub entries: Vec<SourceUsage>,
    pub truncated: bool,
}

/// Largest-first ordering with explicit truncation marker.
#[must_use]
pub fn context_breakdown(mut sources: Vec<SourceUsage>) -> ContextBreakdown {
    sources.sort_by(|a, b| b.tokens.cmp(&a.tokens));
    let truncated = sources.len() > MAX_SOURCES;
    sources.truncate(MAX_SOURCES);
    ContextBreakdown {
        entries: sources,
        truncated,
    }
}

// ---------- UI-017 /memory viewer ----------

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryFile {
    pub path: String,
    pub bytes: u64,
    pub tokens: u64,
    pub cached: bool,
}

impl MemoryFile {
    #[must_use]
    pub fn new(path: impl Into<String>, bytes: u64, tokens: u64, cached: bool) -> Self {
        Self {
            path: path.into(),
            bytes,
            tokens,
            cached,
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum MemoryError {
    #[error("memory file not found: {0}")]
    NotFound(String),
    #[error("invalid memory path: {0}")]
    InvalidPath(String),
    #[error("memory list reached its retained-file limit ({MAX_MEMORY_FILES})")]
    LimitReached,
}

pub fn validate_memory_path(path: &str) -> Result<(), MemoryError> {
    if path.is_empty() || path.starts_with('/') || path.contains("..") {
        return Err(MemoryError::InvalidPath(path.to_owned()));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryViewer {
    files: Vec<MemoryFile>,
}

impl MemoryViewer {
    #[must_use]
    pub fn new(files: Vec<MemoryFile>) -> Self {
        let mut files = files;
        files.truncate(MAX_MEMORY_FILES);
        Self { files }
    }

    #[must_use]
    pub fn list(&self) -> &[MemoryFile] {
        &self.files
    }

    /// Unload a file; cached files carry a prefix-cache cost warning.
    pub fn unload(&mut self, path: &str) -> Result<String, MemoryError> {
        let pos = self
            .files
            .iter()
            .position(|f| f.path == path)
            .ok_or_else(|| MemoryError::NotFound(path.to_owned()))?;
        let removed = self.files.remove(pos);
        if removed.cached {
            Ok(format!(
                "Unloaded {} ({} tokens were prefix-cached; cache cost lost, next load is fresh).",
                removed.path, removed.tokens
            ))
        } else {
            Ok(format!("Unloaded {}.", removed.path))
        }
    }

    pub fn reload(&mut self, file: MemoryFile) -> Result<(), MemoryError> {
        validate_memory_path(&file.path)?;
        if self.files.len() >= MAX_MEMORY_FILES {
            return Err(MemoryError::LimitReached);
        }
        self.files.push(file);
        Ok(())
    }
}

// ---------- UI-018 keybindings ----------

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyHint {
    pub keys: String,
    pub action: String,
}

impl KeyHint {
    #[must_use]
    pub fn new(keys: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            keys: keys.into(),
            action: action.into(),
        }
    }
}

/// Footer hint list for the active submit keymap. Bounded by MAX_HINTS.
#[must_use]
pub fn footer_hints(keymap: SubmitKeymap) -> Vec<KeyHint> {
    let (submit, newline) = match keymap {
        SubmitKeymap::Enter => ("Enter", "Shift+Enter / Ctrl+J"),
        SubmitKeymap::CtrlJ => ("Ctrl+J", "Enter"),
    };
    let mut hints = vec![
        KeyHint::new(submit, "submit"),
        KeyHint::new(newline, "newline"),
        KeyHint::new("Ctrl+P", "model switcher"),
        KeyHint::new("Ctrl+T", "context detail"),
        KeyHint::new("?", "keybindings help"),
    ];
    hints.truncate(MAX_HINTS);
    hints
}

/// Full help text; names both bindings and states the active submit key.
#[must_use]
pub fn keybinding_help(keymap: SubmitKeymap) -> String {
    let active = match keymap {
        SubmitKeymap::Enter => "Enter",
        SubmitKeymap::CtrlJ => "Ctrl+J",
    };
    format!(
        "Keybindings (submit: {active}):\n\
         - {active}: submit composer draft\n\
         - Enter: submit (Enter keymap) / newline (Ctrl+J keymap)\n\
         - Shift+Enter / Ctrl+J: insert newline (Enter keymap)\n\
         - Ctrl+P: open model switcher\n\
         - Ctrl+T: open context detail\n\
         - ?: show this help"
    )
}
