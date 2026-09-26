#![forbid(unsafe_code)]

//! Keyed `For` list + `Show` conditional over `solid_p0` signals.
//!
//! `ForList` renders one `String` per item and detects input changes by
//! index-wise `PartialEq` comparison (no keyed reconciliation beyond that:
//! no moves, no keyed identity, order-sensitive). `Show` renders its closure
//! only while `when` is true.

/// Keyed-by-index list renderer: `items` plus a per-item `render` closure.
pub struct ForList<T: Clone + PartialEq> {
    items: Vec<T>,
    render: Box<dyn Fn(&T) -> String>,
}

impl<T: Clone + PartialEq> ForList<T> {
    /// Build a list renderer over the initial `items`.
    pub fn new(items: Vec<T>, render: impl Fn(&T) -> String + 'static) -> Self {
        Self {
            items,
            render: Box::new(render),
        }
    }

    /// Render every current item, in order.
    #[must_use]
    pub fn render_all(&self) -> Vec<String> {
        self.items.iter().map(|item| (self.render)(item)).collect()
    }

    /// Replace items; returns `true` iff anything changed (length or any
    /// index-wise `PartialEq` mismatch).
    pub fn update(&mut self, items: Vec<T>) -> bool {
        let changed = self.items.len() != items.len()
            || self.items.iter().zip(items.iter()).any(|(a, b)| a != b);
        if changed {
            self.items = items;
        }
        changed
    }

    /// Current item count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// `true` when the list holds no items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Conditional renderer: `render` runs only while `when` is true.
pub struct Show {
    when: bool,
    render: Box<dyn Fn() -> String>,
}

impl Show {
    /// Build a conditional renderer with the initial condition.
    pub fn new(when: bool, render: impl Fn() -> String + 'static) -> Self {
        Self {
            when,
            render: Box::new(render),
        }
    }

    /// Update the condition.
    pub fn set(&mut self, when: bool) {
        self.when = when;
    }

    /// Current condition.
    #[must_use]
    pub fn when(&self) -> bool {
        self.when
    }

    /// Render when `when` is true, else `None`.
    #[must_use]
    pub fn show(&self) -> Option<String> {
        self.when.then(|| (self.render)())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn for_renders_all_items_in_order() {
        let list = ForList::new(vec![1, 2, 3], |n| format!("item:{n}"));
        assert_eq!(list.render_all(), vec!["item:1", "item:2", "item:3"]);
    }

    #[test]
    fn for_renders_empty_as_empty() {
        let list: ForList<i32> = ForList::new(vec![], |n| format!("{n}"));
        assert!(list.render_all().is_empty());
        assert!(list.is_empty());
    }

    #[test]
    fn for_update_detects_changed_item() {
        let mut list = ForList::new(vec![1, 2], |n| format!("{n}"));
        assert!(list.update(vec![1, 3]));
        assert_eq!(list.render_all(), vec!["1", "3"]);
    }

    #[test]
    fn for_update_detects_length_change() {
        let mut list = ForList::new(vec![1], |n| format!("{n}"));
        assert!(list.update(vec![1, 2]));
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn for_update_returns_false_when_unchanged() {
        let mut list = ForList::new(vec![1, 2], |n| format!("{n}"));
        assert!(!list.update(vec![1, 2]));
        assert_eq!(list.render_all(), vec!["1", "2"]);
    }

    #[test]
    fn show_renders_some_when_true() {
        let show = Show::new(true, || "visible".to_string());
        assert_eq!(show.show(), Some("visible".to_string()));
    }

    #[test]
    fn show_renders_none_when_false() {
        let show = Show::new(false, || "visible".to_string());
        assert_eq!(show.show(), None);
    }

    #[test]
    fn show_follows_set_condition() {
        let mut show = Show::new(false, || "hi".to_string());
        show.set(true);
        assert!(show.when());
        assert_eq!(show.show(), Some("hi".to_string()));
        show.set(false);
        assert_eq!(show.show(), None);
    }
}
