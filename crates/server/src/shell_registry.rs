use std::{collections::BTreeMap, sync::{Arc, Mutex}};

use serde::Serialize;

const MAX_SHELL_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_OUTPUT_LIMIT: usize = 1024 * 1024;

#[derive(Clone, Default)]
pub struct ShellRegistry(Arc<Mutex<BTreeMap<String, ShellRecord>>>);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellRecord {
    pub id: String,
    pub session_id: String,
    pub command: String,
    pub cwd: String,
    pub status: String,
    pub output: String,
    pub size: u64,
    pub cursor: u64,
    pub exit_code: Option<i32>,
}

#[derive(Serialize)]
pub struct ShellOutput {
    pub output: String,
    pub cursor: u64,
    pub size: u64,
}

#[derive(Debug)]
pub enum ShellError { NotFound }

impl ShellRegistry {
    pub fn start(&self, id: String, session_id: String, command: String, cwd: String) {
        let record = ShellRecord { id: id.clone(), session_id, command, cwd, status: "running".into(), output: String::new(), size: 0, cursor: 0, exit_code: None };
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(id, record);
    }

    /// Append a live stdout/stderr chunk while the command still runs, so a
    /// `GET /api/shell/{id}/output` poll observes output before completion
    /// (`routes/session/index.tsx:3040-3058`). Bounded to the retained tail.
    pub fn append(&self, id: &str, chunk: &str) {
        let mut shells = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(shell) = shells.get_mut(id) else { return };
        shell.output.push_str(chunk);
        if shell.output.len() > MAX_SHELL_OUTPUT_BYTES {
            shell.output = tail(&shell.output, MAX_SHELL_OUTPUT_BYTES);
        }
        shell.size = shell.output.len() as u64;
        shell.cursor = shell.size;
    }

    pub fn complete(&self, id: &str, output: &str, success: bool) {
        let mut shells = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(shell) = shells.get_mut(id) else { return };
        let output = tail(output, MAX_SHELL_OUTPUT_BYTES);
        shell.size = output.len() as u64;
        shell.cursor = shell.size;
        shell.output = output;
        shell.status = "completed".into();
        shell.exit_code = Some(i32::from(!success));
    }

    pub fn list(&self) -> Vec<ShellRecord> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).values().cloned().collect()
    }

    pub fn get(&self, id: &str) -> Result<ShellRecord, ShellError> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(id).cloned().ok_or(ShellError::NotFound)
    }

    pub fn output(&self, id: &str, cursor: u64, limit: usize) -> Result<ShellOutput, ShellError> {
        let shells = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let shell = shells.get(id).ok_or(ShellError::NotFound)?;
        let start = usize::try_from(cursor).unwrap_or(usize::MAX).min(shell.output.len());
        let start = char_boundary_at_or_after(&shell.output, start);
        let end = char_boundary_at_or_before(&shell.output, start.saturating_add(limit.min(MAX_OUTPUT_LIMIT)).min(shell.output.len()));
        let text = shell.output[start..end].to_owned();
        Ok(ShellOutput { cursor: end as u64, size: shell.size, output: text })
    }
}

fn tail(text: &str, limit: usize) -> String {
    let mut start = text.len().saturating_sub(limit);
    start = char_boundary_at_or_after(text, start);
    text[start..].to_owned()
}

fn char_boundary_at_or_after(text: &str, mut index: usize) -> usize {
    while index < text.len() && !text.is_char_boundary(index) { index += 1; }
    index
}

fn char_boundary_at_or_before(text: &str, mut index: usize) -> usize {
    while index > 0 && !text.is_char_boundary(index) { index -= 1; }
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool006_output_uses_cursor_and_bounded_limit() {
        let registry = ShellRegistry::default();
        registry.start("sh-1".into(), "ses-1".into(), "printf hi".into(), "/work".into());
        registry.complete("sh-1", "αβγδε", true);
        let chunk = registry.output("sh-1", 0, 4).unwrap();
        assert_eq!(chunk.output, "αβ");
        assert_eq!(chunk.cursor, 4);
        assert_eq!(chunk.size, "αβγδε".len() as u64);
        assert!(matches!(registry.output("missing", 0, 1), Err(ShellError::NotFound)));
    }

    // TOOL-006: a running shell's output is observable before completion —
    // `append` advances the projection a live poll reads, and `complete`
    // finalizes the same record.
    #[test]
    fn tool006_running_shell_output_is_live() {
        let registry = ShellRegistry::default();
        registry.start("sh-live".into(), "ses-1".into(), "long job".into(), "/work".into());
        let first = registry.get("sh-live").unwrap();
        assert_eq!(first.status, "running");
        assert!(first.output.is_empty());
        registry.append("sh-live", "partial ");
        let mid = registry.output("sh-live", 0, 1024).unwrap();
        assert_eq!(mid.output, "partial ");
        assert_eq!(mid.cursor, mid.size);
        registry.append("sh-live", "more");
        let next = registry.output("sh-live", mid.cursor, 1024).unwrap();
        assert_eq!(next.output, "more");
        registry.complete("sh-live", "partial more done", true);
        let done = registry.get("sh-live").unwrap();
        assert_eq!(done.status, "completed");
        assert_eq!(done.exit_code, Some(0));
        assert_eq!(done.output, "partial more done");
    }
}
