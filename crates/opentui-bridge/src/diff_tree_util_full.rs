#![forbid(unsafe_code)]

// TS truth: packages/tui/src/feature-plugins/system/diff-viewer-file-tree-utils.ts (FileTree depth/name rows).
// ponytail: depth cap 8 levels (16sp), label 128 chars; no Unicode width handling, add when tree renders CJK.

/// Two spaces per depth level, capped at 16 chars.
pub fn tree_indent(depth: u32) -> String {
    "  ".repeat(depth.min(8) as usize)
}

/// Trimmed label, capped at 128 chars.
pub fn tree_label(name: &str) -> String {
    name.trim().chars().take(128).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indent_scales() {
        assert_eq!(tree_indent(0), "");
        assert_eq!(tree_indent(2), "    ");
    }
    #[test]
    fn indent_caps_at_16() {
        assert_eq!(tree_indent(99).len(), 16);
    }
    #[test]
    fn label_trims() {
        assert_eq!(tree_label("  hi  "), "hi");
    }
    #[test]
    fn label_caps_at_128() {
        assert_eq!(tree_label(&"a".repeat(200)).len(), 128);
    }
}
