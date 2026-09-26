#![forbid(unsafe_code)]
//! Diff viewer presentation state (std-only).
//!
//! Mirrors `packages/tui/src/feature-plugins/system/diff-viewer.tsx`
//! (TS checkout a0d9b6c): `DiffView` (`:48`), hunk scan
//! `line.startsWith("@@")` (`:290-292`), `jumpRelativeHunk` (`:282-315`),
//! `jumpRelativePatchFile`/`movePatchFileIndex` (`:270-280`,
//! `diff-viewer-file-tree-utils.ts:186-191`), `toggleSelectedFileTreeRow`
//! (`:399-406` + `toggleFileTreeDirectory` in
//! `diff-viewer-file-tree-utils.ts:197-203`), `diff.toggle_view`
//! (`:663-673`), file-tree expand markers (`diff-viewer-file-tree.tsx:148`).
//! Reuses `crate::revert_diff::FileDiff`; counts hunks from stored
//! per-file `hunk_count` (no renderable/scroll state ported).

use crate::revert_diff::FileDiff;

/// Cap on files held by [`ViewerState`].
pub const MAX_FILES: usize = 256;
/// Cap on children held by one [`FileNode`].
pub const MAX_CHILDREN: usize = 64;
/// Cap on path chars kept.
pub const MAX_PATH_LEN: usize = 512;

/// Split/unified layout (`diff-viewer.tsx:48`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffView {
    #[default]
    Split,
    Unified,
}

/// File-tree node; children truncated at [`MAX_CHILDREN`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileNode {
    pub path: String,
    pub expanded: bool,
    pub hunk_count: usize,
    pub hunk: usize,
    pub children: Vec<FileNode>,
}

impl FileNode {
    #[must_use]
    pub fn new(path: &str, hunk_count: usize) -> Self {
        Self {
            path: path.chars().take(MAX_PATH_LEN).collect(),
            expanded: true,
            hunk_count,
            hunk: 0,
            children: Vec::new(),
        }
    }

    /// Push child; false when full (bounded, drops excess).
    pub fn push_child(&mut self, child: FileNode) -> bool {
        if self.children.len() >= MAX_CHILDREN {
            return false;
        }
        self.children.push(child);
        true
    }

    pub fn toggle_expand(&mut self) {
        self.expanded = !self.expanded;
    }
}

/// Count hunks in a unified patch (`@@` lines; `diff-viewer.tsx:292`).
#[must_use]
pub fn count_hunks(patch: &str) -> usize {
    patch.lines().filter(|l| l.starts_with("@@")).count()
}

/// Flat viewer state in file-tree order (`orderedPatchFileIndexes`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ViewerState {
    pub files: Vec<FileNode>,
    pub focus: usize,
    pub view: DiffView,
}

impl ViewerState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Build from parsed diffs + parallel patches (hunks counted per file).
    #[must_use]
    pub fn from_diffs(diffs: &[FileDiff], patches: &[&str]) -> Self {
        let mut s = Self::new();
        for (i, d) in diffs.iter().enumerate() {
            let n = patches.get(i).map_or(0, |p| count_hunks(p));
            s.add_file(&d.path, n);
        }
        s
    }

    /// Push file; false when [`MAX_FILES`] reached.
    pub fn add_file(&mut self, path: &str, hunk_count: usize) -> bool {
        if self.files.len() >= MAX_FILES {
            return false;
        }
        self.files.push(FileNode::new(path, hunk_count));
        true
    }

    fn focused_mut(&mut self) -> Option<&mut FileNode> {
        self.files.get_mut(self.focus)
    }

    /// Toggle focused node (`toggleSelectedFileTreeRow`).
    pub fn toggle_expand(&mut self) {
        if let Some(f) = self.focused_mut() {
            f.toggle_expand();
        }
    }

    /// Flip split/unified (`diff.toggle_view`, `:669`).
    pub fn toggle_view(&mut self) {
        self.view = match self.view {
            DiffView::Split => DiffView::Unified,
            DiffView::Unified => DiffView::Split,
        };
    }

    /// Next file, clamped at end (`movePatchFileIndex`, `:190`).
    pub fn next_file(&mut self) {
        if self.files.is_empty() {
            return;
        }
        self.focus = (self.focus + 1).min(self.files.len() - 1);
    }

    /// Previous file, clamped at start.
    pub fn prev_file(&mut self) {
        if self.files.is_empty() {
            return;
        }
        self.focus = self.focus.saturating_sub(1);
    }

    /// Next hunk; rolls into first hunk of next file (`jumpRelativeHunk`).
    pub fn next_hunk(&mut self) {
        let Some(f) = self.files.get_mut(self.focus) else {
            return;
        };
        if f.hunk + 1 < f.hunk_count.max(1) && f.hunk_count > 0 {
            f.hunk += 1;
            return;
        }
        if self.focus + 1 < self.files.len() {
            self.focus += 1;
            if let Some(n) = self.files.get_mut(self.focus) {
                n.hunk = 0;
            }
        }
    }

    /// Previous hunk; rolls into last hunk of previous file.
    pub fn prev_hunk(&mut self) {
        let Some(f) = self.files.get_mut(self.focus) else {
            return;
        };
        if f.hunk > 0 {
            f.hunk -= 1;
            return;
        }
        if self.focus > 0 {
            self.focus -= 1;
            if let Some(p) = self.files.get_mut(self.focus) {
                p.hunk = p.hunk_count.saturating_sub(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> ViewerState {
        let mut s = ViewerState::new();
        s.add_file("a.ts", 2);
        s.add_file("b.ts", 1);
        s
    }

    #[test]
    fn toggle_view_flips() {
        let mut s = state();
        assert_eq!(s.view, DiffView::Split);
        s.toggle_view();
        assert_eq!(s.view, DiffView::Unified);
        s.toggle_view();
        assert_eq!(s.view, DiffView::Split);
    }

    #[test]
    fn next_prev_file_clamp() {
        let mut s = state();
        s.prev_file();
        assert_eq!(s.focus, 0);
        s.next_file();
        assert_eq!(s.focus, 1);
        s.next_file();
        assert_eq!(s.focus, 1);
        s.prev_file();
        assert_eq!(s.focus, 0);
    }

    #[test]
    fn toggle_expand_flips_focused() {
        let mut s = state();
        assert!(s.files[0].expanded);
        s.toggle_expand();
        assert!(!s.files[0].expanded);
        s.next_file();
        s.toggle_expand();
        assert!(!s.files[1].expanded);
        assert!(!s.files[0].expanded);
    }

    #[test]
    fn next_hunk_rolls_to_next_file() {
        let mut s = state();
        s.next_hunk();
        assert_eq!((s.focus, s.files[0].hunk), (0, 1));
        s.next_hunk();
        assert_eq!((s.focus, s.files[1].hunk), (1, 0));
        s.next_hunk();
        assert_eq!((s.focus, s.files[1].hunk), (1, 0));
    }

    #[test]
    fn prev_hunk_rolls_to_prev_file_tail() {
        let mut s = state();
        s.next_file();
        s.prev_hunk();
        assert_eq!((s.focus, s.files[0].hunk), (0, 1));
        s.prev_hunk();
        assert_eq!((s.focus, s.files[0].hunk), (0, 0));
        s.prev_hunk();
        assert_eq!(s.focus, 0);
    }

    #[test]
    fn files_bounded() {
        let mut s = ViewerState::new();
        for i in 0..MAX_FILES + 10 {
            let ok = s.add_file(&format!("f{i}.ts"), 0);
            assert_eq!(ok, i < MAX_FILES);
        }
        assert_eq!(s.files.len(), MAX_FILES);
    }

    #[test]
    fn children_bounded() {
        let mut n = FileNode::new("dir", 0);
        for i in 0..MAX_CHILDREN + 5 {
            let ok = n.push_child(FileNode::new(&format!("f{i}"), 0));
            assert_eq!(ok, i < MAX_CHILDREN);
        }
        assert_eq!(n.children.len(), MAX_CHILDREN);
    }

    #[test]
    fn count_hunks_scans_at_headers() {
        assert_eq!(count_hunks("@@ -1 +1 @@\nx\n@@ -2 +2 @@\ny\n"), 2);
        assert_eq!(count_hunks("no hunks\n"), 0);
    }

    #[test]
    fn from_diffs_reuses_file_diff() {
        let diffs = vec![FileDiff { path: "a.ts".into(), additions: 1, deletions: 0 }];
        let s = ViewerState::from_diffs(&diffs, &["@@ -1 +1 @@\n+a\n"]);
        assert_eq!(s.files.len(), 1);
        assert_eq!(s.files[0].path, "a.ts");
        assert_eq!(s.files[0].hunk_count, 1);
    }
}
