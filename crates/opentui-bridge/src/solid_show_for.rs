//! SolidJS Show/For control flow model (no local truth).
#![forbid(unsafe_code)]

/// Max chars kept per picked branch / mapped item.
pub const MAX_CHARS: usize = 4096;
/// Max items mapped by [`for_each`].
pub const MAX_ITEMS: usize = 64;

fn truncate(s: &str) -> String {
    if s.len() <= MAX_CHARS {
        return s.to_owned();
    }
    s.chars().take(MAX_CHARS).collect()
}

/// Model of `<Show when cond fallback>then</Show>`: pick branch, cap 4KiB chars.
pub fn show(cond: bool, then: &str, otherwise: &str) -> String {
    truncate(if cond { then } else { otherwise })
}

/// Model of `<For each=items>{f}</For>`: map up to 64 items, cap each 4KiB chars.
pub fn for_each(items: &[String], f: &dyn Fn(&str) -> String) -> Vec<String> {
    items
        .iter()
        .take(MAX_ITEMS)
        .map(|s| truncate(&f(s)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_true_picks_then() {
        assert_eq!(show(true, "a", "b"), "a");
    }

    #[test]
    fn show_false_picks_otherwise() {
        assert_eq!(show(false, "a", "b"), "b");
    }

    #[test]
    fn show_caps_branch() {
        let big = "x".repeat(MAX_CHARS + 10);
        assert_eq!(show(true, &big, "").chars().count(), MAX_CHARS);
    }

    #[test]
    fn for_each_maps() {
        let items = vec!["a".to_owned(), "b".to_owned()];
        assert_eq!(for_each(&items, &|s| s.to_uppercase()), vec!["A", "B"]);
    }

    #[test]
    fn for_each_caps_items_and_chars() {
        let items: Vec<String> = (0..100).map(|i| i.to_string()).collect();
        let out = for_each(&items, &|s| "y".repeat(MAX_CHARS + 1) + s);
        assert_eq!(out.len(), MAX_ITEMS);
        assert!(out.iter().all(|s| s.chars().count() == MAX_CHARS));
    }
}
