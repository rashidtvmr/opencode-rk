//! Editable writing/code artifact state: pure fragment supporting WEB-017.
//!
//! Assistant-authored long-form writing and code opens as a persisted artifact
//! part separate from the transcript message. Edits/copy/preview operate on the
//! artifact draft and never rewrite the original message. Run/apply for code
//! artifacts stays disabled until a real safe native executor exists and, once
//! present, remains subject to explicit per-action authorization (normal
//! permission/destructive-action policy is enforced by the caller).
//! Side-effect-free: no filesystem, network, process spawn, clock, or credentials.
//! Bounded: document bytes, preview bytes, exec output bytes, version history.

/// Maximum artifact document bytes (T04 DoS/retention cap).
pub const MAX_ARTIFACT_BYTES: usize = 256 * 1024;
/// Maximum preview work bytes per call (T04 preview cap).
pub const MAX_PREVIEW_BYTES: usize = 64 * 1024;
/// Maximum execution output bytes returned per authorized run (T04 cap).
pub const MAX_EXEC_OUTPUT_BYTES: usize = 16 * 1024;
/// Maximum retained explicit versions per artifact (T04 history cap).
pub const MAX_VERSIONS: usize = 32;
/// Maximum title bytes.
pub const MAX_TITLE_LEN: usize = 256;

/// Artifact content class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    Writing,
    Code,
}

/// A persisted artifact part: explicit versioned drafts plus the immutable
/// original assistant body. Sequence numbers start at 1 (the opened version).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    source_message_seq: u64,
    kind: ArtifactKind,
    title: String,
    language: Option<String>,
    original: String,
    current: String,
    current_seq: u64,
    history: Vec<String>,
    redo: Vec<String>,
    preview_shown: bool,
    focus_id: u64,
    selection: (usize, usize),
}

/// Artifact failures: typed, explicit, keyboard-independent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactError {
    EmptyTitle,
    TitleTooLong { max: usize, actual: usize },
    EmptyLanguage,
    DocumentEmpty,
    DocumentTooLarge { max: usize, actual: usize },
    PreviewCancelled,
    NoUndo,
    NoRedo,
    BadSnapshot,
}

impl core::fmt::Display for ArtifactError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyTitle => write!(f, "artifact title must not be empty"),
            Self::TitleTooLong { max, actual } => {
                write!(f, "artifact title too long: max {max} bytes, got {actual}")
            }
            Self::EmptyLanguage => write!(f, "code artifact needs a language tag"),
            Self::DocumentEmpty => write!(f, "artifact document must not be empty"),
            Self::DocumentTooLarge { max, actual } => {
                write!(f, "artifact document too large: max {max} bytes, got {actual}")
            }
            Self::PreviewCancelled => write!(f, "artifact preview cancelled"),
            Self::NoUndo => write!(f, "nothing to undo"),
            Self::NoRedo => write!(f, "nothing to redo"),
            Self::BadSnapshot => write!(f, "artifact snapshot is invalid"),
        }
    }
}

impl std::error::Error for ArtifactError {}

fn check_title(title: &str) -> Result<String, ArtifactError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(ArtifactError::EmptyTitle);
    }
    if title.len() > MAX_TITLE_LEN {
        return Err(ArtifactError::TitleTooLong {
            max: MAX_TITLE_LEN,
            actual: title.len(),
        });
    }
    Ok(title.to_string())
}

fn check_document(body: &str) -> Result<(), ArtifactError> {
    if body.is_empty() {
        return Err(ArtifactError::DocumentEmpty);
    }
    if body.len() > MAX_ARTIFACT_BYTES {
        return Err(ArtifactError::DocumentTooLarge {
            max: MAX_ARTIFACT_BYTES,
            actual: body.len(),
        });
    }
    Ok(())
}

/// Open a writing artifact as its own editable part (T01).
pub fn open_artifact(
    source_message_seq: u64,
    kind: ArtifactKind,
    title: &str,
    body: &str,
) -> Result<Artifact, ArtifactError> {
    check_document(body)?;
    Ok(Artifact {
        source_message_seq,
        kind,
        title: check_title(title)?,
        language: None,
        original: body.to_string(),
        current: body.to_string(),
        current_seq: 1,
        history: alloc::vec::Vec::new(),
        redo: alloc::vec::Vec::new(),
        preview_shown: false,
        focus_id: source_message_seq,
        selection: (0, 0),
    })
}

/// Open a code artifact with an explicit language tag (T01).
///
/// ponytail: separate constructor instead of an options struct; fold into one
/// open call with `Option<&str>` only when a third artifact kind appears.
pub fn open_code(
    source_message_seq: u64,
    title: &str,
    body: &str,
    language: &str,
) -> Result<Artifact, ArtifactError> {
    check_document(body)?;
    let language = language.trim();
    if language.is_empty() {
        return Err(ArtifactError::EmptyLanguage);
    }
    Ok(Artifact {
        source_message_seq,
        kind: ArtifactKind::Code,
        title: check_title(title)?,
        language: Some(language.to_string()),
        original: body.to_string(),
        current: body.to_string(),
        current_seq: 1,
        history: alloc::vec::Vec::new(),
        redo: alloc::vec::Vec::new(),
        preview_shown: false,
        focus_id: source_message_seq,
        selection: (0, 0),
    })
}

impl Artifact {
    /// Source assistant message sequence this artifact was opened from.
    #[must_use]
    pub fn source_message_seq(&self) -> u64 {
        self.source_message_seq
    }
    /// Content class.
    #[must_use]
    pub fn kind(&self) -> ArtifactKind {
        self.kind
    }
    /// Current draft title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }
    /// Language tag for code artifacts.
    #[must_use]
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }
    /// Original assistant body: immutable, never rewritten by edits (T01/T05).
    #[must_use]
    pub fn original_body(&self) -> &str {
        &self.original
    }
    /// Explicit version sequence of the current draft (T05).
    #[must_use]
    pub fn current_seq(&self) -> u64 {
        self.current_seq
    }
    /// Retained explicit version count, bounded by [`MAX_VERSIONS`] (T04).
    #[must_use]
    pub fn version_count(&self) -> usize {
        self.history.len() + 1
    }
    /// Whether the preview pane is shown.
    #[must_use]
    pub fn preview_shown(&self) -> bool {
        self.preview_shown
    }
    /// Stable focus handle for screen-reader/keyboard focus restore (T03).
    #[must_use]
    pub fn focus_id(&self) -> u64 {
        self.focus_id
    }
    /// Preserved editor selection (start, end) byte offsets (T03).
    #[must_use]
    pub fn selection(&self) -> (usize, usize) {
        self.selection
    }
    /// Move the editor selection; clamped to the current draft (T03).
    pub fn set_selection(&mut self, start: usize, end: usize) {
        let len = self.current.len();
        self.selection = (start.min(len), end.min(len));
    }
}

/// Edit the draft: pushes the prior draft to bounded history and mints a new
/// explicit version sequence (T01/T05). Never touches `original`.
pub fn edit_artifact(
    artifact: &mut Artifact,
    next_body: &str,
) -> Result<u64, ArtifactError> {
    check_document(next_body)?;
    if artifact.history.len() >= MAX_VERSIONS.saturating_sub(1) {
        artifact.history.remove(0);
    }
    artifact
        .history
        .push(core::mem::replace(&mut artifact.current, next_body.to_string()));
    artifact.redo.clear();
    artifact.current_seq = artifact.current_seq.saturating_add(1);
    Ok(artifact.current_seq)
}

/// Undo one draft version; preserves focus/selection (T03).
pub fn undo_edit(artifact: &mut Artifact) -> Result<u64, ArtifactError> {
    let prior = artifact.history.pop().ok_or(ArtifactError::NoUndo)?;
    artifact
        .redo
        .push(core::mem::replace(&mut artifact.current, prior));
    artifact.current_seq = artifact.current_seq.saturating_sub(1).max(1);
    Ok(artifact.current_seq)
}

/// Redo the single undone head; fails explicitly when nothing was undone
/// rather than fabricating history (T03).
pub fn redo_edit(artifact: &mut Artifact) -> Result<u64, ArtifactError> {
    let next = artifact.redo.pop().ok_or(ArtifactError::NoRedo)?;
    if artifact.history.len() >= MAX_VERSIONS.saturating_sub(1) {
        artifact.history.remove(0);
    }
    artifact
        .history
        .push(core::mem::replace(&mut artifact.current, next));
    artifact.current_seq = artifact.current_seq.saturating_add(1);
    Ok(artifact.current_seq)
}

/// Copy the current draft text (T01). Pure read; alters nothing.
#[must_use]
pub fn copy_text(artifact: &Artifact) -> &str {
    &artifact.current
}

/// Bounded preview of the current draft (T01/T04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub text: String,
    pub truncated: bool,
}

pub fn preview_artifact(
    artifact: &Artifact,
    cancelled: bool,
) -> Result<Preview, ArtifactError> {
    if cancelled {
        return Err(ArtifactError::PreviewCancelled);
    }
    let bytes = artifact.current.as_bytes();
    if bytes.len() <= MAX_PREVIEW_BYTES {
        return Ok(Preview {
            text: artifact.current.clone(),
            truncated: false,
        });
    }
    let mut end = MAX_PREVIEW_BYTES;
    while end > 0 && !artifact.current.is_char_boundary(end) {
        end -= 1;
    }
    Ok(Preview {
        text: artifact.current[..end].to_string(),
        truncated: true,
    })
}

/// Toggle the preview pane; preserves focus and selection (T03).
pub fn toggle_preview(artifact: &mut Artifact) -> bool {
    artifact.preview_shown = !artifact.preview_shown;
    artifact.preview_shown
}

/// Serializable snapshot for reload persistence (T05).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub source_message_seq: u64,
    pub kind_is_code: bool,
    pub title: String,
    pub language: Option<String>,
    pub original: String,
    pub current: String,
    pub current_seq: u64,
    pub history: Vec<String>,
}

/// Capture a reload snapshot (T05).
#[must_use]
pub fn snapshot(artifact: &Artifact) -> Snapshot {
    Snapshot {
        source_message_seq: artifact.source_message_seq,
        kind_is_code: artifact.kind == ArtifactKind::Code,
        title: artifact.title.clone(),
        language: artifact.language.clone(),
        original: artifact.original.clone(),
        current: artifact.current.clone(),
        current_seq: artifact.current_seq,
        history: artifact.history.clone(),
    }
}

/// Restore from a reload snapshot; validates bounds without rewriting the
/// original assistant body (T05).
pub fn restore_snapshot(snap: Snapshot) -> Result<Artifact, ArtifactError> {
    check_document(&snap.current)?;
    check_document(&snap.original)?;
    if snap.history.len() >= MAX_VERSIONS {
        return Err(ArtifactError::BadSnapshot);
    }
    for version in &snap.history {
        check_document(version)?;
    }
    Ok(Artifact {
        focus_id: snap.source_message_seq,
        selection: (0, 0),
        source_message_seq: snap.source_message_seq,
        kind: if snap.kind_is_code {
            ArtifactKind::Code
        } else {
            ArtifactKind::Writing
        },
        title: check_title(&snap.title)?,
        language: snap.language,
        original: snap.original,
        current: snap.current,
        current_seq: snap.current_seq.max(1),
        history: snap.history,
        redo: alloc::vec::Vec::new(),
        preview_shown: false,
    })
}

/// Explicit run/apply authorization for code artifacts (T02).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunError {
    /// No safe native executor exists: run/apply controls stay disabled.
    NoExecutor,
    /// Executor exists but no explicit human/grant approval: denied.
    Denied,
}

impl core::fmt::Display for RunError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoExecutor => write!(
                f,
                "code run/apply disabled: no safe native executor is installed"
            ),
            Self::Denied => write!(
                f,
                "code run/apply denied: explicit authorization is required"
            ),
        }
    }
}

impl std::error::Error for RunError {}

/// Authorized run permit. Execution itself is performed by the caller's safe
/// native executor through the permission broker; this permit only proves the
/// gate passed and bounds the returned output. Pure: holds no handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunPermit;

/// Gate run/apply: disabled without a safe executor, denied without an
/// explicit grant, permitted only when both hold (T02).
pub fn authorize_run(
    executor_present: bool,
    explicitly_granted: bool,
) -> Result<RunPermit, RunError> {
    if !executor_present {
        return Err(RunError::NoExecutor);
    }
    if !explicitly_granted {
        return Err(RunError::Denied);
    }
    Ok(RunPermit)
}

/// Bounded execution output produced through an authorized permit (T04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecOutput {
    pub text: String,
    pub truncated: bool,
}

impl RunPermit {
    /// Bound caller-supplied executor output; pure truncation, no execution.
    /// The caller runs the code via its sandboxed executor; this only proves
    /// the output path is bounded and never rewrites the artifact.
    #[must_use]
    pub fn execute(self, output: impl Into<String>) -> ExecOutput {
        let output = output.into();
        if output.len() <= MAX_EXEC_OUTPUT_BYTES {
            return ExecOutput {
                text: output,
                truncated: false,
            };
        }
        let mut end = MAX_EXEC_OUTPUT_BYTES;
        while end > 0 && !output.is_char_boundary(end) {
            end -= 1;
        }
        ExecOutput {
            text: output[..end].to_string(),
            truncated: true,
        }
    }
}

/// Keyboard/screen-reader control descriptor (T03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactControl {
    pub id: &'static str,
    pub label: &'static str,
    pub role: &'static str,
    pub shortcut: &'static str,
}

/// Editor/preview/undo/redo/copy/run controls. Every control carries a
/// non-empty label (screen reader), an ARIA role, and a keyboard shortcut so
/// voice/pointer is never the only path. Focus handle and selection are
/// preserved across toggle/undo by [`toggle_preview`]/[`undo_edit`].
#[must_use]
pub fn artifact_controls(kind: ArtifactKind) -> Vec<ArtifactControl> {
    let mut controls = alloc::vec::Vec::new();
    controls.push(ArtifactControl {
        id: "artifact-edit",
        label: "Edit artifact",
        role: "textbox",
        shortcut: "e",
    });
    controls.push(ArtifactControl {
        id: "artifact-copy",
        label: "Copy artifact text",
        role: "button",
        shortcut: "c",
    });
    controls.push(ArtifactControl {
        id: "artifact-preview",
        label: "Toggle preview",
        role: "switch",
        shortcut: "p",
    });
    controls.push(ArtifactControl {
        id: "artifact-undo",
        label: "Undo edit",
        role: "button",
        shortcut: "u",
    });
    controls.push(ArtifactControl {
        id: "artifact-redo",
        label: "Redo edit",
        role: "button",
        shortcut: "r",
    });
    if kind == ArtifactKind::Code {
        controls.push(ArtifactControl {
            id: "artifact-run",
            label: "Run code with explicit authorization",
            role: "button",
            shortcut: "R",
        });
        controls.push(ArtifactControl {
            id: "artifact-apply",
            label: "Apply code with explicit authorization",
            role: "button",
            shortcut: "A",
        });
    }
    controls
}

mod alloc {
    pub mod vec {
        pub use std::vec::Vec;
    }
}
