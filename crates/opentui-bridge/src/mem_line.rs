#![forbid(unsafe_code)]
//! Memory transcript seed line: `memory: N file(s) loaded`.
//!
//! TS truth (`crates/cli/src/tui_entry.rs:638`): transcript pushes
//! `format!("memory: {} file(s) loaded", memory.len())` only when non-empty.
//!
//! `ponytail:` no path/byte detail; add when caller needs it.

/// Seed line for `count` memory files; `None` when 0. Capped at 128 chars.
#[must_use]
pub fn mem_seed(count: usize) -> Option<String> {
    if count == 0 {
        return None;
    }
    let s = format!("memory: {count} file(s) loaded");
    Some(s.chars().take(128).collect())
}

/// Transcript lines for `count` memory files: empty or one line.
#[must_use]
pub fn mem_lines(count: usize) -> Vec<String> {
    mem_seed(count).into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_when_zero() {
        assert_eq!(mem_seed(0), None);
    }

    #[test]
    fn empty_vec_when_zero() {
        assert!(mem_lines(0).is_empty());
    }

    #[test]
    fn seed_matches_tui_truth() {
        assert_eq!(mem_seed(2), Some("memory: 2 file(s) loaded".to_string()));
    }

    #[test]
    fn lines_single_entry() {
        assert_eq!(mem_lines(1), vec!["memory: 1 file(s) loaded".to_string()]);
    }

    #[test]
    fn seed_capped_at_128() {
        let s = mem_seed(usize::MAX).unwrap();
        assert!(s.chars().count() <= 128);
        assert!(s.starts_with("memory: "));
    }
}
