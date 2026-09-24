//! File operations tool implementation.
//!
//! Provides FileTool for file system operations including read, write,
//! list directory, and create directory.

use opencode_rk_security::{Decision, FileAction, OperationIntent, PermissionBroker};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::{fs, io::{Read, Seek, SeekFrom, Write}};
use thiserror::Error;

/// Hard cap for one file-read result. Callers may request a smaller limit.
pub const MAX_READ_BYTES: usize = 64 * 1024;

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
/// Every operation authorizes its concrete path before filesystem I/O.
/// `Deny` and `RequireHuman` return a fixed failure without reading, creating,
/// truncating, listing, or making parent directories.
pub fn execute_authorized(
    op: FileOperation,
    broker: &PermissionBroker,
) -> Result<FileResult, ToolError> {
    let (action, path) = match &op {
        FileOperation::Read { path, .. } | FileOperation::List { path } => {
            (FileAction::Read, path)
        }
        FileOperation::Write { path, .. } => (FileAction::Write, path),
        FileOperation::CreateDir { path, .. } => (FileAction::CreateDirectory, path),
    };
    let intent = OperationIntent::File {
        action,
        path: path.clone(),
    };
    match broker.authorize(&intent) {
        Decision::Allow => (),
        Decision::Deny { reason } => {
            return Ok(FileResult::failure(format!("file operation denied: {reason}")));
        }
        Decision::RequireHuman { reason, .. } => {
            return Ok(FileResult::failure(format!(
                "file operation requires human approval: {reason}"
            )));
        }
    }
    execute(op)
}

/// Executes a file operation and returns the result.
pub fn execute(op: FileOperation) -> Result<FileResult, ToolError> {
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
        } => write_file(&path, &content, append),
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
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(FileResult::failure("File not found"));
        }
        Err(error) => return Err(ToolError::IoError(error)),
    };
    let offset = offset.unwrap_or(0);
    let file_len = file.metadata().map_err(ToolError::IoError)?.len();
    if u64::try_from(offset).unwrap_or(u64::MAX) > file_len {
        return Ok(FileResult::failure("read offset exceeds file length"));
    }
    file.seek(SeekFrom::Start(offset as u64))
        .map_err(ToolError::IoError)?;
    let limit = limit.unwrap_or(MAX_READ_BYTES).min(MAX_READ_BYTES);
    let mut bytes = Vec::with_capacity(limit.min(8 * 1024));
    file.take(limit as u64)
        .read_to_end(&mut bytes)
        .map_err(ToolError::IoError)?;
    match String::from_utf8(bytes) {
        Ok(content) => Ok(FileResult::success(content)),
        Err(_) => Ok(FileResult::failure("file is not valid UTF-8")),
    }
}

/// Writes content to a file at the given path.
fn write_file(path: &Path, content: &str, append: bool) -> Result<FileResult, ToolError> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(ToolError::IoError)?;
        }
    }

    if append {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(ToolError::IoError)?;
        file.write_all(content.as_bytes())
            .map_err(ToolError::IoError)?;
    } else {
        fs::write(path, content).map_err(ToolError::IoError)?;
    }

    Ok(FileResult::success(format!(
        "Successfully wrote {} bytes to {:?}",
        content.len(),
        path
    )))
}

/// Lists the contents of a directory.
fn list_dir(path: &Path) -> Result<FileResult, ToolError> {
    if !path.exists() {
        return Ok(FileResult::failure("directory not found"));
    }

    if !path.is_dir() {
        return Ok(FileResult::failure("path is not a directory"));
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
}
