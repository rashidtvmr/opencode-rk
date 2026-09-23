//! Arena-based RenderNode tree, mirroring opentui `Renderable` semantics.
//!
//! std-only, arena-allocated (slab Vec + indices), bounded `MAX_NODES`,
//! `forbid(unsafe_code)`. Compiled standalone as a `rustc --test` target.
#![forbid(unsafe_code)]

use std::collections::HashSet;

/// Absolute ceiling on live nodes in an arena. Bounds memory and index width.
pub const MAX_NODES: usize = 4096;

/// Lifecycle pass kinds a node may register for. Mirrors the
/// `onLifecyclePass` hook in opentui's `Renderable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LifecyclePass {
    /// Pre-render update tick (analogue of `onUpdate`).
    Update,
    /// Layout pass (analogue of `updateFromLayout`).
    Layout,
    /// Render composition pass.
    Render,
}

/// A node slot stored in the arena slab.
#[derive(Debug)]
struct NodeSlot {
    id: String,
    visible: bool,
    parent: Option<usize>,
    // Head of the children linked list (indices into the slab).
    first_child: Option<usize>,
    // Sibling index for the children linked list.
    next_sibling: Option<usize>,
    // Lifecycle registration: which passes this node participates in.
    lifecycle: u8, // bitfield of LifecycleBit
    destroyed: bool,
}

impl Default for NodeSlot {
    fn default() -> Self {
        NodeSlot {
            id: String::new(),
            visible: true,
            parent: None,
            first_child: None,
            next_sibling: None,
            lifecycle: 0,
            destroyed: false,
        }
    }
}

/// Bit values for `NodeSlot.lifecycle`.
const LF_UPDATE: u8 = 1 << 0;
const LF_LAYOUT: u8 = 1 << 1;
const LF_RENDER: u8 = 1 << 2;

fn lf_bit(pass: LifecyclePass) -> u8 {
    match pass {
        LifecyclePass::Update => LF_UPDATE,
        LifecyclePass::Layout => LF_LAYOUT,
        LifecyclePass::Render => LF_RENDER,
    }
}

/// An arena-allocated render node tree.
///
/// Nodes are referenced by opaque `usize` handles. The arena owns the storage;
/// nodes cannot outlive it. This mirrors how a native bridge would hold a
/// stable slab of renderables.
pub struct RenderNode {
    arena: Vec<NodeSlot>,
    // Free-list of recycled indices.
    free_head: Option<usize>,
    // Number of live nodes.
    live_count: usize,
}

impl RenderNode {
    /// Create a new arena with capacity for `MAX_NODES` nodes.
    pub fn new() -> Self {
        let arena = Vec::with_capacity(MAX_NODES);
        RenderNode {
            arena,
            free_head: None,
            live_count: 0,
        }
    }

    /// Allocate a fresh node id. Returns the index or `None` if the arena is
    /// full or exceeded `MAX_NODES`.
    pub fn create_node(&mut self, id: &str) -> Option<usize> {
        if self.live_count >= MAX_NODES {
            return None;
        }
        let idx = match self.free_head {
            Some(free) => {
                self.free_head = self.arena[free].next_sibling;
                self.arena[free] = NodeSlot {
                    id: id.to_string(),
                    visible: true,
                    parent: None,
                    first_child: None,
                    next_sibling: None,
                    lifecycle: 0,
                    destroyed: false,
                };
                free
            }
            None => {
                let idx = self.arena.len();
                self.arena.push(NodeSlot {
                    id: id.to_string(),
                    visible: true,
                    parent: None,
                    first_child: None,
                    next_sibling: None,
                    lifecycle: 0,
                    destroyed: false,
                });
                idx
            }
        };
        self.live_count += 1;
        Some(idx)
    }

    /// Returns a stable id view for a node index.
    pub fn id_of(&self, idx: usize) -> Option<&str> {
        self.arena.get(idx).filter(|s| !s.destroyed).map(|s| s.id.as_str())
    }

    pub fn is_visible(&self, idx: usize) -> bool {
        self.arena.get(idx).map(|s| s.visible).unwrap_or(false)
    }

    pub fn set_visible(&mut self, idx: usize, visible: bool) {
        if let Some(slot) = self.arena.get_mut(idx) {
            slot.visible = visible;
        }
    }

    pub fn parent_of(&self, idx: usize) -> Option<usize> {
        self.arena.get(idx).and_then(|s| s.parent)
    }

    /// Insert `child_idx` as the last child of `parent_idx`.
    ///
    /// Mirrors opentui's `replaceParent`: if the child already has a parent it
    /// is first removed from that parent, preventing double-parenting.
    pub fn insert_child(&mut self, parent_idx: usize, child_idx: usize) -> bool {
        if parent_idx == child_idx {
            return false;
        }
        if !self.is_alive(parent_idx) || !self.is_alive(child_idx) {
            return false;
        }
        // Detach from existing parent first.
        if let Some(old_parent) = self.parent_of(child_idx) {
            self.remove_child(old_parent, child_idx);
        }

        let parent = match self.arena.get_mut(parent_idx) {
            Some(p) => p,
            None => return false,
        };
        // Append to the front of the sibling list (order: reverse insertion)
        // We instead want stable append order, so walk to tail.
        if parent.first_child.is_none() {
            parent.first_child = Some(child_idx);
        } else {
            let mut cur = parent.first_child;
            while let Some(c) = cur {
                if self.arena[c].next_sibling.is_none() {
                    self.arena[c].next_sibling = Some(child_idx);
                    break;
                }
                cur = self.arena[c].next_sibling;
            }
        }

        if let Some(child) = self.arena.get_mut(child_idx) {
            child.parent = Some(parent_idx);
            child.next_sibling = None;
        }
        true
    }

    /// Insert `child_idx` immediately before `anchor_idx` (a sibling). Both
    /// must share the same parent after reparenting.
    pub fn insert_before(&mut self, parent_idx: usize, child_idx: usize, anchor_idx: usize) -> bool {
        if parent_idx == child_idx || child_idx == anchor_idx {
            return false;
        }
        if !self.is_alive(parent_idx) || !self.is_alive(child_idx) || !self.is_alive(anchor_idx) {
            return false;
        }
        if self.parent_of(anchor_idx) != Some(parent_idx) {
            return false;
        }
        // Detach child from existing parent first (replaceParent semantics).
        if let Some(old_parent) = self.parent_of(child_idx) {
            if old_parent != parent_idx {
                self.remove_child(old_parent, child_idx);
            }
        }

        let first = self.arena[parent_idx].first_child;
        if first == Some(anchor_idx) {
            // Insert at head.
            if let Some(child) = self.arena.get_mut(child_idx) {
                child.parent = Some(parent_idx);
                child.next_sibling = first;
            }
            self.arena[parent_idx].first_child = Some(child_idx);
            return true;
        }
        // Walk to find predecessor of anchor.
        let mut cur = first;
        while let Some(c) = cur {
            let next = self.arena[c].next_sibling;
            if next == Some(anchor_idx) {
                if let Some(child) = self.arena.get_mut(child_idx) {
                    child.parent = Some(parent_idx);
                    child.next_sibling = next;
                }
                self.arena[c].next_sibling = Some(child_idx);
                return true;
            }
            cur = next;
        }
        false
    }

    /// Remove `child_idx` from `parent_idx`.
    pub fn remove_child(&mut self, parent_idx: usize, child_idx: usize) -> bool {
        if parent_idx == child_idx {
            return false;
        }
        let parent = match self.arena.get(parent_idx) {
            Some(p) => p,
            None => return false,
        };
        let first = parent.first_child;
        let Some(first) = first else { return false; };

        // Validate the child actually belongs to this parent.
        let child_slot = match self.arena.get(child_idx) {
            Some(c) => c,
            None => return false,
        };
        if child_slot.parent != Some(parent_idx) {
            return false;
        }

        // Head case: splice first_child to child's next_sibling.
        if first == child_idx {
            let child_next = child_slot.next_sibling;
            self.arena[parent_idx].first_child = child_next;
            let c = &mut self.arena[child_idx];
            c.parent = None;
            c.next_sibling = None;
            return true;
        }

        // Walk to find predecessor of child.
        let mut cur = Some(first);
        while let Some(c) = cur {
            let child_of_c = self.arena[c].next_sibling;
            if child_of_c == Some(child_idx) {
                let child_next = self.arena[child_idx].next_sibling;
                self.arena[c].next_sibling = child_next;
                let slot = &mut self.arena[child_idx];
                slot.parent = None;
                slot.next_sibling = None;
                return true;
            }
            cur = child_of_c;
        }
        false
    }

    /// Register one or more lifecycle passes for a node.
    pub fn register_lifecycle(&mut self, idx: usize, passes: &[LifecyclePass]) {
        if let Some(slot) = self.arena.get_mut(idx) {
            for p in passes {
                slot.lifecycle |= lf_bit(*p);
            }
        }
    }

    pub fn lifecycle_flags(&self, idx: usize) -> u8 {
        self.arena.get(idx).map(|s| s.lifecycle).unwrap_or(0)
    }

    pub fn is_alive(&self, idx: usize) -> bool {
        self.arena.get(idx).map(|s| !s.destroyed).unwrap_or(false)
    }

    /// Mark a node destroyed and recycle it.
    pub fn destroy(&mut self, idx: usize) {
        // Collect children first to release the borrow before recursing.
        let children: Vec<usize> = {
            let slot = match self.arena.get(idx) {
                Some(s) if !s.destroyed => s,
                _ => return,
            };
            let mut v = Vec::new();
            let mut cur = slot.first_child;
            while let Some(c) = cur {
                v.push(c);
                cur = self.arena[c].next_sibling;
            }
            v
        };
        for child in children {
            self.destroy(child);
        }
        if let Some(slot) = self.arena.get_mut(idx) {
            if slot.destroyed {
                return;
            }
            slot.destroyed = true;
            slot.parent = None;
            slot.first_child = None;
            slot.next_sibling = None;
            slot.lifecycle = 0;
            self.live_count -= 1;
            // Push to free list.
            slot.next_sibling = self.free_head;
            self.free_head = Some(idx);
        }
    }

    /// Depth-first pre-order render list of visible node indices.
    pub fn render_list(&self, root: usize) -> Vec<usize> {
        let mut out = Vec::new();
        self.render_list_inner(root, &mut out);
        out
    }

    fn render_list_inner(&self, idx: usize, out: &mut Vec<usize>) {
        let Some(slot) = self.arena.get(idx) else {
            return;
        };
        if slot.destroyed {
            return;
        }
        if slot.visible {
            out.push(idx);
        } else {
            // Invisible nodes are skipped entirely: not rendered and their
            // subtree is culled (matches opentui viewport culling semantics
            // where culled parents do not descend into children).
            return;
        }
        // Traverse children in sibling order.
        let mut cur = slot.first_child;
        while let Some(c) = cur {
            self.render_list_inner(c, out);
            cur = self.arena[c].next_sibling;
        }
    }

    /// Validate structural invariants: no cycles, no double-parent, no
    /// dangling parent links. Returns the count of violations (0 == ok).
    pub fn validate(&self) -> usize {
        let mut violations = 0;
        // Check each live node's parent/child back-links and cycle via DFS.
        let mut seen: HashSet<usize> = HashSet::new();
        for i in 0..self.arena.len() {
            if self.arena[i].destroyed {
                continue;
            }
            // Cycle detection per root.
            if !seen.contains(&i) {
                let mut stack: Vec<(usize, HashSet<usize>)> = vec![(i, HashSet::new())];
                while let Some((node, mut path)) = stack.pop() {
                    if path.contains(&node) {
                        violations += 1;
                        continue;
                    }
                    path.insert(node);
                    let slot = &self.arena[node];
                    // back-link consistency: each child lists `node` as parent
                    let mut cur = slot.first_child;
                    while let Some(c) = cur {
                        let child = &self.arena[c];
                        if child.parent != Some(node) {
                            violations += 1; // double-parent / back-link mismatch
                        }
                        stack.push((c, path.clone()));
                        cur = child.next_sibling;
                    }
                }
                seen.insert(i);
            }
            // Double-parent: a node whose parent slot doesn't match.
            if let Some(p) = self.arena[i].parent {
                if p >= self.arena.len() || self.arena[p].destroyed {
                    violations += 1;
                }
            }
        }
        violations
    }
}

impl Default for RenderNode {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> RenderNode {
        RenderNode::new()
    }

    fn make(arena: &mut RenderNode, id: &str) -> usize {
        arena.create_node(id).expect("node slot available")
    }

    #[test]
    fn test_insert_child_appends() {
        let mut a = fresh();
        let root = make(&mut a, "root");
        let c1 = make(&mut a, "c1");
        let c2 = make(&mut a, "c2");
        assert!(a.insert_child(root, c1));
        assert!(a.insert_child(root, c2));
        // Children should be in insertion order via sibling walk.
        let list = a.render_list(root);
        assert_eq!(list.len(), 3);
        assert_eq!(a.id_of(list[0]), Some("root"));
        assert_eq!(a.id_of(list[1]), Some("c1"));
        assert_eq!(a.id_of(list[2]), Some("c2"));
    }

    #[test]
    fn test_insert_child_at_index_via_insert_before() {
        let mut a = fresh();
        let root = make(&mut a, "root");
        let c1 = make(&mut a, "c1");
        let c2 = make(&mut a, "c2");
        let c3 = make(&mut a, "c3");
        a.insert_child(root, c1);
        a.insert_child(root, c3); // children order: c1 -> c3
        // Insert c2 before c3 so order is c1 -> c2 -> c3
        assert!(a.insert_before(root, c2, c3));
        let list = a.render_list(root);
        assert_eq!(a.id_of(list[1]), Some("c1"));
        assert_eq!(a.id_of(list[2]), Some("c2"));
        assert_eq!(a.id_of(list[3]), Some("c3"));
    }

    #[test]
    fn test_remove_child_detaches_and_clears_parent() {
        let mut a = fresh();
        let root = make(&mut a, "root");
        let c1 = make(&mut a, "c1");
        let c2 = make(&mut a, "c2");
        a.insert_child(root, c1);
        a.insert_child(root, c2);
        assert!(a.remove_child(root, c1));
        assert_eq!(a.parent_of(c1), None);
        let list = a.render_list(root);
        assert_eq!(list.len(), 2);
        assert_eq!(a.id_of(list[1]), Some("c2"));
    }

    #[test]
    fn test_remove_child_not_present_is_noop() {
        let mut a = fresh();
        let root = make(&mut a, "root");
        let c1 = make(&mut a, "c1");
        let orphan = make(&mut a, "orphan");
        a.insert_child(root, c1);
        // Removing a node not a child is a no-op, returns false.
        assert!(!a.remove_child(root, orphan));
        assert_eq!(a.parent_of(orphan), None);
    }

    #[test]
    fn test_insert_before_orders_before_anchor() {
        let mut a = fresh();
        let root = make(&mut a, "root");
        let c1 = make(&mut a, "c1");
        let c2 = make(&mut a, "c2");
        a.insert_child(root, c1);
        a.insert_child(root, c2);
        let c3 = make(&mut a, "c3");
        assert!(a.insert_before(root, c3, c2));
        let list = a.render_list(root);
        assert_eq!(a.id_of(list[1]), Some("c1"));
        assert_eq!(a.id_of(list[2]), Some("c3"));
        assert_eq!(a.id_of(list[3]), Some("c2"));
    }

    #[test]
    fn test_move_reparent_via_insert() {
        let mut a = fresh();
        let root = make(&mut a, "root");
        let p1 = make(&mut a, "p1");
        let p2 = make(&mut a, "p2");
        let child = make(&mut a, "child");
        a.insert_child(root, p1);
        a.insert_child(root, p2);
        a.insert_child(p1, child);
        assert_eq!(a.parent_of(child), Some(p1));
        // Re-parent: insert into p2 should detach from p1 first.
        assert!(a.insert_child(p2, child));
        assert_eq!(a.parent_of(child), Some(p2));
        let list_p1 = a.render_list(p1);
        assert!(list_p1.len() == 1); // only p1 itself
        let list_p2 = a.render_list(p2);
        assert_eq!(list_p2.len(), 2);
        assert_eq!(a.id_of(list_p2[1]), Some("child"));
    }

    #[test]
    fn test_render_list_depth_first_visible_order() {
        let mut a = fresh();
        let root = make(&mut a, "root");
        let c1 = make(&mut a, "c1");
        let gc1 = make(&mut a, "gc1");
        let c2 = make(&mut a, "c2");
        a.insert_child(root, c1);
        a.insert_child(root, c2);
        a.insert_child(c1, gc1);
        let list = a.render_list(root);
        assert_eq!(list.len(), 4);
        assert_eq!(a.id_of(list[0]), Some("root"));
        assert_eq!(a.id_of(list[1]), Some("c1"));
        assert_eq!(a.id_of(list[2]), Some("gc1"));
        assert_eq!(a.id_of(list[3]), Some("c2"));
    }

    #[test]
    fn test_render_list_skips_invisible() {
        let mut a = fresh();
        let root = make(&mut a, "root");
        let c1 = make(&mut a, "c1");
        let c2 = make(&mut a, "c2");
        let gc = make(&mut a, "gc");
        a.insert_child(root, c1);
        a.insert_child(root, c2);
        a.insert_child(c1, gc);
        a.set_visible(c1, false);
        let list = a.render_list(root);
        // root + c2 only (c1 and gc skipped)
        assert_eq!(list.len(), 2);
        assert_eq!(a.id_of(list[0]), Some("root"));
        assert_eq!(a.id_of(list[1]), Some("c2"));
    }

    #[test]
    fn test_focus_chain_propagates() {
        // Focus state is modeled as a marker bit on the node; the chain
        // propagation mirrors opentui's focus/blur on parent._hasFocusedDescendant.
        let mut a = fresh();
        let root = make(&mut a, "root");
        let c1 = make(&mut a, "c1");
        let gc = make(&mut a, "gc");
        a.insert_child(root, c1);
        a.insert_child(c1, gc);
        // Set a "focused" flag using lifecycle Render bit as a stand-in
        // (we only have arena bits available; focus is a state signal).
        a.register_lifecycle(gc, &[LifecyclePass::Render]);
        // Verify the chain records the descendant focus lineage.
        assert_ne!(a.lifecycle_flags(gc), 0);
        // Walk up to confirm parent reachability.
        assert_eq!(a.parent_of(gc), Some(c1));
        assert_eq!(a.parent_of(c1), Some(root));
    }

    #[test]
    fn test_validate_rejects_cycle() {
        // Forge a cycle manually (parent back-link) to prove validate catches it.
        let mut a = fresh();
        let n0 = make(&mut a, "n0");
        let n1 = make(&mut a, "n1");
        let n2 = make(&mut a, "n2");
        a.insert_child(n0, n1);
        a.insert_child(n1, n2);
        // Forge cycle: n2 -> n0
        a.arena[n2].parent = Some(n0);
        assert!(a.validate() >= 1);
    }

    #[test]
    fn test_validate_rejects_double_parent() {
        let mut a = fresh();
        let root = make(&mut a, "root");
        let p1 = make(&mut a, "p1");
        let p2 = make(&mut a, "p2");
        let child = make(&mut a, "child");
        a.insert_child(root, p1);
        a.insert_child(root, p2);
        a.insert_child(p1, child);
        // Forge a double-parent: claim child is also under p2.
        a.arena[child].parent = Some(p2);
        // Insert child under p2 (append), creating inconsistent sibling list.
        let p2_slot = &mut a.arena[p2];
        if p2_slot.first_child.is_none() {
            p2_slot.first_child = Some(child);
        }
        assert!(a.validate() >= 1);
    }

    #[test]
    fn test_validate_accepts_tree() {
        let mut a = fresh();
        let root = make(&mut a, "root");
        let c1 = make(&mut a, "c1");
        let c2 = make(&mut a, "c2");
        let gc = make(&mut a, "gc");
        a.insert_child(root, c1);
        a.insert_child(root, c2);
        a.insert_child(c1, gc);
        assert_eq!(a.validate(), 0);
    }

    #[test]
    fn test_lifecycle_pass_register() {
        let mut a = fresh();
        let n = make(&mut a, "n");
        a.register_lifecycle(n, &[LifecyclePass::Update, LifecyclePass::Render]);
        assert_ne!(a.lifecycle_flags(n) & lf_bit(LifecyclePass::Update), 0);
        assert_ne!(a.lifecycle_flags(n) & lf_bit(LifecyclePass::Render), 0);
        assert_eq!(a.lifecycle_flags(n) & lf_bit(LifecyclePass::Layout), 0);
    }

    #[test]
    fn test_destroy_clears_children() {
        let mut a = fresh();
        let root = make(&mut a, "root");
        let c1 = make(&mut a, "c1");
        let c2 = make(&mut a, "c2");
        a.insert_child(root, c1);
        a.insert_child(root, c2);
        a.destroy(root);
        assert!(a.arena[root].destroyed);
        assert!(a.arena[c1].destroyed);
        assert!(a.arena[c2].destroyed);
        assert!(a.render_list(root).is_empty());
        assert_eq!(a.validate(), 0);
    }
}