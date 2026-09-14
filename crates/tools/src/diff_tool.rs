//! Text diff tool for comparing old and new content.
//!
//! Provides DiffTool for line-based diffing with options to ignore
//! whitespace and case differences.

use serde::Serialize;

/// Options controlling diff behavior.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DiffOptions {
    /// Whether to ignore leading/trailing whitespace when comparing lines.
    #[serde(default)]
    pub ignore_whitespace: bool,

    /// Whether to ignore character case when comparing lines.
    #[serde(default)]
    pub ignore_case: bool,
}

/// Request to diff two pieces of text.
#[derive(Debug, Clone, Serialize)]
pub struct DiffRequest {
    /// Original text content.
    pub old_content: String,

    /// New text content to compare against.
    pub new_content: String,

    /// Options for the diff operation.
    #[serde(default)]
    pub options: DiffOptions,
}

/// Type of change for a single line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ChangeType {
    /// A new line was added.
    Add,
    /// An existing line was removed.
    Delete,
    /// A line was modified (old line replaced).
    Replace,
}

/// A single line-level change in a diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiffChange {
    /// The type of change: Add, Delete, or Replace.
    #[serde(rename = "type")]
    pub change_type: ChangeType,

    /// The old line content if applicable.
    pub old_line: Option<String>,

    /// The new line content if applicable.
    pub new_line: Option<String>,

    /// The line number where this change occurs (1-based).
    pub line_number: usize,
}

/// Result of a diff operation.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DiffResult {
    /// Whether any changes were detected.
    pub has_changes: bool,

    /// List of individual line-level changes.
    #[serde(default)]
    pub changes: Vec<DiffChange>,

    /// Total number of lines in the old content.
    pub old_line: usize,

    /// Total number of lines in the new content.
    pub new_line: usize,
}

/// DiffTool for text comparison.
pub struct DiffTool;

impl DiffTool {
    /// Creates a new DiffTool instance.
    pub fn new() -> Self {
        Self
    }

    /// Executes a diff request and returns the result.
    pub fn execute(&self, request: DiffRequest) -> DiffResult {
        diff(&request.old_content, &request.new_content, &request.options)
    }
}

impl Default for DiffTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Normalizes a line according to diff options.
fn normalize_line(line: &str, opts: &DiffOptions) -> String {
    let mut result = line.to_string();

    if opts.ignore_whitespace {
        result = result.trim().to_string();
    }

    if opts.ignore_case {
        result = result.to_lowercase();
    }

    result
}

/// Splits content into lines without trailing newlines.
fn split_lines(content: &str) -> Vec<String> {
    if content.is_empty() {
        return vec![];
    }
    content
        .lines()
        .map(|l| l.to_string())
        .collect()
}

/// Computes a line-based diff between two text contents.
///
/// Returns a DiffResult with all detected changes (Add, Delete, Replace).
/// Uses a simple LCS-style algorithm for line alignment.
pub fn diff(old_content: &str, new_content: &str, options: &DiffOptions) -> DiffResult {
    let old_lines = split_lines(old_content);
    let new_lines = split_lines(new_content);

    let old_normalized: Vec<String> = old_lines
        .iter()
        .map(|l| normalize_line(l, options))
        .collect();
    let new_normalized: Vec<String> = new_lines
        .iter()
        .map(|l| normalize_line(l, options))
        .collect();

    let changes = compute_diff(&old_normalized, &new_normalized, &old_lines, &new_lines, options);

    DiffResult {
        has_changes: !changes.is_empty(),
        changes,
        old_line: old_lines.len(),
        new_line: new_lines.len(),
    }
}

/// Internal diff computation using LCS for alignment.
fn compute_diff(
    old_norm: &[String],
    new_norm: &[String],
    old_orig: &[String],
    new_orig: &[String],
    _opts: &DiffOptions,
) -> Vec<DiffChange> {
    let lcs = lcs_indices(old_norm, new_norm);
    let mut changes = Vec::new();

    let mut old_idx = 0usize;
    let mut new_idx = 0usize;
    let mut line_number = 1usize;

    loop {
        if old_idx < old_norm.len() && new_idx < new_norm.len() {
            // Check if current lines are matched in LCS
            let in_lcs_old = lcs.iter().any(|&(oi, _)| oi == old_idx);
            let in_lcs_new = lcs.iter().any(|&(_, ni)| ni == new_idx);

            if in_lcs_old && in_lcs_new {
                // Both are matched - advance both
                old_idx += 1;
                new_idx += 1;
                line_number += 1;
            } else if in_lcs_old {
                // Old is matched, new is not - it's an Add
                changes.push(DiffChange {
                    change_type: ChangeType::Add,
                    old_line: None,
                    new_line: Some(new_orig[new_idx].clone()),
                    line_number,
                });
                new_idx += 1;
                line_number += 1;
            } else if in_lcs_new {
                // New is matched, old is not - it's a Delete
                changes.push(DiffChange {
                    change_type: ChangeType::Delete,
                    old_line: Some(old_orig[old_idx].clone()),
                    new_line: None,
                    line_number,
                });
                old_idx += 1;
                line_number += 1;
            } else {
                // Neither is matched - Replace
                changes.push(DiffChange {
                    change_type: ChangeType::Replace,
                    old_line: Some(old_orig[old_idx].clone()),
                    new_line: Some(new_orig[new_idx].clone()),
                    line_number,
                });
                old_idx += 1;
                new_idx += 1;
                line_number += 1;
            }
        } else if old_idx < old_norm.len() {
            // Remaining old lines are deletions
            changes.push(DiffChange {
                change_type: ChangeType::Delete,
                old_line: Some(old_orig[old_idx].clone()),
                new_line: None,
                line_number,
            });
            old_idx += 1;
            line_number += 1;
        } else if new_idx < new_norm.len() {
            // Remaining new lines are additions
            changes.push(DiffChange {
                change_type: ChangeType::Add,
                old_line: None,
                new_line: Some(new_orig[new_idx].clone()),
                line_number,
            });
            new_idx += 1;
            line_number += 1;
        } else {
            break;
        }
    }

    changes
}

/// Computes the longest common subsequence of matching line indices.
/// Returns pairs of (old_index, new_index) that match.
fn lcs_indices(old: &[String], new: &[String]) -> Vec<(usize, usize)> {
    let n = old.len();
    let m = new.len();

    if n == 0 || m == 0 {
        return vec![];
    }

    // Build DP table
    let mut dp = vec![vec![0usize; m + 1]; n + 1];

    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if old[i] == new[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    // Backtrack to find matching pairs
    let mut result = Vec::new();
    let mut i = 0usize;
    let mut j = 0usize;

    while i < n && j < m {
        if old[i] == new[j] {
            result.push((i, j));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }

    result
}

/// Generates unified diff format output from diff changes.
///
/// Produces lines prefixed with `-` (removed), `+` (added), or ` ` (context)
/// in the standard unified diff style.
pub fn unified_diff(changes: &[DiffChange]) -> String {
    let mut output = String::new();

    for change in changes {
        match change.change_type {
            ChangeType::Add => {
                output.push_str(&format!("+{}\n", change.new_line.clone().unwrap_or_default()));
            }
            ChangeType::Delete => {
                output.push_str(&format!("-{}\n", change.old_line.clone().unwrap_or_default()));
            }
            ChangeType::Replace => {
                output.push_str(&format!("-{}\n", change.old_line.clone().unwrap_or_default()));
                output.push_str(&format!("+{}\n", change.new_line.clone().unwrap_or_default()));
            }
        }
    }

    output
}

/// Generates a side-by-side diff representation from diff changes.
///
/// Returns a formatted string showing old and new lines side by side
/// with change type indicators.
pub fn side_by_side_diff(changes: &[DiffChange]) -> String {
    let mut output = String::new();
    output.push_str("OLD                         | CHANGE | NEW\n");
    output.push_str("------------------------------+--------+------------------------------\n");

    for change in changes {
        let old_str = change.old_line.clone().unwrap_or_default();
        let new_str = change.new_line.clone().unwrap_or_default();
        let type_char = match change.change_type {
            ChangeType::Add => "A",
            ChangeType::Delete => "D",
            ChangeType::Replace => "R",
        };

        // Pad/truncate to fixed width
        let old_padded = format!("{:<30}", &old_str[..old_str.len().min(30)]);
        let new_padded = format!("{:<30}", &new_str[..new_str.len().min(30)]);

        output.push_str(&format!("{} |  {}     | {}\n", old_padded, type_char, new_padded));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_change() {
        let old = "hello\nworld\n";
        let new = "hello\nrust\n";

        let result = diff(old, new, &DiffOptions::default());

        assert!(result.has_changes);
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].change_type, ChangeType::Replace);
        assert_eq!(result.changes[0].old_line.as_deref(), Some("world"));
        assert_eq!(result.changes[0].new_line.as_deref(), Some("rust"));
        assert!(result.changes[0].line_number >= 1);
    }

    #[test]
    fn detect_add() {
        let old = "hello\n";
        let new = "hello\nworld\n";

        let result = diff(old, new, &DiffOptions::default());

        assert!(result.has_changes);
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].change_type, ChangeType::Add);
        assert!(result.changes[0].old_line.is_none());
        assert_eq!(result.changes[0].new_line.as_deref(), Some("world"));
    }

    #[test]
    fn detect_delete() {
        let old = "hello\nworld\n";
        let new = "hello\n";

        let result = diff(old, new, &DiffOptions::default());

        assert!(result.has_changes);
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].change_type, ChangeType::Delete);
        assert_eq!(result.changes[0].old_line.as_deref(), Some("world"));
        assert!(result.changes[0].new_line.is_none());
    }

    #[test]
    fn diff_options() {
        let old = "Hello World\n";
        let new = "hello world\n";

        // Without options, content differs
        let result_default = diff(old, new, &DiffOptions::default());
        assert!(result_default.has_changes);

        // With ignore_case, content is treated as same
        let result_ignore_case = diff(
            old,
            new,
            &DiffOptions { ignore_case: true, ignore_whitespace: false },
        );
        assert!(!result_ignore_case.has_changes);
        assert!(result_ignore_case.changes.is_empty());

        // With ignore_whitespace, leading/trailing whitespace differences are ignored
        let result_ignore_ws = diff(
            "Hello  ",
            "Hello",
            &DiffOptions { ignore_whitespace: true, ignore_case: false },
        );
        assert!(!result_ignore_ws.has_changes);
    }

    #[test]
    fn unified_output() {
        let old = "hello\nworld\n";
        let new = "hello\nrust\n";

        let result = diff(old, new, &DiffOptions::default());
        let unified = unified_diff(&result.changes);

        assert!(unified.contains("-world"));
        assert!(unified.contains("+rust"));
        assert!(!unified.contains("\n hello"));
    }
}
