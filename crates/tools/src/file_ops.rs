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
pub struct FileTool;

impl FileTool {
    /// Creates a new FileTool instance.
    pub fn new() -> Self {
        Self
    }

    /// Executes a file operation and returns the result.
    pub fn execute(&self, op: FileOperation) -> Result<FileResult, ToolError> {
        execute(op)
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
        execute_authorized(op, broker)
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

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn path_has_symlink_component(path: &Path) -> Result<bool, ToolError> {
    let cwd = normalize_platform_path(&std::env::current_dir().map_err(ToolError::IoError)?);
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    let absolute = normalize_platform_path(&joined);
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
    let normalized = normalize_write_path(path)?;
    if path_has_symlink_component(path)? {
        return Ok(FileResult::failure(
            "write denied: path contains a symlink component",
        ));
    }
    let leaf = normalized
        .components
        .last()
        .ok_or_else(|| ToolError::FileError("write denied: path has no leaf".to_owned()))?;
    let parent_components = &normalized.components[..normalized.components.len() - 1];
    let root = open_write_root()?;
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
        hook(path, append)?;
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
        content.len(),
        path
    )))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn normalize_write_path(path: &Path) -> Result<NormalizedWritePath, ToolError> {
    if path.as_os_str().len() > MAX_WRITE_PATH_BYTES {
        return Err(ToolError::FileError("write denied: path too long".to_owned()));
    }
    let cwd = std::env::current_dir().map_err(ToolError::IoError)?;
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    let absolute = normalize_platform_path(&joined);
    let mut components = Vec::new();
    for component in absolute.components() {
        match component {
            std::path::Component::RootDir => {}
            std::path::Component::CurDir => {}
            std::path::Component::Normal(name) => components.push(name.to_owned()),
            std::path::Component::ParentDir => {
                return Err(ToolError::FileError(
                    "write denied: path traversal".to_owned(),
                ));
            }
            std::path::Component::Prefix(_) => {
                return Err(ToolError::FileError(
                    "write denied: unsupported path prefix".to_owned(),
                ));
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
fn is_symlink_error(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn open_write_root() -> Result<std::os::fd::OwnedFd, ToolError> {
    openat(
        rustix_fs::CWD,
        Path::new("/"),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| ToolError::IoError(error.into()))
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
}
