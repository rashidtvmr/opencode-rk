#![forbid(unsafe_code)]
//! Unified-diff summary (std-only; no upstream `util/revert-diff.ts`).
//!
//! Spec cited `util/revert-diff.ts` (18 lines); absent in checkout.
//! Nearest equivalent is the custom `*** Begin Patch` parser in
//! `patch/index.ts:185-241`, which is NOT unified diff. This port parses
//! standard unified diffs (`---`/`+++` headers, `@@` hunks) for revert
//! summaries (`session/revert.ts:82-85` additions/deletions/files).
//! Fail-closed: empty/bad input -> empty vec, never panics.

/// Cap on files reported per diff.
pub const MAX_FILES: usize = 1024;
/// Cap on path chars kept.
pub const MAX_PATH_LEN: usize = 512;

/// Per-file `+`/`-` line counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub path: String,
    pub additions: u32,
    pub deletions: u32,
}

fn strip_prefix(p: &str) -> &str {
    p.strip_prefix("a/").or_else(|| p.strip_prefix("b/")).unwrap_or(p)
}

fn header_path(line: &str) -> Option<String> {
    let rest = line.get(4..)?.trim();
    if rest == "/dev/null" {
        return None;
    }
    let end = rest.find('\t').unwrap_or(rest.len());
    let p: String = strip_prefix(&rest[..end]).chars().take(MAX_PATH_LEN).collect();
    if p.is_empty() {
        return None;
    }
    Some(p)
}

/// Parse unified diff into per-file counts. Bad input -> empty, no panic.
#[must_use]
pub fn parse_unified_diff(text: &str) -> Vec<FileDiff> {
    let mut out: Vec<FileDiff> = Vec::new();
    let mut cur: Option<usize> = None;
    let mut in_hunk = false;
    let mut pending_old: Option<String> = None;
    let mut pending_new: Option<String> = None;
    let mut had_old = false;
    for line in text.lines() {
        if line.starts_with("--- ") {
            pending_old = header_path(line);
            had_old = true;
            in_hunk = false;
        } else if line.starts_with("+++ ") {
            pending_new = header_path(line);
            let path = if had_old {
                pending_new.clone().or_else(|| pending_old.clone())
            } else {
                None
            };
            pending_old = None;
            pending_new = None;
            had_old = false;
            in_hunk = false;
            if let Some(path) = path {
                if out.len() >= MAX_FILES {
                    cur = None;
                } else if let Some(i) = out.iter().position(|f| f.path == path) {
                    cur = Some(i);
                } else {
                    out.push(FileDiff {
                        path,
                        additions: 0,
                        deletions: 0,
                    });
                    cur = Some(out.len() - 1);
                }
            } else {
                cur = None;
            }
        } else if line.starts_with("@@") {
            in_hunk = true;
        } else if in_hunk {
            if let Some(i) = cur {
                if let Some(f) = out.get_mut(i) {
                    if line.starts_with('+') {
                        f.additions = f.additions.saturating_add(1);
                    } else if line.starts_with('-') {
                        f.deletions = f.deletions.saturating_add(1);
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "--- a/foo.ts\n+++ b/foo.ts\n@@ -1,2 +1,3 @@\n ctx\n-old\n+new1\n+new2\n";

    #[test]
    fn counts_add_del() {
        let r = parse_unified_diff(DIFF);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].additions, 2);
        assert_eq!(r[0].deletions, 1);
    }

    #[test]
    fn new_file_dev_null() {
        let r = parse_unified_diff("--- /dev/null\n+++ b/new.ts\n@@ -0,0 +1 @@\n+hi\n");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].path, "new.ts");
        assert_eq!(r[0].additions, 1);
    }

    #[test]
    fn strips_ab_prefix() {
        let r = parse_unified_diff(DIFF);
        assert_eq!(r[0].path, "foo.ts");
    }

    #[test]
    fn empty_gives_empty() {
        assert!(parse_unified_diff("").is_empty());
    }

    #[test]
    fn bad_input_no_panic() {
        assert!(parse_unified_diff("garbage\n+++ missing\n@@\n+no headers\n").is_empty());
        assert!(parse_unified_diff("--- \n+++ \n").is_empty());
    }
}
