//! Anchor-merge for session replay rows (read-only merge helper).
//!
//! Mirrors `run/session-replay.ts` `replaySession`/`replayLocalRows`: local
//! rows come first, remote rows are appended, anchor id lines (starting with
//! `#`) are deduped. Output capped at [`REPLAY_CAP`] rows.

/// Maximum rows returned by [`replay_merge`].
pub const REPLAY_CAP: usize = 2000;

/// Merge local and remote replay rows: local first, remote appended.
/// Lines starting with `#` are anchors and emitted once (first wins).
/// Non-anchor lines are kept verbatim (duplicates preserved, order stable).
pub fn replay_merge(local: &[String], remote: &[String]) -> Vec<String> {
    use std::collections::HashSet;
    let mut seen: HashSet<&str> = HashSet::new();
    let mut out = Vec::with_capacity(local.len().saturating_add(remote.len()).min(REPLAY_CAP));
    for row in local.iter().chain(remote.iter()) {
        if out.len() >= REPLAY_CAP {
            break;
        }
        if row.starts_with('#') && !seen.insert(row.as_str()) {
            continue;
        }
        out.push(row.clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(rows: &[&str]) -> Vec<String> {
        rows.iter().map(|r| r.to_string()).collect()
    }

    #[test]
    fn disjoint_concatenates_local_first() {
        assert_eq!(
            replay_merge(&s(&["a", "b"]), &s(&["c"])),
            s(&["a", "b", "c"])
        );
    }

    #[test]
    fn anchor_deduped_remote_dup_skipped() {
        assert_eq!(
            replay_merge(&s(&["#1", "a"]), &s(&["#1", "b"])),
            s(&["#1", "a", "b"])
        );
    }

    #[test]
    fn anchor_dup_within_local_skipped() {
        assert_eq!(
            replay_merge(&s(&["#1", "#1", "x"]), &s(&[])),
            s(&["#1", "x"])
        );
    }

    #[test]
    fn non_anchor_duplicates_preserved() {
        assert_eq!(
            replay_merge(&s(&["a", "a"]), &s(&["a"])),
            s(&["a", "a", "a"])
        );
    }

    #[test]
    fn cap_truncates_at_2000() {
        let local = vec!["l".to_string(); 1500];
        let remote = vec!["r".to_string(); 1500];
        let out = replay_merge(&local, &remote);
        assert_eq!(out.len(), REPLAY_CAP);
        assert!(out.iter().take(1500).all(|r| r == "l"));
        assert!(out.iter().skip(1500).all(|r| r == "r"));
    }

    #[test]
    fn empties_yield_empty() {
        assert!(replay_merge(&[], &[]).is_empty());
        assert_eq!(replay_merge(&s(&["a"]), &s(&[])), s(&["a"]));
        assert_eq!(replay_merge(&s(&[]), &s(&["b"])), s(&["b"]));
    }

    #[test]
    fn order_stable_remote_after_local() {
        assert_eq!(
            replay_merge(&s(&["#a", "x", "#b"]), &s(&["y", "#a", "#c"])),
            s(&["#a", "x", "#b", "y", "#c"])
        );
    }
}
