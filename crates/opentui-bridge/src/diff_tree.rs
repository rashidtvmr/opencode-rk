#![forbid(unsafe_code)]
//! Flat file-tree + hunk-expand helpers (std-only).
//!
//! Mirrors TS checkout a0d9b6c:
//! - depth-first walk with collapsed skip: `flattenFileTree`
//!   (`diff-viewer-file-tree-utils.ts:76-104`, gate at `:100`)
//! - expand markers collapsed `▸` / expanded `▾`
//!   (`diff-viewer-file-tree.tsx:148`)
//! - toggle row: files jump, dirs flip (`diff-viewer.tsx:399-406` +
//!   `toggleFileTreeDirectory`, `utils:197-203`)
//! - `diff.expand` already-expanded moves to first child (`:516-532` +
//!   `moveFileTreeSelectionToFirstChild`, `utils:129-135`)
//! - `diff.collapse` non-dir/collapsed moves to parent (`:545-562` +
//!   `moveFileTreeSelectionToParent`, `utils:137-142`)
//! - `diff.expand_all` all dirs expanded (`:534-543` +
//!   `allExpandedFileTreeDirectories`, `utils:193-195`)
//! - default all-expanded on tree build (`diff-viewer.tsx:178-179`)
//! - hunk scan `line.startsWith("@@")` (`diff-viewer.tsx:290-292`)
//!
//! Reuses `crate::diff_viewer::FileNode` (`expanded` starts true,
//! `diff_viewer.rs:47`; `toggle_expand`, `diff_viewer.rs:63-65`).
//! Single-child dir-chain joining (`utils:91-99,106-111`) stays a render
//! concern; paths here are emitted as stored.

use crate::diff_viewer::FileNode;

/// Max depth descended by [`TreeFlatten`] (node itself still emitted).
pub const MAX_DEPTH: usize = 32;
/// Max hunks held by [`HunkExpand`].
pub const MAX_HUNKS: usize = 1024;
/// Max query chars kept by [`TreeFilter`].
pub const MAX_QUERY_LEN: usize = 256;

/// Depth-first flatten over a forest; collapsed subtrees skipped
/// (`flattenFileTree` gate, `utils:100`).
pub struct TreeFlatten;

impl TreeFlatten {
    /// Flatten `nodes` depth-first; children of `!expanded` nodes and
    /// anything deeper than [`MAX_DEPTH`] omitted.
    #[must_use]
    pub fn flatten(nodes: &[FileNode]) -> Vec<String> {
        let mut out = Vec::new();
        for node in nodes {
            Self::visit(node, 0, &mut out);
        }
        out
    }

    fn visit(node: &FileNode, depth: usize, out: &mut Vec<String>) {
        out.push(node.path.clone());
        if !node.expanded || depth >= MAX_DEPTH {
            return;
        }
        for child in &node.children {
            Self::visit(child, depth + 1, out);
        }
    }
}

/// Case-insensitive substring filter over [`TreeFlatten::flatten`].
pub struct TreeFilter;

impl TreeFilter {
    /// Filter flattened paths; empty query returns all.
    #[must_use]
    pub fn filter(nodes: &[FileNode], query: &str) -> Vec<String> {
        let q: String = query.chars().take(MAX_QUERY_LEN).collect::<String>().to_lowercase();
        TreeFlatten::flatten(nodes)
            .into_iter()
            .filter(|p| q.is_empty() || p.to_lowercase().contains(&q))
            .collect()
    }
}

/// Per-hunk expand bits; default all true (viewer default all-expanded,
/// `diff-viewer.tsx:178-179`); `toggle` flips like
/// `toggleFileTreeDirectory` (`utils:197-203`); `expand`/`collapse` set
/// like `setFileTreeDirectoryExpanded` (`utils:205-216`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HunkExpand {
    pub expanded: Vec<bool>,
}

impl HunkExpand {
    /// New bits for `hunk_count` hunks, truncated at [`MAX_HUNKS`].
    #[must_use]
    pub fn new(hunk_count: usize) -> Self {
        Self { expanded: vec![true; hunk_count.min(MAX_HUNKS)] }
    }

    /// Flip bit `i`; out-of-bounds ignored.
    pub fn toggle(&mut self, i: usize) {
        if let Some(e) = self.expanded.get_mut(i) {
            *e = !*e;
        }
    }

    /// Set bit `i` (`diff.expand`, `:526-528`); out-of-bounds ignored.
    pub fn expand(&mut self, i: usize) {
        if let Some(e) = self.expanded.get_mut(i) {
            *e = true;
        }
    }

    /// Clear bit `i` (`diff.collapse`, `:556-558`); out-of-bounds ignored.
    pub fn collapse(&mut self, i: usize) {
        if let Some(e) = self.expanded.get_mut(i) {
            *e = false;
        }
    }

    /// Set all (`diff.expand_all`, `:539`).
    pub fn expand_all(&mut self) {
        self.expanded.fill(true);
    }

    /// Bit `i`; false when out-of-bounds.
    #[must_use]
    pub fn is_expanded(&self, i: usize) -> bool {
        self.expanded.get(i).copied().unwrap_or(false)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.expanded.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.expanded.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(path: &str) -> FileNode {
        FileNode::new(path, 0)
    }

    fn dir(path: &str, kids: Vec<FileNode>) -> FileNode {
        let mut n = FileNode::new(path, 0);
        for k in kids {
            n.push_child(k);
        }
        n
    }

    #[test]
    fn flatten_depth_first_order() {
        let nodes = vec![dir("src", vec![leaf("src/a.ts"), leaf("src/b.ts")]), leaf("README.md")];
        assert_eq!(TreeFlatten::flatten(&nodes), vec!["src", "src/a.ts", "src/b.ts", "README.md"]);
    }

    #[test]
    fn flatten_skips_collapsed_children() {
        let mut d = dir("src", vec![leaf("src/a.ts")]);
        d.toggle_expand();
        assert_eq!(TreeFlatten::flatten(&[d]), vec!["src"]);
    }

    #[test]
    fn flatten_depth_capped_at_32() {
        let mut node = leaf("d40");
        for i in (0..40).rev() {
            let mut parent = FileNode::new(&format!("d{i}"), 0);
            parent.push_child(node);
            node = parent;
        }
        let rows = TreeFlatten::flatten(&[node]);
        assert_eq!(rows.len(), 33);
        assert_eq!(rows[32], "d32");
        assert!(!rows.contains(&"d40".to_string()));
    }

    #[test]
    fn filter_case_insensitive_substring() {
        let nodes = vec![leaf("src/App.TS"), leaf("docs/readme.md")];
        assert_eq!(TreeFilter::filter(&nodes, "app.ts"), vec!["src/App.TS"]);
        assert_eq!(TreeFilter::filter(&nodes, "SRC"), vec!["src/App.TS"]);
        assert!(TreeFilter::filter(&nodes, "zzz").is_empty());
    }

    #[test]
    fn filter_empty_query_returns_all() {
        let nodes = vec![leaf("a.ts"), leaf("b.ts")];
        assert_eq!(TreeFilter::filter(&nodes, ""), vec!["a.ts", "b.ts"]);
    }

    #[test]
    fn hunk_toggle_flips_and_ignores_oob() {
        let mut h = HunkExpand::new(2);
        assert!(h.is_expanded(0));
        h.toggle(0);
        assert!(!h.is_expanded(0));
        h.toggle(0);
        assert!(h.is_expanded(0));
        h.toggle(99);
        assert_eq!(h.len(), 2);
        assert!(!h.is_expanded(99));
    }

    #[test]
    fn hunk_bounded_at_1024() {
        assert_eq!(HunkExpand::new(2000).len(), MAX_HUNKS);
        assert_eq!(HunkExpand::new(3).len(), 3);
        assert!(HunkExpand::new(0).is_empty());
    }

    #[test]
    fn hunk_expand_collapse_setters() {
        let mut h = HunkExpand::new(1);
        h.collapse(0);
        assert!(!h.is_expanded(0));
        h.expand(0);
        assert!(h.is_expanded(0));
        h.collapse(0);
        h.expand_all();
        assert!(h.is_expanded(0));
    }
}
