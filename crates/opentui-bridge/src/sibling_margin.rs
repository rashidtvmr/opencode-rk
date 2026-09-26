#![forbid(unsafe_code)]
//! Pre-layout sibling margin cache (mirrors
//! `packages/tui/src/util/layout.ts:1-25`, TS checkout a0d9b6c).
//!
//! TS keeps `previousByParent: WeakMap<parent, { frameID, previous }>` and
//! recomputes the child->previous map once per frame (`el.ctx.frameId`);
//! `setPreLayoutSiblingMargin` then sets `el.marginTop = margin(previous)`.

/// Max cached parent entries (TS WeakMap unbounded; Rust bounded fail-closed).
pub const MAX_PARENTS: usize = 256;
/// Max children tracked per parent entry.
pub const MAX_CHILDREN: usize = 1024;

/// One parent's cached sibling chain for a single frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreLayout {
    /// Opaque parent id (caller maps renderable ptr -> id).
    pub parent: u32,
    /// Frame the `order` snapshot was taken in (mirrors `el.ctx.frameId`).
    pub frame: u64,
    /// Child ids in order; previous sibling of `order[i]` is `order[i-1]`.
    pub order: Vec<u32>,
}

/// Bounded per-frame sibling-margin tracker.
#[derive(Debug, Default, Clone)]
pub struct SiblingMargins {
    entries: Vec<PreLayout>,
}

impl SiblingMargins {
    #[must_use]
    pub const fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Record child order for `parent` in `frame`. Evicts oldest when full.
    /// Truncates `order` to `MAX_CHILDREN`.
    pub fn observe(&mut self, parent: u32, frame: u64, order: &[u32]) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.parent == parent) {
            e.frame = frame;
            e.order.clear();
            e.order.extend(order.iter().copied().take(MAX_CHILDREN));
            return;
        }
        if self.entries.len() >= MAX_PARENTS {
            self.entries.remove(0);
        }
        self.entries.push(PreLayout {
            parent,
            frame,
            order: order.iter().copied().take(MAX_CHILDREN).collect(),
        });
    }

    /// Previous sibling of `id` under `parent` in `frame`, or `None` on
    /// stale frame / unknown parent (mirrors cache-miss -> recompute path).
    /// TS returns `undefined` for first child; same here.
    #[must_use]
    pub fn previous(&self, parent: u32, frame: u64, id: u32) -> Option<u32> {
        let e = self.entries.iter().find(|e| e.parent == parent)?;
        if e.frame != frame {
            return None;
        }
        let pos = e.order.iter().position(|&c| c == id)?;
        if pos == 0 {
            None
        } else {
            Some(e.order[pos - 1])
        }
    }

    /// Margin for `id`: `first` when no previous sibling, `after(prev)` else.
    /// Pure helper around [`Self::previous`] (mirrors `margin(previous.get(el))`).
    #[must_use]
    pub fn margin_before(&self, parent: u32, frame: u64, id: u32, first: u32, after: u32) -> u32 {
        match self.previous(parent, frame, id) {
            None => first,
            Some(_) => after,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_child_gets_first_margin() {
        let mut m = SiblingMargins::new();
        m.observe(1, 7, &[10, 11, 12]);
        assert_eq!(m.previous(1, 7, 10), None);
        assert_eq!(m.margin_before(1, 7, 10, 0, 1), 0);
        assert_eq!(m.margin_before(1, 7, 11, 0, 1), 1);
    }

    #[test]
    fn stale_frame_misses() {
        let mut m = SiblingMargins::new();
        m.observe(1, 7, &[10, 11]);
        assert_eq!(m.previous(1, 8, 11), None);
        assert_eq!(m.margin_before(1, 8, 11, 0, 1), 0);
    }

    #[test]
    fn unknown_parent_or_child_misses() {
        let mut m = SiblingMargins::new();
        m.observe(1, 7, &[10]);
        assert_eq!(m.previous(2, 7, 10), None);
        assert_eq!(m.previous(1, 7, 99), None);
    }

    #[test]
    fn reobserve_refreshes_frame_and_order() {
        let mut m = SiblingMargins::new();
        m.observe(1, 7, &[10, 11]);
        m.observe(1, 8, &[11, 10]);
        assert_eq!(m.previous(1, 8, 11), None);
        assert_eq!(m.previous(1, 8, 10), Some(11));
    }
}
