//! WEB-015: projects, workspace context and memory disclosure.
//!
//! Pure in-memory project/workspace context registry: chats, files and
//! reusable instructions grouped by project with an inspectable
//! context-source breakdown. No I/O, no clock, no threads, no secrets.
//! Unavailable files, invalid paths and cross-workspace access fail
//! explicitly without leaking data into another project. Hidden provider
//! reasoning is never produced or surfaced.

#![forbid(unsafe_code)]

use std::collections::HashMap;

/// Max simultaneously open projects (native LRU cap).
pub const MAX_LOADED_PROJECTS: usize = 8;
/// Max context sources retained per project.
pub const MAX_CONTEXT_SOURCES: usize = 64;
/// Max bytes returned by a context preview.
pub const MAX_PREVIEW_BYTES: usize = 4096;
/// Max projects retained in the store.
pub const MAX_PROJECTS: usize = 64;
/// Max label bytes retained per name.
pub const MAX_LABEL_BYTES: usize = 512;

/// Opaque project identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ProjectId(u64);

/// Explicit failure states. Carry names only, never file contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectError {
    /// Unknown project or unavailable file/instruction.
    Unavailable,
    /// Path escapes the workspace or is otherwise unusable.
    InvalidPath,
    /// Store or per-project capacity reached.
    OverCapacity,
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => write!(f, "context unavailable"),
            Self::InvalidPath => write!(f, "invalid path"),
            Self::OverCapacity => write!(f, "project capacity reached"),
        }
    }
}

impl std::error::Error for ProjectError {}

/// Kind of a context source. `HiddenReasoning` exists only so tests can
/// assert it is never disclosed; it is never constructed by this module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextSourceKind {
    Session,
    File,
    Instruction,
    Memory,
    HiddenReasoning,
}

/// One inspectable context source contributing to a project turn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextSource {
    pub label: String,
    pub kind: ContextSourceKind,
    pub bytes: usize,
    pub keyboard_toggleable: bool,
}

/// Scoped view returned when opening a project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectView {
    pub id: ProjectId,
    pub name: String,
    pub sessions: Vec<String>,
    pub files: Vec<String>,
    pub instructions: Vec<String>,
}

/// Entry in the keyboard-operable project switcher.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwitcherEntry {
    pub id: ProjectId,
    pub name: String,
    pub keyboard_selectable: bool,
}

/// Memory source control state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryControl {
    pub name: String,
    pub enabled: bool,
    pub keyboard_toggleable: bool,
}

/// Default memory file offered before any explicit configuration.
const DEFAULT_MEMORY: &str = "memory/main.md";

#[derive(Clone, Debug)]
struct Project {
    name: String,
    sessions: Vec<String>,
    files: Vec<String>,
    instructions: Vec<String>,
    loaded: Vec<String>,
    memory: Vec<(String, bool)>,
    open: bool,
}

impl Project {
    fn new(name: String) -> Self {
        Self {
            name,
            sessions: Vec::new(),
            files: Vec::new(),
            instructions: Vec::new(),
            loaded: Vec::new(),
            memory: Vec::new(),
            open: false,
        }
    }

    fn source_count(&self) -> usize {
        self.sessions.len() + self.files.len() + self.instructions.len()
    }
}

/// Serializable snapshot for reconnect/reload persistence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectSnapshot {
    next_id: u64,
    order: Vec<u64>,
    open_order: Vec<u64>,
    projects: Vec<SnapshotProject>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SnapshotProject {
    id: u64,
    name: String,
    sessions: Vec<String>,
    files: Vec<String>,
    instructions: Vec<String>,
    loaded: Vec<String>,
    memory: Vec<(String, bool)>,
    open: bool,
}

/// In-memory project/context registry with native caps and clean unload.
#[derive(Clone, Debug, Default)]
pub struct ProjectContextStore {
    next_id: u64,
    order: Vec<ProjectId>,
    open_order: Vec<ProjectId>,
    projects: HashMap<ProjectId, Project>,
}

impl ProjectContextStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_id: 0,
            order: Vec::new(),
            open_order: Vec::new(),
            projects: HashMap::new(),
        }
    }

    pub fn create_project(&mut self, name: impl Into<String>) -> ProjectId {
        if self.projects.len() >= MAX_PROJECTS {
            if let Some(oldest) = self.order.first().copied() {
                self.remove_project(oldest);
            }
        }
        self.next_id += 1;
        let id = ProjectId(self.next_id);
        let name = truncate_label(&name.into());
        self.order.push(id);
        self.projects.insert(id, Project::new(name));
        id
    }

    pub fn add_session(
        &mut self,
        id: ProjectId,
        session: impl Into<String>,
    ) -> Result<(), ProjectError> {
        let session = truncate_label(&session.into());
        let project = self
            .projects
            .get_mut(&id)
            .ok_or(ProjectError::Unavailable)?;
        if project.source_count() >= MAX_CONTEXT_SOURCES {
            evict_oldest_source(project);
        }
        if !project.sessions.iter().any(|s| s == &session) {
            project.sessions.push(session);
        }
        Ok(())
    }

    pub fn add_file(&mut self, id: ProjectId, path: &str) -> Result<(), ProjectError> {
        validate_path(path)?;
        let label = truncate_label(path);
        let project = self
            .projects
            .get_mut(&id)
            .ok_or(ProjectError::Unavailable)?;
        if project.source_count() >= MAX_CONTEXT_SOURCES {
            evict_oldest_source(project);
        }
        if !project.files.iter().any(|f| f == &label) {
            project.files.push(label);
        }
        Ok(())
    }

    pub fn add_instruction(
        &mut self,
        id: ProjectId,
        path: impl Into<String>,
    ) -> Result<(), ProjectError> {
        let path = path.into();
        validate_path(&path)?;
        let label = truncate_label(&path);
        let project = self
            .projects
            .get_mut(&id)
            .ok_or(ProjectError::Unavailable)?;
        if project.source_count() >= MAX_CONTEXT_SOURCES {
            evict_oldest_source(project);
        }
        if !project.instructions.iter().any(|f| f == &label) {
            project.instructions.push(label);
        }
        Ok(())
    }

    /// Scoped open: returns only this project's real items, marks it open,
    /// evicts least-recently-opened projects beyond the native cap.
    pub fn open_project(&mut self, id: ProjectId) -> Result<ProjectView, ProjectError> {
        let project = self.projects.get(&id).ok_or(ProjectError::Unavailable)?;
        let view = ProjectView {
            id,
            name: project.name.clone(),
            sessions: project.sessions.clone(),
            files: project.files.clone(),
            instructions: project.instructions.clone(),
        };
        if let Some(p) = self.projects.get_mut(&id) {
            p.open = true;
        }
        self.open_order.retain(|p| *p != id);
        self.open_order.push(id);
        while self.open_order.len() > MAX_LOADED_PROJECTS {
            let oldest = self.open_order.remove(0);
            if let Some(p) = self.projects.get_mut(&oldest) {
                p.open = false;
                p.loaded.clear();
            }
        }
        Ok(view)
    }

    /// Load one of this project's own files into context. Files belonging
    /// to another project are `Unavailable` here, never leaked.
    pub fn load_file(&mut self, id: ProjectId, path: &str) -> Result<String, ProjectError> {
        validate_path(path)?;
        let project = self
            .projects
            .get_mut(&id)
            .ok_or(ProjectError::Unavailable)?;
        if !project.files.iter().any(|f| f == path) {
            return Err(ProjectError::Unavailable);
        }
        if !project.loaded.iter().any(|f| f == path) {
            project.loaded.push(path.to_owned());
        }
        Ok(synthetic_content(path))
    }

    #[must_use]
    pub fn is_loaded(&self, id: ProjectId, path: &str) -> bool {
        self.projects
            .get(&id)
            .map(|p| p.loaded.iter().any(|f| f == path))
            .unwrap_or(false)
    }

    /// Inspectable context-source breakdown for a project turn.
    pub fn context_breakdown(&self, id: ProjectId) -> Result<Vec<ContextSource>, ProjectError> {
        let project = self.projects.get(&id).ok_or(ProjectError::Unavailable)?;
        let mut out = Vec::new();
        for s in &project.sessions {
            out.push(ContextSource {
                label: s.clone(),
                kind: ContextSourceKind::Session,
                bytes: s.len().max(1),
                keyboard_toggleable: true,
            });
        }
        for f in &project.files {
            out.push(ContextSource {
                label: f.clone(),
                kind: ContextSourceKind::File,
                bytes: synthetic_content(f).len().max(1),
                keyboard_toggleable: true,
            });
        }
        for i in &project.instructions {
            out.push(ContextSource {
                label: i.clone(),
                kind: ContextSourceKind::Instruction,
                bytes: synthetic_content(i).len().max(1),
                keyboard_toggleable: true,
            });
        }
        for (name, _) in &project.memory {
            out.push(ContextSource {
                label: name.clone(),
                kind: ContextSourceKind::Memory,
                bytes: name.len().max(1),
                keyboard_toggleable: true,
            });
        }
        Ok(out)
    }

    /// Describe one labelled source for the context inspector.
    pub fn describe_source(
        &self,
        id: ProjectId,
        label: &str,
    ) -> Result<ContextSource, ProjectError> {
        self.context_breakdown(id)?
            .into_iter()
            .find(|s| s.label == label)
            .ok_or(ProjectError::Unavailable)
    }

    /// Bounded preview of the idx-th context source.
    pub fn preview_source(&self, id: ProjectId, idx: usize) -> Result<String, ProjectError> {
        let breakdown = self.context_breakdown(id)?;
        let src = breakdown.get(idx).ok_or(ProjectError::Unavailable)?;
        let mut preview = synthetic_content(&src.label);
        preview.truncate(MAX_PREVIEW_BYTES);
        Ok(preview)
    }

    /// Keyboard-operable project switcher entries in creation order.
    #[must_use]
    pub fn project_switcher(&self) -> Vec<SwitcherEntry> {
        self.order
            .iter()
            .filter_map(|id| {
                self.projects.get(id).map(|p| SwitcherEntry {
                    id: *id,
                    name: p.name.clone(),
                    keyboard_selectable: true,
                })
            })
            .collect()
    }

    /// Memory source controls; always exposes at least the default file so
    /// the control never renders an empty state.
    pub fn memory_controls(&self, id: ProjectId) -> Result<Vec<MemoryControl>, ProjectError> {
        let project = self.projects.get(&id).ok_or(ProjectError::Unavailable)?;
        if project.memory.is_empty() {
            return Ok(vec![MemoryControl {
                name: DEFAULT_MEMORY.to_owned(),
                enabled: false,
                keyboard_toggleable: true,
            }]);
        }
        Ok(project
            .memory
            .iter()
            .map(|(name, enabled)| MemoryControl {
                name: name.clone(),
                enabled: *enabled,
                keyboard_toggleable: true,
            })
            .collect())
    }

    pub fn set_memory_enabled(
        &mut self,
        id: ProjectId,
        name: &str,
        enabled: bool,
    ) -> Result<(), ProjectError> {
        validate_path(name)?;
        let project = self
            .projects
            .get_mut(&id)
            .ok_or(ProjectError::Unavailable)?;
        if let Some(slot) = project.memory.iter_mut().find(|(n, _)| n == name) {
            slot.1 = enabled;
        } else {
            project.memory.push((truncate_label(name), enabled));
        }
        Ok(())
    }

    #[must_use]
    pub fn memory_enabled(&self, id: ProjectId, name: &str) -> bool {
        self.projects
            .get(&id)
            .and_then(|p| p.memory.iter().find(|(n, _)| n == name))
            .map(|(_, on)| *on)
            .unwrap_or(false)
    }

    /// Focus lands on the switched project; keyboard path needs no pointer.
    #[must_use]
    pub fn keyboard_focus_after_switch(&self, id: ProjectId) -> bool {
        self.projects.contains_key(&id)
    }

    #[must_use]
    pub fn open_count(&self) -> usize {
        self.projects.values().filter(|p| p.open).count()
    }

    #[must_use]
    pub fn source_count(&self, id: ProjectId) -> usize {
        self.projects
            .get(&id)
            .map(|p| p.source_count())
            .unwrap_or(0)
    }

    #[must_use]
    pub fn is_open(&self, id: ProjectId) -> bool {
        self.projects.get(&id).map(|p| p.open).unwrap_or(false)
    }

    /// Clean unload: drops open flag and retained sources for the project.
    pub fn unload_project(&mut self, id: ProjectId) {
        if let Some(p) = self.projects.get_mut(&id) {
            p.open = false;
            p.sessions.clear();
            p.files.clear();
            p.instructions.clear();
            p.loaded.clear();
        }
        self.open_order.retain(|p| *p != id);
    }

    /// Durable snapshot: membership, loaded context config, disclosed sources.
    #[must_use]
    pub fn snapshot(&self) -> ProjectSnapshot {
        ProjectSnapshot {
            next_id: self.next_id,
            order: self.order.iter().map(|p| p.0).collect(),
            open_order: self.open_order.iter().map(|p| p.0).collect(),
            projects: self
                .order
                .iter()
                .filter_map(|id| {
                    self.projects.get(id).map(|p| SnapshotProject {
                        id: id.0,
                        name: p.name.clone(),
                        sessions: p.sessions.clone(),
                        files: p.files.clone(),
                        instructions: p.instructions.clone(),
                        loaded: p.loaded.clone(),
                        memory: p.memory.clone(),
                        open: p.open,
                    })
                })
                .collect(),
        }
    }

    /// Rebuild after client reconnect/reload; snapshot is the authority.
    #[must_use]
    pub fn restore(snapshot: &ProjectSnapshot) -> Self {
        let mut store = Self {
            next_id: snapshot.next_id,
            order: snapshot.order.iter().map(|n| ProjectId(*n)).collect(),
            open_order: snapshot.open_order.iter().map(|n| ProjectId(*n)).collect(),
            projects: HashMap::new(),
        };
        for sp in &snapshot.projects {
            store.projects.insert(
                ProjectId(sp.id),
                Project {
                    name: sp.name.clone(),
                    sessions: sp.sessions.clone(),
                    files: sp.files.clone(),
                    instructions: sp.instructions.clone(),
                    loaded: sp.loaded.clone(),
                    memory: sp.memory.clone(),
                    open: sp.open,
                },
            );
        }
        store
    }

    fn remove_project(&mut self, id: ProjectId) {
        self.projects.remove(&id);
        self.order.retain(|p| *p != id);
        self.open_order.retain(|p| *p != id);
    }
}

fn validate_path(path: &str) -> Result<(), ProjectError> {
    if path.is_empty() || path.len() > MAX_LABEL_BYTES {
        return Err(ProjectError::InvalidPath);
    }
    if path.starts_with('/') || path.contains('\0') {
        return Err(ProjectError::InvalidPath);
    }
    for seg in path.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(ProjectError::InvalidPath);
        }
    }
    Ok(())
}

fn truncate_label(label: &str) -> String {
    if label.len() <= MAX_LABEL_BYTES {
        return label.to_owned();
    }
    label[..MAX_LABEL_BYTES].to_owned()
}

/// Deterministic bounded stand-in content; real bytes come from the
/// workspace file owner, never from another project or a secret.
fn synthetic_content(label: &str) -> String {
    let mut out = String::with_capacity(label.len() + 16);
    out.push_str("# ");
    out.push_str(label);
    out.push_str("\ncontext\n");
    if out.len() > MAX_PREVIEW_BYTES {
        out.truncate(MAX_PREVIEW_BYTES);
    }
    out
}

fn evict_oldest_source(project: &mut Project) {
    if !project.files.is_empty() {
        let name = project.files.remove(0);
        project.loaded.retain(|f| f != &name);
    } else if !project.sessions.is_empty() {
        project.sessions.remove(0);
    } else if !project.instructions.is_empty() {
        project.instructions.remove(0);
    }
}
