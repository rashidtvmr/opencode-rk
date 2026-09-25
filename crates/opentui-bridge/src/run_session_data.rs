#![forbid(unsafe_code)]

//! Session data rows for run view.
//! Mirrors `packages/opencode/src/cli/cmd/run/session-data.ts`
//! snapshot pick (`fetch_snapshot` lexicographic `updated_at` max).

/// Single session row. `id` and `updated_at` capped at 64 chars.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionData {
    pub id: String,
    pub updated_at: String,
    pub message_count: u32,
}

fn cap64(s: &str) -> String {
    if s.chars().count() <= 64 {
        s.to_owned()
    } else {
        s.chars().take(64).collect()
    }
}

impl SessionData {
    pub fn new(id: &str, updated_at: &str, message_count: u32) -> Self {
        Self {
            id: cap64(id),
            updated_at: cap64(updated_at),
            message_count,
        }
    }
}

/// Latest row by lexicographic `updated_at` max. `None` when empty.
pub fn pick_latest(items: &[SessionData]) -> Option<&SessionData> {
    items.iter().max_by(|a, b| a.updated_at.cmp(&b.updated_at))
}

/// Saturating sum of `message_count`.
pub fn count_total(items: &[SessionData]) -> u32 {
    items
        .iter()
        .fold(0u32, |acc, s| acc.saturating_add(s.message_count))
}

/// Preview helper: passthrough when <=80 chars, else 80 chars + `...`.
pub fn preview_title(title: &str) -> String {
    const MAX: usize = 80;
    if title.chars().count() <= MAX {
        title.to_owned()
    } else {
        let head: String = title.chars().take(MAX).collect();
        head + "..."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, updated_at: &str, n: u32) -> SessionData {
        SessionData::new(id, updated_at, n)
    }

    #[test]
    fn latest_picks_max() {
        let items = vec![
            row("a", "2024-01-01", 1),
            row("b", "2024-06-01", 2),
            row("c", "2024-03-01", 3),
        ];
        assert_eq!(pick_latest(&items).unwrap().id, "b");
    }

    #[test]
    fn empty_none() {
        let items: Vec<SessionData> = vec![];
        assert!(pick_latest(&items).is_none());
    }

    #[test]
    fn saturating_total() {
        let items = vec![row("a", "2024-01-01", u32::MAX), row("b", "2024-01-02", 1)];
        assert_eq!(count_total(&items), u32::MAX);
    }

    #[test]
    fn preview_truncates() {
        let long = "x".repeat(100);
        let out = preview_title(&long);
        assert_eq!(out.len(), 83);
        assert!(out.ends_with("..."));
    }

    #[test]
    fn preview_short_passthrough() {
        assert_eq!(preview_title("hello"), "hello");
    }

    #[test]
    fn id_caps_at_64() {
        let long = "y".repeat(100);
        let s = row(&long, "2024-01-01", 1);
        assert_eq!(s.id.chars().count(), 64);
    }
}
