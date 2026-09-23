# BRG-TREE worklog

## Claim
TASK-ID: BRG-TREE, session: ses_brg_tree. Owned file: crates/opentui-bridge/src/renderable.rs

## Source evidence
- opentui/packages/core/src/Renderable.ts (1893 lines total, read full)
  - BaseRenderable: abstract parent/children, id/num, visible, dirty
  - Renderable extends BaseRenderable
    - parent: Renderable | null
    - _childrenInLayoutOrder: Renderable[]
    - _childrenInZIndexOrder: Renderable[]
    - needsZIndexSort; childrenPrimarySortDirty; childrenSortedByPrimaryAxis
    - LifecyclePass: onLifecyclePass: (() => void) | null
    - add(obj, index?): number -> insertChild semantic
    - insertBefore(obj, anchor): number
    - remove(child): voids; splices both arrays, sets parent=null, unregisterLifecyclePass, bumps revision
    - replaceParent: if child has parent, remove it first (prevent double-parent)
    - childrenPrimarySortDirty on x/y/translate/zIndex/parent changes
    - childrenSortedByPrimaryAxis sorted by screenX/screenY
    - focusable/_focused/_hasFocusedDescendant; focus() propagates, blur() propagates
    - destroy(): destroys children, clears arrays, removes parent link
    - onLifecyclePass registration tied to ctx.registerLifecyclePass
- lib/renderable.validations.ts
  - validateOptions: width/height non-negative
  - isDimensionType/isSizeType/isMarginType etc.
- lib/objects-in-viewport.ts
  - getObjectsInViewport: pre-sorted by screenY/screenX, binary search + zIndex sort result

## Target boundary
Arena-based RenderNode tree in Rust (lib.rs module `renderable`), std-only, forbid(unsafe_code).
Markers: pub struct RenderNode, pub enum LifecyclePass, pub fn insert_child, pub fn remove_child, pub fn render_list, pub fn validate.
Min 150 lines, >=8 tests. Tests compiled/tested via rustc --edition 2021 --test standalone (no crate features, no unsafe).

## Tests written (RED)
1. test_insert_child_appends
2. test_insert_child_at_index
3. test_remove_child_detaches_and_clears_parent
4. test_remove_child_not_present_is_noop
5. test_insert_before_orders_before_anchor
6. test_move_reparent_via_insert
7. test_render_list_depth_first_visible_order
8. test_render_list_skips_invisible
9. test_focus_chain_propagates
10. test_validate_rejects_cycle
11. test_validate_rejects_double_parent
12. test_validate_accepts_tree
13. test_lifecycle_pass_register
14. test_destroy_clears_children

## Decisions
- Arena: Vec<NodeSlot> with index keys; Option<usize> parent/child links. MAX_NODES constant.
- LifecyclePass enum: Update, Render, Layout variants mapping to onLifecyclePass-style hooks.
- insert_child replaces parent on child (mirrors replaceParent).
- insert_before: detach-from-old-parent semantics (anchor must already belong to parent).
- remove_child: head splice + predecessor walk, back-link validation.
- render_list: depth-first pre-order; invisible nodes and their subtrees skipped.
- validate: DFS cycle detection + parent back-link/back-link mismatch counting.
- destroy: recursive depth-first, then recycle into free-list (next_sibling reuse).

## Verification (GREEN)
- Exact success-criteria command:
  `rustc --edition 2021 --test crates/opentui-bridge/src/renderable.rs -o /tmp/opencode/brg_tree_test && /tmp/opencode/brg_tree_test`
  => 14 passed; 0 failed.
- forbid(unsafe_code) asserted; grep shows only the attribute line.
- bridge_lane_gate.py: PASS BRG-TREE 652 lines, 14 tests, all markers present.
- Markers confirmed: pub struct RenderNode (71), pub enum LifecyclePass (15),
  pub fn insert_child (151), pub fn remove_child (236), pub fn render_list (336),
  pub fn validate (367).

## Result
Completed. No commit/push per task instructions.
