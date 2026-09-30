//! File operations tool implementation.
//!
//! Provides FileTool for file system operations including read, write,
//! list directory, and create directory.

use opencode_rk_security::{Decision, FileAction, OperationIntent, PermissionBroker};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::{fs, io::Write};
use thiserror::Error;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use rustix::fs::{self as rustix_fs, fstat, mkdirat, openat, Mode, OFlags};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::ffi::OsStr;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::fd::AsFd;

/// Error type for file operations.
#[derive(Debug, Error)]
pub enum ToolError {
    /// A file-related error with message.
    #[error("file error: {0}")]
    FileError(String),
    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Result of a file operation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FileResult {
    /// Whether the operation succeeded.
    pub success: bool,
    /// Content of the result (file content, directory listing, etc).
    pub content: String,
    /// Error message if operation failed.
    pub error: Option<String>,
}

impl FileResult {
    /// Creates a successful result with content.
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            success: true,
            content: content.into(),
            error: None,
        }
    }

    /// Creates a failed result with an error message.
    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            content: String::new(),
            error: Some(error.into()),
        }
    }
}

/// File operation types.
#[derive(Debug, Clone)]
pub enum FileOperation {
    /// Read a file at the given path.
    Read {
        /// Path to the file to read.
        path: PathBuf,
        /// Optional byte offset to start reading from.
        offset: Option<usize>,
        /// Optional maximum bytes to read.
        limit: Option<usize>,
    },
    /// Write content to a file at the given path.
    Write {
        /// Path to the file to write.
        path: PathBuf,
        /// Content to write.
        content: String,
        /// Whether to append to existing file.
        append: bool,
    },
    /// List directory contents.
    List {
        /// Path to the directory to list.
        path: PathBuf,
    },
    /// Create a directory.
    CreateDir {
        /// Path to the directory to create.
        path: PathBuf,
        /// Whether to create parent directories.
        recursive: bool,
    },
}

impl FileOperation {
    /// Creates a Read operation.
    pub fn read(path: impl Into<PathBuf>) -> Self {
        Self::Read {
            path: path.into(),
            offset: None,
            limit: None,
        }
    }

    /// Creates a Read operation with offset and limit.
    pub fn read_with_range(path: impl Into<PathBuf>, offset: usize, limit: Option<usize>) -> Self {
        Self::Read {
            path: path.into(),
            offset: Some(offset),
            limit,
        }
    }

    /// Creates a Write operation.
    pub fn write(path: impl Into<PathBuf>, content: String) -> Self {
        Self::Write {
            path: path.into(),
            content,
            append: false,
        }
    }

    /// Creates a Write operation with append mode.
    pub fn write_append(path: impl Into<PathBuf>, content: String) -> Self {
        Self::Write {
            path: path.into(),
            content,
            append: true,
        }
    }

    /// Creates a List directory operation.
    pub fn list(path: impl Into<PathBuf>) -> Self {
        Self::List { path: path.into() }
    }

    /// Creates a CreateDir operation.
    pub fn create_dir(path: impl Into<PathBuf>) -> Self {
        Self::CreateDir {
            path: path.into(),
            recursive: true,
        }
    }
}

/// FileTool for performing file system operations.
pub struct FileTool {
    /// Lexically normalized, approved project root. A rooted tool never uses
    /// the process working directory to resolve an operation path.
    project_root: Option<PathBuf>,
}

impl FileTool {
    /// Creates a new FileTool instance.
    ///
    /// This is the legacy compatibility constructor. Its write path is scoped
    /// to the target's nearest existing parent and is not server authority.
    pub fn new() -> Self {
        Self { project_root: None }
    }

    /// Creates a FileTool scoped to one project root.
    ///
    /// The root is captured and lexically normalized once. Callers should pass
    /// an absolute project directory; relative roots are rejected when used so
    /// a later working-directory change cannot change the authority boundary.
    pub fn with_project_root(root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: Some(normalize_project_root(root.into())),
        }
    }

    /// Executes a file operation and returns the result.
    pub fn execute(&self, op: FileOperation) -> Result<FileResult, ToolError> {
        match self.project_root.as_deref() {
            Some(root) => execute_rooted(root, op),
            None => execute(op),
        }
    }

    /// Executes a file operation after broker authorization.
    ///
    /// Write ops authorize `OperationIntent::File { Write, path }` first;
    /// denial (or human-gate) returns `Ok(failure)` with zero filesystem I/O.
    pub fn execute_authorized(
        &self,
        op: FileOperation,
        broker: &PermissionBroker,
    ) -> Result<FileResult, ToolError> {
        if let FileOperation::Write { ref path, .. } = op {
            let authorized_path = self
                .project_root
                .as_deref()
                .filter(|_| !path.is_absolute())
                .map(|root| root.join(path))
                .unwrap_or_else(|| path.clone());
            let intent = OperationIntent::File {
                action: FileAction::Write,
                path: authorized_path,
            };
            match broker.authorize(&intent) {
                Decision::Allow => (),
                Decision::Deny { reason } => {
                    return Ok(FileResult::failure(format!("write denied: {reason}")));
                }
                Decision::RequireHuman { reason, .. } => {
                    return Ok(FileResult::failure(format!(
                        "write requires human approval: {reason}"
                    )));
                }
            }
        }
        self.execute(op)
    }
}

impl Default for FileTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Executes a file operation after broker authorization.
///
/// Write ops authorize before any filesystem I/O: `Deny` and `RequireHuman`
/// return `Ok(FileResult::failure(..))` without creating, truncating, or
/// making parent directories. Non-write ops pass through to [`execute`].
pub fn execute_authorized(
    op: FileOperation,
    broker: &PermissionBroker,
) -> Result<FileResult, ToolError> {
    if let FileOperation::Write { ref path, .. } = op {
        let intent = OperationIntent::File {
            action: FileAction::Write,
            path: path.clone(),
        };
        match broker.authorize(&intent) {
            Decision::Allow => (),
            Decision::Deny { reason } => {
                return Ok(FileResult::failure(format!("write denied: {reason}")));
            }
            Decision::RequireHuman { reason, .. } => {
                return Ok(FileResult::failure(format!(
                    "write requires human approval: {reason}"
                )));
            }
        }
    }
    execute(op)
}

/// Executes a file operation and returns the result.
pub fn execute(op: FileOperation) -> Result<FileResult, ToolError> {
    execute_with_preopen_hook(op, None)
}

/// Internal testability seam: [`execute`] with an optional write pre-open hook.
///
/// The hook is invoked once per write that passes the symlink check, after any
/// parent-directory creation and immediately before the final parent recheck and
/// leaf open. It cannot bypass broker authorization (earlier in [`execute_authorized`]).
pub(crate) fn execute_with_preopen_hook(
    op: FileOperation,
    hook: Option<fn(&Path, bool) -> Result<(), ToolError>>,
) -> Result<FileResult, ToolError> {
    match op {
        FileOperation::Read {
            path,
            offset,
            limit,
        } => read_file(&path, offset, limit),
        FileOperation::Write {
            path,
            content,
            append,
        } => write_file(&path, &content, append, hook),
        FileOperation::List { path } => list_dir(&path),
        FileOperation::CreateDir { path, recursive } => create_directory(&path, recursive),
    }
}

/// Reads a file at the given path with optional offset and limit.
fn read_file(
    path: &Path,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<FileResult, ToolError> {
    if !path.exists() {
        return Ok(FileResult::failure(format!("File not found: {:?}", path)));
    }

    let mut content = fs::read_to_string(path).map_err(ToolError::IoError)?;

    if let Some(off) = offset {
        if off > content.len() {
            return Ok(FileResult::failure(format!(
                "Offset {} exceeds file length {}",
                off,
                content.len()
            )));
        }
        content = content[off..].to_string();
    }

    if let Some(lim) = limit {
        content = content.chars().take(lim).collect();
    }

    Ok(FileResult::success(content))
}

/// Writes content to a file at the given path.
fn write_file(
    path: &Path,
    content: &str,
    append: bool,
    hook: Option<fn(&Path, bool) -> Result<(), ToolError>>,
) -> Result<FileResult, ToolError> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        return write_file_descriptor_relative(path, content, append, hook);
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (path, content, append, hook);
        Ok(FileResult::failure(
            "write denied: unsupported platform",
        ))
    }
}

fn execute_rooted(root: &Path, op: FileOperation) -> Result<FileResult, ToolError> {
    if !root.is_absolute() {
        return Ok(FileResult::failure(
            "file operation denied: project root must be absolute",
        ));
    }
    match op {
        FileOperation::Read {
            path,
            offset,
            limit,
        } => {
            let path = rooted_path(root, &path)?;
            read_file(&path, offset, limit)
        }
        FileOperation::Write {
            path,
            content,
            append,
        } => write_file_rooted(root, &path, &content, append, None),
        FileOperation::List { path } => {
            let path = rooted_path(root, &path)?;
            list_dir(&path)
        }
        FileOperation::CreateDir { path, recursive } => {
            let path = rooted_path(root, &path)?;
            create_directory(&path, recursive)
        }
    }
}

fn normalize_project_root(root: PathBuf) -> PathBuf {
    normalize_lexical_path(&root)
}

fn normalize_lexical_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalize_platform_path(&normalized)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn rooted_path(root: &Path, path: &Path) -> Result<PathBuf, ToolError> {
    let normalized = normalize_rooted_path(root, path)?;
    Ok(normalized_path(root, &normalized))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn rooted_path(root: &Path, path: &Path) -> Result<PathBuf, ToolError> {
    let mut joined = if path.is_absolute() {
        let root = normalize_lexical_path(root);
        let path = normalize_lexical_path(path);
        path.strip_prefix(&root)
            .map_err(|_| ToolError::FileError("path outside project root".to_owned()))?
            .to_path_buf()
    } else {
        path.to_path_buf()
    };
    for component in joined.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err(ToolError::FileError("write denied: invalid path".to_owned()));
        }
    }
    joined = root.join(joined);
    Ok(joined)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn path_has_symlink_component_from(root: &Path, path: &Path) -> Result<bool, ToolError> {
    let absolute = if path.is_absolute() {
        normalize_platform_path(path)
    } else {
        normalize_platform_path(&root.join(path))
    };
    let mut current = PathBuf::new();
    for component in absolute.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                return Err(ToolError::FileError(
                    "write denied: unable to inspect path".to_owned(),
                ));
            }
        }
    }
    Ok(false)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
const MAX_WRITE_COMPONENTS: usize = 256;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const MAX_WRITE_PATH_BYTES: usize = 4096;

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct NormalizedWritePath {
    components: Vec<std::ffi::OsString>,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn write_file_descriptor_relative(
    path: &Path,
    content: &str,
    append: bool,
    hook: Option<fn(&Path, bool) -> Result<(), ToolError>>,
) -> Result<FileResult, ToolError> {
    let (anchor, normalized) = legacy_write_anchor(path)?;
    if path_has_symlink_component_from(&anchor, &normalized_path(&anchor, &normalized))? {
        return Ok(FileResult::failure(
            "write denied: path contains a symlink component",
        ));
    }
    write_file_descriptor_at(&anchor, &normalized, path, content, append, hook)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn write_file_rooted(
    root: &Path,
    path: &Path,
    content: &str,
    append: bool,
    hook: Option<fn(&Path, bool) -> Result<(), ToolError>>,
) -> Result<FileResult, ToolError> {
    let normalized = normalize_rooted_path(root, path)?;
    if path_has_symlink_component_from(root, &normalized_path(root, &normalized))? {
        return Ok(FileResult::failure(
            "write denied: path contains a symlink component",
        ));
    }
    write_file_descriptor_at(root, &normalized, path, content, append, hook)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn write_file_rooted(
    _root: &Path,
    _path: &Path,
    _content: &str,
    _append: bool,
    _hook: Option<fn(&Path, bool) -> Result<(), ToolError>>,
) -> Result<FileResult, ToolError> {
    Ok(FileResult::failure("write denied: unsupported platform"))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn write_file_descriptor_at(
    root_path: &Path,
    normalized: &NormalizedWritePath,
    display_path: &Path,
    content: &str,
    append: bool,
    hook: Option<fn(&Path, bool) -> Result<(), ToolError>>,
) -> Result<FileResult, ToolError> {
    let leaf = normalized
        .components
        .last()
        .ok_or_else(|| ToolError::FileError("write denied: path has no leaf".to_owned()))?;
    let parent_components = &normalized.components[..normalized.components.len() - 1];
    let root = open_write_root_at(root_path, None)?;
    let pinned = match traverse_write_parent(&root, parent_components, true) {
        Ok(descriptors) => descriptors,
        Err(ToolError::IoError(error)) if is_symlink_error(&error) => {
            return Ok(FileResult::failure(
                "write denied: path contains a symlink component",
            ));
        }
        Err(error) => return Err(error),
    };

    // Pre-open seam: traversal and parent creation are complete; descriptor-relative
    // re-traversal below catches parent swaps before opening the leaf.
    if let Some(hook) = hook {
        hook(display_path, append)?;
    }

    let current = traverse_write_parent(&root, parent_components, false)
        .map_err(|_| ToolError::FileError("write denied: parent changed during write".to_owned()))?;
    if !same_descriptor_path(&pinned, &current)? {
        return Ok(FileResult::failure(
            "write denied: parent changed during write",
        ));
    }
    let parent = pinned
        .last()
        .ok_or_else(|| ToolError::FileError("write denied: missing parent".to_owned()))?;
    let mut flags = OFlags::WRONLY | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    flags |= if append { OFlags::APPEND } else { OFlags::TRUNC };
    let fd = openat(parent.as_fd(), OsStr::new(leaf), flags, Mode::from(0o600))
        .map_err(|error| ToolError::IoError(error.into()))?;
    let mut file = std::fs::File::from(fd);
    file.write_all(content.as_bytes()).map_err(ToolError::IoError)?;

    Ok(FileResult::success(format!(
        "Successfully wrote {} bytes to {:?}",
        content.len(), display_path
    )))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn normalized_path(root: &Path, path: &NormalizedWritePath) -> PathBuf {
    root.join(path.components.iter().collect::<PathBuf>())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn normalize_rooted_path(root: &Path, path: &Path) -> Result<NormalizedWritePath, ToolError> {
    if path.as_os_str().len() > MAX_WRITE_PATH_BYTES {
        return Err(ToolError::FileError("write denied: path too long".to_owned()));
    }
    let root = root.to_path_buf();
    let relative = if path.is_absolute() {
        let absolute = normalize_absolute_path_without_parent(path)?;
        absolute
            .strip_prefix(&root)
            .map_err(|_| {
            ToolError::FileError("write denied: path outside project root".to_owned())
            })?
            .to_path_buf()
    } else {
        path.to_path_buf()
    };
    let mut components = Vec::new();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(name) => components.push(name.to_owned()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                return Err(ToolError::FileError("write denied: path traversal".to_owned()))
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err(ToolError::FileError("write denied: invalid path".to_owned()))
            }
        }
    }
    if components.is_empty() {
        return Err(ToolError::FileError("write denied: path has no leaf".to_owned()));
    }
    if components.len() > MAX_WRITE_COMPONENTS {
        return Err(ToolError::FileError(
            "write denied: too many path components".to_owned(),
        ));
    }
    Ok(NormalizedWritePath { components })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn normalize_absolute_path_without_parent(path: &Path) -> Result<PathBuf, ToolError> {
    let mut normalized = PathBuf::new();
    for component in normalize_platform_path(path).components() {
        match component {
            std::path::Component::ParentDir => {
                return Err(ToolError::FileError("write denied: path traversal".to_owned()))
            }
            std::path::Component::CurDir => {}
            _ => normalized.push(component.as_os_str()),
        }
    }
    Ok(normalized)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn legacy_write_anchor(path: &Path) -> Result<(PathBuf, NormalizedWritePath), ToolError> {
    let cwd = if path.is_absolute() {
        None
    } else {
        Some(normalize_platform_path(
            &std::env::current_dir().map_err(ToolError::IoError)?,
        ))
    };
    let absolute = if let Some(cwd) = cwd {
        cwd.join(path)
    } else {
        path.to_path_buf()
    };
    let absolute = normalize_absolute_path_without_parent(&absolute)?;
    let mut anchor = absolute
        .parent()
        .ok_or_else(|| ToolError::FileError("write denied: path has no parent".to_owned()))?
            .to_path_buf();
    while !anchor.exists() {
        anchor = anchor
            .parent()
            .ok_or_else(|| ToolError::FileError("write denied: no existing target parent".to_owned()))?
            .to_path_buf();
    }
    if anchor.parent().is_none() {
        return Err(ToolError::FileError(
            "write denied: filesystem root is not a compatibility anchor".to_owned(),
        ));
    }
    let relative = absolute
        .strip_prefix(&anchor)
        .map_err(|_| ToolError::FileError("write denied: invalid target parent".to_owned()))?;
    let normalized = normalize_rooted_path(&anchor, relative)?;
    Ok((anchor, normalized))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn is_symlink_error(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
/// Root directory file descriptor opened for descriptor-relative operations.
///
/// The root FD is opened ONCE per write operation with O_NOFOLLOW|O_DIRECTORY,
/// providing a stable anchor for traversing paths without following symlinks.
/// This observer seam allows tests to observe the root descriptor's metadata
/// (device/inode identity) for verification without modifying production behavior.
pub(crate) fn open_write_root_with_observer(
    mut observer: Option<
        &mut dyn FnMut(&std::os::fd::OwnedFd) -> Result<(), ToolError>,
    >,
) -> Result<std::os::fd::OwnedFd, ToolError> {
    open_write_root_at(
        &normalize_platform_path(&std::env::current_dir().map_err(ToolError::IoError)?),
        observer,
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn open_write_root_at(
    root: &Path,
    mut observer: Option<
        &mut dyn FnMut(&std::os::fd::OwnedFd) -> Result<(), ToolError>,
    >,
) -> Result<std::os::fd::OwnedFd, ToolError> {
    if !root.is_absolute() {
        return Err(ToolError::FileError(
            "write denied: project root must be absolute".to_owned(),
        ));
    }
    if root.parent().is_none() {
        return Err(ToolError::FileError(
            "write denied: filesystem root is not an approved project root".to_owned(),
        ));
    }
    let fd = openat(
        rustix_fs::CWD,
        root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| ToolError::IoError(error.into()))?;
    if let Some(obs) = observer.as_mut() {
        obs(&fd)?;
    }
    Ok(fd)
}

/// Opens the root directory as a file descriptor for descriptor-relative operations.
///
/// This is the production entry point: calls `open_write_root_with_observer` with
/// no observer, preserving exact byte-for-byte behavior.
pub(crate) fn open_write_root() -> Result<std::os::fd::OwnedFd, ToolError> {
    open_write_root_with_observer(None)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn traverse_write_parent(
    root: &std::os::fd::OwnedFd,
    components: &[std::ffi::OsString],
    create: bool,
) -> Result<Vec<std::os::fd::OwnedFd>, ToolError> {
    let mut descriptors = vec![rustix::io::dup(root).map_err(|error| ToolError::IoError(error.into()))?];
    for component in components {
        let parent = descriptors
            .last()
            .ok_or_else(|| ToolError::FileError("write denied: missing parent".to_owned()))?;
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let next = match openat(parent.as_fd(), component.as_os_str(), flags, Mode::empty()) {
            Ok(fd) => fd,
            Err(error)
                if create && error == rustix::io::Errno::NOENT => {
                    mkdirat(parent.as_fd(), component.as_os_str(), Mode::from(0o700))
                        .map_err(|error| ToolError::IoError(error.into()))?;
                    openat(parent.as_fd(), component.as_os_str(), flags, Mode::empty())
                        .map_err(|error| ToolError::IoError(error.into()))?
                }
            Err(error) => return Err(ToolError::IoError(error.into())),
        };
        descriptors.push(next);
    }
    Ok(descriptors)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn same_descriptor_path(
    expected: &[std::os::fd::OwnedFd],
    current: &[std::os::fd::OwnedFd],
) -> Result<bool, ToolError> {
    if expected.len() != current.len() {
        return Ok(false);
    }
    for (left, right) in expected.iter().zip(current) {
        let left_stat = fstat(left).map_err(|error| ToolError::IoError(error.into()))?;
        let right_stat = fstat(right).map_err(|error| ToolError::IoError(error.into()))?;
        if left_stat.st_dev != right_stat.st_dev || left_stat.st_ino != right_stat.st_ino {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(target_os = "macos")]
fn normalize_platform_path(path: &Path) -> PathBuf {
    for (alias, physical) in [
        (Path::new("/var"), Path::new("/private/var")),
        (Path::new("/tmp"), Path::new("/private/tmp")),
    ] {
        if let Ok(relative) = path.strip_prefix(alias) {
            return physical.join(relative);
        }
    }
    path.to_path_buf()
}

#[cfg(target_os = "linux")]
fn normalize_platform_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

/// Lists the contents of a directory.
fn list_dir(path: &Path) -> Result<FileResult, ToolError> {
    if !path.exists() {
        return Ok(FileResult::failure(format!(
            "Directory not found: {:?}",
            path
        )));
    }

    if !path.is_dir() {
        return Ok(FileResult::failure(format!(
            "Path is not a directory: {:?}",
            path
        )));
    }

    let mut entries: Vec<String> = Vec::new();
    for entry in fs::read_dir(path).map_err(ToolError::IoError)? {
        let entry = entry.map_err(ToolError::IoError)?;
        if let Some(name) = entry.file_name().to_str() {
            entries.push(name.to_string());
        }
    }

    entries.sort();
    Ok(FileResult::success(
        serde_json::to_string(&entries).unwrap(),
    ))
}

/// Creates a directory at the given path.
fn create_directory(path: &Path, recursive: bool) -> Result<FileResult, ToolError> {
    if recursive {
        fs::create_dir_all(path).map_err(ToolError::IoError)?;
    } else {
        fs::create_dir(path).map_err(ToolError::IoError)?;
    }

    Ok(FileResult::success(format!(
        "Successfully created directory {:?}",
        path
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[tokio::test]
    async fn read_file() {
        let dir = tempdir().expect("Failed to create temp dir");
        let file_path = dir.path().join("test.txt");

        // Create a test file
        let mut file = std::fs::File::create(&file_path).expect("Failed to create file");
        writeln!(file, "Hello, World!").expect("Failed to write");

        // Create read operation
        let op = FileOperation::read(&file_path);
        let result = execute(op).expect("execute should succeed");

        assert!(result.success);
        assert!(result.content.contains("Hello, World!"));
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn write_file() {
        let dir = tempdir().expect("Failed to create temp dir");
        let file_path = dir.path().join("new_file.txt");
        let content = "Test content from write_file test".to_string();

        // Create write operation
        let op = FileOperation::write(&file_path, content.clone());
        let result = execute(op).expect("execute should succeed");

        assert!(result.success);
        assert!(result.content.contains("Successfully wrote"));

        // Verify file was written
        let read_result = std::fs::read_to_string(&file_path).expect("Failed to read file");
        assert_eq!(read_result, content);
    }

    #[tokio::test]
    async fn list_dir() {
        let dir = tempdir().expect("Failed to create temp dir");
        let dir_path = dir.path();

        // Create some test files
        std::fs::write(dir_path.join("file1.txt"), b"content1").expect("Failed to write file1");
        std::fs::write(dir_path.join("file2.txt"), b"content2").expect("Failed to write file2");

        // Create list operation
        let op = FileOperation::list(dir_path);
        let result = execute(op).expect("execute should succeed");

        assert!(result.success);
        // Verify the JSON contains the file names
        assert!(result.content.contains("file1.txt"));
        assert!(result.content.contains("file2.txt"));
    }

    #[tokio::test]
    async fn create_dir() {
        let dir = tempdir().expect("Failed to create temp dir");
        let new_dir = dir.path().join("new_directory");

        // Verify it doesn't exist
        assert!(!new_dir.exists());

        // Create directory operation
        let op = FileOperation::create_dir(&new_dir);
        let result = execute(op).expect("execute should succeed");

        assert!(result.success);
        assert!(new_dir.exists());
    }

    #[tokio::test]
    async fn handle_missing_file() {
        let dir = tempdir().expect("Failed to create temp dir");
        let missing_file = dir.path().join("nonexistent.txt");

        // Create read operation for non-existent file
        let op = FileOperation::read(&missing_file);
        let result = execute(op).expect("execute should succeed");

        // The operation should complete but report failure
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("File not found"));
    }

    // --- APP-012-TOCTOU-SEAM-W1: pre-open seam tests -----------------------
    // Hooks are plain non-capturing fn pointers: no global mutable state, no
    // sleeps, no unsafe. Observation happens via deterministic files derived
    // from the hook's own path argument inside the disposable tempdir.

    /// Records that the hook ran at the pre-open boundary and with which flag.
    fn hook_record(path: &Path, append: bool) -> Result<(), ToolError> {
        let parent = path.parent().expect("hook: path has parent");
        assert!(parent.exists(), "hook must run after parent creation");
        let marker = parent.join(if append { "hook_append_true" } else { "hook_append_false" });
        fs::write(&marker, b"seen").map_err(ToolError::IoError)?;
        Ok(())
    }

    /// Cancels the write from the hook.
    fn hook_cancel(_path: &Path, _append: bool) -> Result<(), ToolError> {
        Err(ToolError::FileError("pre-open canceled by test hook".to_owned()))
    }

    /// Deterministic disposable-fixture mutation: swap the leaf for a symlink
    /// just before the real open, simulating a concurrent TOCTOU race.
    #[cfg(unix)]
    fn hook_swap_leaf_to_symlink(path: &Path, _append: bool) -> Result<(), ToolError> {
        let target = path.with_extension("sentinel_target");
        fs::write(&target, b"external sentinel").map_err(ToolError::IoError)?;
        assert!(path.symlink_metadata().is_err(), "leaf must not exist pre-open");
        std::os::unix::fs::symlink(&target, path).map_err(ToolError::IoError)?;
        Ok(())
    }

    /// Test that symlink write is denied before any file open.
    #[test]
    fn write_file_symlink_denied() {
        let dir = tempdir().expect("Failed to create temp dir");
        let outside_dir = tempdir().expect("outside sentinel dir");
        let sentinel = outside_dir.path().join("sentinel.txt");
        std::fs::write(&sentinel, "sentinel content").expect("write sentinel");

        // Create symlink in fixture dir pointing outside
        let symlink_path = dir.path().join("symlink_to_outside.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&sentinel, &symlink_path).expect("create symlink");

        // Attempt write through the symlink target
        let op = FileOperation::write(&symlink_path, "ATTACK".to_string());
        let result = execute(op).expect("execute should succeed");

        // Write must be denied
        assert!(!result.success, "symlink write should be denied");
        assert!(
            result.error.unwrap_or_default().contains("symlink"),
            "error should mention symlink"
        );

        // Sentinel must be unchanged (no write through symlink)
        assert_eq!(
            std::fs::read_to_string(&sentinel).expect("read sentinel"),
            "sentinel content"
        );
    }

    /// Hook is invoked exactly once per write, receives the correct path and
    /// append=false for a truncate write, and the write completes normally.
    #[test]
    fn write_file_preopen_hook_invoked() {
        let dir = tempdir().expect("temp dir");
        let file_path = dir.path().join("hook_test.txt");
        let content = "hook test content";

        let result = execute_with_preopen_hook(
            FileOperation::write(&file_path, content.to_owned()),
            Some(hook_record),
        )
        .expect("execute ok");

        assert!(result.success);
        assert_eq!(fs::read_to_string(&file_path).expect("read file"), content);
        assert!(dir.path().join("hook_append_false").exists());
        assert!(!dir.path().join("hook_append_true").exists());
    }

    /// Hook observes append=true for an append write and the append lands.
    #[test]
    fn write_file_append_hook_sees_flag() {
        let dir = tempdir().expect("temp dir");
        let file_path = dir.path().join("append_test.txt");
        fs::write(&file_path, "initial").expect("write initial");

        let result = execute_with_preopen_hook(
            FileOperation::write_append(&file_path, "more".to_owned()),
            Some(hook_record),
        )
        .expect("execute ok");

        assert!(result.success);
        assert_eq!(fs::read_to_string(&file_path).expect("read"), "initialmore");
        assert!(dir.path().join("hook_append_true").exists());
        assert!(!dir.path().join("hook_append_false").exists());
    }

    /// No hook (production default via `execute`): write proceeds with
    /// identical behavior; parent dirs are created.
    #[test]
    fn write_file_no_hook_production_default() {
        let dir = tempdir().expect("temp dir");
        let file_path = dir.path().join("nested/deep/prod.txt");

        let result = execute(FileOperation::write(&file_path, "prod".to_owned())).expect("ok");

        assert!(result.success);
        assert_eq!(fs::read_to_string(&file_path).expect("read"), "prod");
    }

    /// Symlink denial happens BEFORE the hook: hook error (which would also
    /// fail the write) must never run, proving seam ordering.
    #[test]
    fn write_file_symlink_denied_before_hook() {
        let dir = tempdir().expect("temp dir");
        let outside = tempdir().expect("outside dir");
        let sentinel = outside.path().join("sentinel.txt");
        fs::write(&sentinel, "sentinel content").expect("write sentinel");
        let link = dir.path().join("link.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&sentinel, &link).expect("create symlink");

        let err = execute_with_preopen_hook(
            FileOperation::write(&link, "ATTACK".to_owned()),
            Some(hook_cancel),
        )
        .expect("denial is Ok(failure), not Err");

        assert!(!err.success);
        assert!(err.error.unwrap_or_default().contains("symlink"));
        // cancel hook would have errored instead; symlink denial won => hook never ran
        assert_eq!(fs::read_to_string(&sentinel).expect("read"), "sentinel content");
        assert!(!dir.path().join("hook_append_false").exists());
    }

    /// Hook Err aborts the write BEFORE open: no file is created.
    #[test]
    fn write_file_hook_cancel_aborts_before_open() {
        let dir = tempdir().expect("temp dir");
        let file_path = dir.path().join("never_created.txt");

        let result = execute_with_preopen_hook(
            FileOperation::write(&file_path, "bytes".to_owned()),
            Some(hook_cancel),
        );

        match result {
            Err(ToolError::FileError(msg)) => assert!(msg.contains("test hook")),
            other => panic!("expected hook ToolError::FileError, got {other:?}"),
        }
        assert!(!file_path.exists(), "cancelled write must not create the file");
    }

    /// Seam supports deterministic disposable-fixture mutation without
    /// globals: hook swaps the leaf for a symlink just before open; the
    /// O_NOFOLLOW leaf open then fails the write instead of following it.
    /// Documents the boundary only; NOT a TOCTOU fix claim.
    #[test]
    #[cfg(unix)]
    fn write_file_hook_fixture_mutation_leaf_open_rejects_symlink() {
        let dir = tempdir().expect("temp dir");
        let file_path = dir.path().join("raceme.txt");

        let result = execute_with_preopen_hook(
            FileOperation::write(&file_path, "ATTACK".to_owned()),
            Some(hook_swap_leaf_to_symlink),
        );

        match result {
            Err(ToolError::IoError(e)) => {
                // O_NOFOLLOW open on a symlink leaf fails: ELOOP on Linux,
                // EFTYPE on macOS/BSD. Accept either; the key is the open
                // failed rather than following the link.
                assert!(
                    matches!(e.raw_os_error(), Some(62) | Some(40) | Some(79)),
                    "expected ELOOP/EFTYPE from O_NOFOLLOW open, got {e:?}"
                );
            }
            other => panic!("expected io error from O_NOFOLLOW open, got {other:?}"),
        }
        let meta = fs::symlink_metadata(&file_path).expect("leaf still symlink");
        assert!(meta.file_type().is_symlink(), "leaf must remain a symlink, not be written through");
assert_eq!(
            fs::read_to_string(file_path.with_extension("sentinel_target")).expect("sentinel"),
            "external sentinel",
            "sentinel must be untouched"
        );
    }

    /// Parent-directory TOCTOU: hook swaps an intermediate parent directory
    /// for a symlink pointing outside the fixture tree AFTER the symlink
    /// precheck passed but BEFORE open_and_write. The vulnerable code follows
    /// the swapped parent and writes into the outside directory.
    /// Contract: operation must fail/deny before outside I/O; outside sentinel
    /// bytes must remain exact. This test MUST RED on the current seam because
    /// open_and_write uses a path-based open that resolves through the swapped
    /// parent symlink.
    #[cfg(unix)]
    fn hook_swap_parent_to_symlink(path: &Path, _append: bool) -> Result<(), ToolError> {
        // Derive deterministic fixture paths from the target path argument.
        // path = <tempdir>/real_parent/target.txt
        let parent = path.parent().expect("hook: path has parent");
        let leaf_name = path.file_name().expect("hook: path has leaf");
        let grandparent = parent.parent().unwrap_or_else(|| Path::new("/"));

        // Outside directory: sibling of grandparent, deterministically named.
        let outside_dir = grandparent.join("outside_escape_dir");
        fs::create_dir_all(&outside_dir).map_err(ToolError::IoError)?;

        // Sentinel file in outside dir with same leaf name so path resolution
        // through the swapped parent lands exactly here.
        let sentinel = outside_dir.join(leaf_name);
        fs::write(&sentinel, b"SENTINEL_UNCHANGED").map_err(ToolError::IoError)?;

        // Rename the real parent aside (atomic on same filesystem).
        let backup = parent.with_extension("real_parent_bak");
        fs::rename(parent, &backup).map_err(ToolError::IoError)?;

        // Replace the parent path with a symlink to the outside directory.
        std::os::unix::fs::symlink(&outside_dir, parent).map_err(ToolError::IoError)?;

        Ok(())
    }

    /// Proves the parent-directory TOCTOU vulnerability via the pre-open seam.
    /// The hook atomically replaces the checked intermediate parent with a
    /// symlink to an outside directory. The vulnerable open_and_write follows
    /// the swapped parent and overwrites the outside sentinel. A correct
    /// implementation must deny or fail before any outside I/O.
    #[test]
    #[cfg(unix)]
    fn write_file_parent_toctou_symlink_swap_must_not_escape() {
        let dir = tempdir().expect("temp dir");
        // Target: <tempdir>/real_parent/target.txt
        // The intermediate parent "real_parent" will be swapped by the hook.
        let file_path = dir.path().join("real_parent").join("target.txt");

        let result = execute_with_preopen_hook(
            FileOperation::write(&file_path, "ATTACK_PAYLOAD".to_owned()),
            Some(hook_swap_parent_to_symlink),
        );

        // Derive the outside sentinel path identically to the hook.
        let outside_dir = dir.path().join("outside_escape_dir");
        let sentinel = outside_dir.join("target.txt");

        // Contract: the operation must fail or deny before writing outside.
        // If it succeeded, the sentinel was overwritten -- that is the bug.
        let sentinel_bytes = fs::read(&sentinel).expect("sentinel must exist");
        assert_eq!(
            sentinel_bytes,
            b"SENTINEL_UNCHANGED",
            "PARENT TOCTOU: outside sentinel was modified through swapped parent symlink; \
             write_file must re-validate or use fd-relative open to prevent escape. \
             result={result:?}"
        );

        // Cleanup: restore real parent so tempdir cleanup works.
        let parent = dir.path().join("real_parent");
        let backup = dir.path().join("real_parent.real_parent_bak");
        if backup.exists() && parent.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false) {
            let _ = fs::remove_file(&parent);
            let _ = fs::rename(&backup, &parent);
        }
    }

    // --- APP-012-ROOT-ANCHOR-SEAM-W1: root FD observer seam tests ----

    /// Observer sees the opened root descriptor exactly once; None keeps the
    /// existing wrapper path and descriptor identity unchanged.
    #[test]
    #[cfg(unix)]
    fn open_write_root_observes_actual_fd_once_and_none_is_unchanged() {
        let mut calls = 0;
        let mut observed_identity = None;
        let mut observer = |fd: &std::os::fd::OwnedFd| {
            calls += 1;
            let stat = fstat(fd).map_err(|error| ToolError::IoError(error.into()))?;
            observed_identity = Some((stat.st_dev, stat.st_ino));
            Ok(())
        };
        let observed_fd = open_write_root_with_observer(Some(&mut observer))
            .expect("observer root open should succeed");
        assert_eq!(calls, 1, "observer must run once for the opened fd");
        let observed_stat = fstat(&observed_fd).expect("fstat observed fd");
        assert_eq!(
            observed_identity,
            Some((observed_stat.st_dev, observed_stat.st_ino)),
            "observer must receive the actual returned root fd"
        );

        let default_fd = open_write_root().expect("default root open should succeed");
        let explicit_none_fd = open_write_root_with_observer(None)
            .expect("None-observer root open should succeed");
        let default_stat = fstat(&default_fd).expect("fstat default fd");
        let none_stat = fstat(&explicit_none_fd).expect("fstat None fd");
        assert_eq!(
            (default_stat.st_dev, default_stat.st_ino),
            (none_stat.st_dev, none_stat.st_ino),
            "production wrapper must retain the None observer behavior"
        );
    }

    #[test]
    fn rooted_file_tool_writes_only_inside_project_root() {
        let project = tempdir().expect("project dir");
        let outside = tempdir().expect("outside dir");
        let tool = FileTool::with_project_root(project.path().to_path_buf());

        let inside = tool
            .execute(FileOperation::write(
                PathBuf::from("nested/in-project.txt"),
                "rooted".to_owned(),
            ))
            .expect("rooted write result");
        assert!(inside.success);
        assert_eq!(
            fs::read_to_string(project.path().join("nested/in-project.txt")).expect("inside"),
            "rooted"
        );

        let outside_path = outside.path().join("escape.txt");
        assert!(tool
            .execute(FileOperation::write(outside_path.clone(), "escape".to_owned()))
            .is_err());
        assert!(!outside_path.exists());
    }

    #[test]
    fn rooted_file_tool_rejects_parent_traversal_without_side_effects() {
        let project = tempdir().expect("project dir");
        let outside = project.path().parent().expect("project parent");
        let escaped = outside.join("rooted-traversal-denied.txt");
        let tool = FileTool::with_project_root(project.path().to_path_buf());

        assert!(tool
            .execute(FileOperation::write(
                PathBuf::from("../rooted-traversal-denied.txt"),
                "escape".to_owned(),
            ))
            .is_err());
        assert!(!escaped.exists());
    }

    // --- APP-012-ROOT-ANCHOR-RED-W1: frozen RED root anchor contract ---
    #[cfg(unix)]
    mod root_anchor_red {
        use super::*;
        use std::os::fd::{AsFd, OwnedFd};

        /// RAII cwd swap mirroring the serialized CwdGuard pattern in
        /// `crates/server/tests/live_file_tool_symlink_escape.rs:74-89`.
        /// Safe under the lane convention `--test-threads=1`.
        struct CwdGuard {
            original: std::path::PathBuf,
        }

        impl CwdGuard {
            fn set(path: &Path) -> std::io::Result<Self> {
                let original = std::env::current_dir()?;
                std::env::set_current_dir(path)?;
                Ok(Self { original })
            }
        }

        impl Drop for CwdGuard {
            fn drop(&mut self) {
                let _ = std::env::set_current_dir(&self.original);
            }
        }

        /// Frozen RED contract: the write-root descriptor must be anchored on
        /// the approved project directory (the current working directory), not
        /// the filesystem root. The observer fstats the actually opened fd and
        /// compares device/inode against the approved project directory, whose
        /// identity is derived inside the callback. Setup failures panic with a
        /// distinct `setup:` prefix; the expected failure is a genuine
        /// observer-produced `ToolError` from `open_write_root_with_observer`.
        #[test]
        fn open_write_root_anchors_opened_fd_on_approved_project_dir() {
            let dir = tempdir().expect("setup: disposable approved project dir");
            let _guard = CwdGuard::set(dir.path()).expect("setup: set current_dir");

            let mut observer = |fd: &OwnedFd| -> Result<(), ToolError> {
                let opened = fstat(fd.as_fd()).map_err(|e| ToolError::IoError(e.into()))?;
                let cwd = std::env::current_dir()
                    .map_err(|e| ToolError::IoError(e.into()))?;
                let project = std::fs::File::open(&cwd)
                    .map_err(|e| ToolError::IoError(e.into()))?;
                let wanted = fstat(project.as_fd())
                    .map_err(|e| ToolError::IoError(e.into()))?;
                if (opened.st_dev, opened.st_ino) == (wanted.st_dev, wanted.st_ino) {
                    Ok(())
                } else {
                    Err(ToolError::FileError(format!(
                        "root anchor descriptor mismatch: opened fd is ({}, {}), approved project dir {} is ({}, {})",
                        opened.st_dev,
                        opened.st_ino,
                        cwd.display(),
                        wanted.st_dev,
                        wanted.st_ino,
                    )))
                }
            };

            let root = open_write_root_with_observer(Some(&mut observer))
                .expect("root anchor must equal approved project root");
            let opened = fstat(root.as_fd()).expect("setup: fstat returned root fd");
            let project_file =
                std::fs::File::open(dir.path()).expect("setup: open approved project dir");
            let wanted = fstat(project_file.as_fd()).expect("setup: fstat approved project dir");
            assert_eq!(
                (opened.st_dev, opened.st_ino),
                (wanted.st_dev, wanted.st_ino),
                "opened root fd identity must equal the approved project directory"
            );
        }
    }
    // --- END APP-012-ROOT-ANCHOR-RED-W1 ---
}
