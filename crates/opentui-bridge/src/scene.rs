#![forbid(unsafe_code)]
//! Retained scene tree over [`CellGrid`] (FFI-02, FFI-11).
//!
//! EVIDENCE / BOUNDARY
//! - The audited boundary (`docs/ui-parity/opencode-ui-parity-kit/
//!   audit-opentui-boundary.md` §2 "Scene, Yoga and measurement", refs [N2],
//!   [N3], [N9]) describes the native contract this module models: a
//!   context-owned retained node tree, native layout, *terminal display cells*
//!   for numeric dimensions and 0..=100 for percentages, geometry with border
//!   insets, clipping intersected down the tree, opacity propagation and
//!   viewport filtering that skips nodes outside the configured viewport.
//! - The vendored `libopentui` exports **no scene entry point** at all
//!   (`nm -gU …/libopentui.dylib` has no `scene*`/`node*` symbol; only
//!   `nativeRenderableAttachYogaNode` / `nativeRenderableSetMeasureTarget`),
//!   so no `extern "C"` is declared here. A scene "renderable" cannot be bound
//!   in this release; it is modelled in Rust over [`CellGrid`] and fails closed
//!   (unknown handles → [`SceneError::Invalid`]).
//! - Yoga's C API (`YGNode*`) is exported by the dylib, but no audited header
//!   at the selected commit records the scene↔Yoga binding or the cell
//!   rounding rules used by `scene.zig`; binding a second layout engine would
//!   be an unaudited re-implementation, not parity. Layout below is therefore a
//!   documented subset: row/column flow, cells/percent/auto (flex) dimensions,
//!   padding + border insets, gap and ordered children.
//! - Ordered moves and state preservation are the FFI-11 half: a moved node
//!   keeps its arena identity (and `tag`), which is what "keyed moves preserve
//!   component state" requires; focus restores to a live focusable node after
//!   the focused one is destroyed.

use crate::buffer::{Cell, CellGrid};
use crate::color::RgbaU16;
use crate::handle::{Arena, GenHandle, ResourceError};
use crate::layout::Rect;
use crate::text;

/// Maximum node nesting depth (fail-closed; mirrors the native tree bound we
/// are willing to walk without recursion guards).
pub const MAX_DEPTH: usize = 32;
/// Maximum children per node.
pub const MAX_CHILDREN: usize = 256;
/// Maximum text bytes per node.
pub const MAX_TEXT_BYTES: usize = 64 * 1024;

/// One axis dimension, in terminal cells or percent of the parent's **content**
/// box (audit §2: numeric dimensions are display cells, percentages are 0-100).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dimension {
    /// Flex: share the remaining main-axis space with the other `Auto`
    /// siblings; on the cross axis it stretches to the parent content box.
    Auto,
    /// Fixed display cells.
    Cells(u32),
    /// Percent of the parent content box (values above 100 are rejected).
    Percent(u8),
}

impl Default for Dimension {
    fn default() -> Self {
        Self::Auto
    }
}

impl Dimension {
    /// Resolve against `basis`. `None` for [`Dimension::Auto`] and for
    /// percentages above 100 (fail closed, never clamps a bad value silently).
    #[must_use]
    pub fn resolve(self, basis: u32) -> Option<u32> {
        match self {
            Self::Auto => None,
            Self::Cells(v) => Some(v.min(basis)),
            Self::Percent(p) if p <= 100 => Some(basis.saturating_mul(p as u32) / 100),
            Self::Percent(_) => None,
        }
    }
}

/// Per-side cell insets (padding or border width).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Edges {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

impl Edges {
    pub const ZERO: Self = Self {
        top: 0,
        right: 0,
        bottom: 0,
        left: 0,
    };

    #[must_use]
    pub const fn all(cells: u32) -> Self {
        Self {
            top: cells,
            right: cells,
            bottom: cells,
            left: cells,
        }
    }

    #[must_use]
    pub const fn horizontal(self) -> u32 {
        self.left.saturating_add(self.right)
    }

    #[must_use]
    pub const fn vertical(self) -> u32 {
        self.top.saturating_add(self.bottom)
    }
}

/// Main-axis direction of a container's children.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    Row,
    Column,
}

/// Node style. Mirrors the native style vocabulary the audit records, restricted
/// to what this model resolves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub width: Dimension,
    pub height: Dimension,
    pub padding: Edges,
    /// Border insets: the border box keeps this much space from its children.
    /// Border glyph painting stays with the caller (`buffer::border_chars`).
    pub border: Edges,
    pub gap: u32,
    pub direction: Direction,
    /// Translation applied to the laid-out box. The audited scene reads computed
    /// geometry and then *applies translations* before clipping and painting
    /// (audit [N3], `scene.zig:1096-1106`), so a node can sit outside the
    /// viewport and be filtered out. Negative offsets saturate at 0.
    pub offset: (i32, i32),
}

impl Default for Style {
    fn default() -> Self {
        Self {
            width: Dimension::Auto,
            height: Dimension::Auto,
            padding: Edges::ZERO,
            border: Edges::ZERO,
            gap: 0,
            direction: Direction::Row,
            offset: (0, 0),
        }
    }
}

impl Style {
    /// Row flow with an explicit cell width/height.
    #[must_use]
    pub const fn sized(width: u32, height: u32) -> Self {
        Self {
            width: Dimension::Cells(width),
            height: Dimension::Cells(height),
            padding: Edges::ZERO,
            border: Edges::ZERO,
            gap: 0,
            direction: Direction::Row,
            offset: (0, 0),
        }
    }

    #[must_use]
    pub const fn with_padding(mut self, padding: Edges) -> Self {
        self.padding = padding;
        self
    }

    #[must_use]
    pub const fn with_border(mut self, border: Edges) -> Self {
        self.border = border;
        self
    }

    #[must_use]
    pub const fn with_gap(mut self, gap: u32) -> Self {
        self.gap = gap;
        self
    }

    #[must_use]
    pub const fn with_offset(mut self, x: i32, y: i32) -> Self {
        self.offset = (x, y);
        self
    }

    #[must_use]
    pub const fn column(mut self) -> Self {
        self.direction = Direction::Column;
        self
    }

    /// Content box of `border_box`: insets are subtracted twice (border, then
    /// padding), saturating, exactly like nested native insets.
    #[must_use]
    pub fn content_rect(self, border_box: Rect) -> Rect {
        let dx = self.border.horizontal().saturating_add(self.padding.horizontal());
        let dy = self.border.vertical().saturating_add(self.padding.vertical());
        Rect::new(
            border_box.x.saturating_add(self.border.left + self.padding.left),
            border_box.y.saturating_add(self.border.top + self.padding.top),
            border_box.w.saturating_sub(dx),
            border_box.h.saturating_sub(dy),
        )
    }
}

/// Intrinsic size in display cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Size {
    pub w: u32,
    pub h: u32,
}

/// Scene failures. Handle failures carry the underlying [`ResourceError`] so a
/// caller can tell a stale node from a stale renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneError {
    /// Handle is null, stale, or belongs to another scene.
    Invalid(ResourceError),
    /// Insertion would exceed [`MAX_DEPTH`].
    DepthExceeded,
    /// Parent already holds [`MAX_CHILDREN`] children.
    TooManyChildren,
    /// The root node cannot be removed or re-parented.
    RootImmutable,
    /// Re-parenting would create a cycle.
    Cycle,
    /// Node is not focusable.
    NotFocusable,
    /// Node text exceeds [`MAX_TEXT_BYTES`].
    TextTooLarge,
}

impl From<ResourceError> for SceneError {
    fn from(err: ResourceError) -> Self {
        Self::Invalid(err)
    }
}

impl core::fmt::Display for SceneError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Invalid(e) => write!(f, "invalid scene node: {e}"),
            Self::DepthExceeded => f.write_str("scene depth exceeded"),
            Self::TooManyChildren => f.write_str("scene node child limit"),
            Self::RootImmutable => f.write_str("scene root is immutable"),
            Self::Cycle => f.write_str("scene re-parent would create a cycle"),
            Self::NotFocusable => f.write_str("node is not focusable"),
            Self::TextTooLarge => f.write_str("node text too large"),
        }
    }
}

impl std::error::Error for SceneError {}

/// One retained scene node.
#[derive(Debug, Clone)]
pub struct Node {
    parent: Option<GenHandle>,
    children: Vec<GenHandle>,
    pub style: Style,
    /// Leaf text content, wrapped into the content box when painted.
    pub text: Option<String>,
    /// `overflow: hidden` equivalent: children are clipped to this node's box.
    pub clip_children: bool,
    /// Per-node opacity 0..=100, multiplied down the tree like native opacity.
    pub opacity_pct: u8,
    /// Participates in focus cycling.
    pub focusable: bool,
    /// Caller payload preserved across ordered moves (FFI-11 keyed moves).
    pub tag: u32,
    pub fg: Option<RgbaU16>,
    pub bg: Option<RgbaU16>,
    computed: Rect,
    visible: bool,
}

impl Node {
    fn new(style: Style) -> Self {
        Self {
            parent: None,
            children: Vec::new(),
            style,
            text: None,
            clip_children: false,
            opacity_pct: 100,
            focusable: false,
            tag: 0,
            fg: None,
            bg: None,
            computed: Rect::new(0, 0, 0, 0),
            visible: true,
        }
    }

    /// Absolute border box from the last [`Scene::layout`].
    #[must_use]
    pub const fn computed(&self) -> Rect {
        self.computed
    }

    /// Viewport-filter result of the last [`Scene::layout`].
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        self.visible
    }

    #[must_use]
    pub fn children(&self) -> &[GenHandle] {
        &self.children
    }

    #[must_use]
    pub const fn parent(&self) -> Option<GenHandle> {
        self.parent
    }
}

/// Retained scene: one arena of nodes rooted at an immutable root.
///
/// Bound to its creating thread by [`Arena`] (matches the native rule that a
/// context is used only from its creating thread).
pub struct Scene {
    nodes: Arena<Node>,
    root: GenHandle,
    viewport: Rect,
    focus: Option<GenHandle>,
}

impl Scene {
    /// Scene whose root border box is `viewport` (`0`-size is allowed; layout
    /// then produces zero-size rects and painting writes nothing).
    #[must_use]
    pub fn new(viewport: Rect) -> Self {
        let mut nodes = Arena::new();
        let mut root_node = Node::new(Style::default());
        root_node.computed = viewport;
        let root = nodes.insert(root_node).expect("empty arena always inserts");
        Self {
            nodes,
            root,
            viewport,
            focus: None,
        }
    }

    #[must_use]
    pub const fn root(&self) -> GenHandle {
        self.root
    }

    #[must_use]
    pub const fn viewport(&self) -> Rect {
        self.viewport
    }

    /// Resize the viewport (equivalent to a native viewport reconfiguration).
    /// Layout must be re-run; computed rects are marked stale by zeroing them.
    pub fn set_viewport(&mut self, viewport: Rect) {
        self.viewport = viewport;
        for handle in self.nodes.handles() {
            if let Ok(node) = self.nodes.get_mut(handle) {
                node.computed = Rect::new(0, 0, 0, 0);
                node.visible = false;
            }
        }
        if let Ok(root) = self.nodes.get_mut(self.root) {
            root.computed = viewport;
            root.visible = true;
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Live node, or [`SceneError::Invalid`] for a stale/null handle.
    pub fn node(&self, handle: GenHandle) -> Result<&Node, SceneError> {
        Ok(self.nodes.get(handle)?)
    }

    /// Append a child to `parent`.
    pub fn insert(&mut self, parent: GenHandle, style: Style) -> Result<GenHandle, SceneError> {
        self.insert_at(parent, style, usize::MAX)
    }

    /// Insert a **new** child at `index` (clamped to the end).
    pub fn insert_at(
        &mut self,
        parent: GenHandle,
        style: Style,
        index: usize,
    ) -> Result<GenHandle, SceneError> {
        self.check_parent(parent)?;
        let handle = self.nodes.insert(Node::new(style))?;
        if let Err(err) = self.attach(parent, handle, index) {
            let _ = self.nodes.remove(handle);
            return Err(err);
        }
        Ok(handle)
    }

    /// Ordered move of an existing node (FFI-02/FFI-11): the node keeps its
    /// handle, style, text and `tag`; only its position changes.
    pub fn move_child(
        &mut self,
        parent: GenHandle,
        child: GenHandle,
        index: usize,
    ) -> Result<(), SceneError> {
        if child == self.root {
            return Err(SceneError::RootImmutable);
        }
        self.check_parent(parent)?;
        // Reject cycles: the new parent must not be the node or its descendant.
        if parent == child || self.is_ancestor(child, parent)? {
            return Err(SceneError::Cycle);
        }
        if let Some(old_parent) = self.nodes.get(child)?.parent {
            self.detach(old_parent, child);
        }
        self.attach(parent, child, index)
    }

    /// Destroy `handle` and every descendant (native recursive destruction).
    /// The node is detached from its parent first; all handles in the subtree
    /// become [`ResourceError::StaleHandle`].
    pub fn remove(&mut self, handle: GenHandle) -> Result<(), SceneError> {
        if handle == self.root {
            return Err(SceneError::RootImmutable);
        }
        if self.focus == Some(handle) {
            self.focus = None;
        }
        let parent = self.nodes.get(handle)?.parent;
        if let Some(parent) = parent {
            self.detach(parent, handle);
        }
        self.destroy_subtree(handle)
    }

    pub fn set_style(&mut self, handle: GenHandle, style: Style) -> Result<(), SceneError> {
        self.nodes.get_mut(handle)?.style = style;
        Ok(())
    }

    pub fn set_text(&mut self, handle: GenHandle, text: Option<&str>) -> Result<(), SceneError> {
        if let Some(text) = text {
            if text.len() > MAX_TEXT_BYTES {
                return Err(SceneError::TextTooLarge);
            }
        }
        self.nodes.get_mut(handle)?.text = text.map(str::to_owned);
        Ok(())
    }

    pub fn set_clip_children(&mut self, handle: GenHandle, clip: bool) -> Result<(), SceneError> {
        self.nodes.get_mut(handle)?.clip_children = clip;
        Ok(())
    }

    pub fn set_opacity(&mut self, handle: GenHandle, pct: u8) -> Result<(), SceneError> {
        self.nodes.get_mut(handle)?.opacity_pct = pct.min(100);
        Ok(())
    }

    pub fn set_focusable(&mut self, handle: GenHandle, focusable: bool) -> Result<(), SceneError> {
        self.nodes.get_mut(handle)?.focusable = focusable;
        Ok(())
    }

    pub fn set_tag(&mut self, handle: GenHandle, tag: u32) -> Result<(), SceneError> {
        self.nodes.get_mut(handle)?.tag = tag;
        Ok(())
    }

    pub fn set_colors(
        &mut self,
        handle: GenHandle,
        fg: Option<RgbaU16>,
        bg: Option<RgbaU16>,
    ) -> Result<(), SceneError> {
        let node = self.nodes.get_mut(handle)?;
        node.fg = fg;
        node.bg = bg;
        Ok(())
    }

    /// Walk parents to the root. Bounded by [`MAX_DEPTH`]+1 so a corrupt parent
    /// chain fails closed instead of looping.
    fn depth(&self, handle: GenHandle) -> Result<usize, SceneError> {
        let mut depth = 0usize;
        let mut current = Some(handle);
        while let Some(node) = current {
            depth += 1;
            if depth > MAX_DEPTH + 1 {
                return Err(SceneError::DepthExceeded);
            }
            current = self.nodes.get(node)?.parent;
        }
        Ok(depth)
    }

    fn is_ancestor(&self, ancestor: GenHandle, node: GenHandle) -> Result<bool, SceneError> {
        let mut current = self.nodes.get(node)?.parent;
        let mut steps = 0usize;
        while let Some(candidate) = current {
            if candidate == ancestor {
                return Ok(true);
            }
            steps += 1;
            if steps > MAX_DEPTH + 1 {
                return Err(SceneError::DepthExceeded);
            }
            current = self.nodes.get(candidate)?.parent;
        }
        Ok(false)
    }

    fn check_parent(&self, parent: GenHandle) -> Result<(), SceneError> {
        self.nodes.get(parent)?;
        if self.depth(parent)? >= MAX_DEPTH {
            return Err(SceneError::DepthExceeded);
        }
        Ok(())
    }

    fn attach(
        &mut self,
        parent: GenHandle,
        child: GenHandle,
        index: usize,
    ) -> Result<(), SceneError> {
        let len = self.nodes.get(parent)?.children.len();
        if len >= MAX_CHILDREN {
            return Err(SceneError::TooManyChildren);
        }
        let index = index.min(len);
        self.nodes.get_mut(parent)?.children.insert(index, child);
        self.nodes.get_mut(child)?.parent = Some(parent);
        Ok(())
    }

    fn detach(&mut self, parent: GenHandle, child: GenHandle) {
        if let Ok(node) = self.nodes.get_mut(parent) {
            node.children.retain(|existing| *existing != child);
        }
        if let Ok(node) = self.nodes.get_mut(child) {
            node.parent = None;
        }
    }

    fn destroy_subtree(&mut self, handle: GenHandle) -> Result<(), SceneError> {
        let children = self.nodes.get(handle)?.children.clone();
        for child in children {
            self.destroy_subtree(child)?;
        }
        self.nodes.remove(handle)?;
        Ok(())
    }

    /// Compute absolute border boxes for the whole tree from the root viewport.
    pub fn layout(&mut self) -> Result<(), SceneError> {
        let viewport = self.viewport;
        self.layout_node(self.root, viewport)
    }

    fn layout_node(
        &mut self,
        handle: GenHandle,
        border_box: Rect,
    ) -> Result<(), SceneError> {
        let children = {
            let node = self.nodes.get_mut(handle)?;
            node.computed = border_box;
            node.visible = handle == self.root || border_box.overlaps(self.viewport);
            node.children.clone()
        };
        if children.is_empty() {
            return Ok(());
        }
        let style = self.nodes.get(handle)?.style;
        let content = style.content_rect(border_box);
        let (main_len, cross_len, horizontal) = match style.direction {
            Direction::Row => (content.w, content.h, true),
            Direction::Column => (content.h, content.w, false),
        };
        let gap_total = style
            .gap
            .saturating_mul(children.len().saturating_sub(1) as u32);
        let main_total = main_len.saturating_sub(gap_total);

        let mut extents = vec![0u32; children.len()];
        let mut used = 0u32;
        let mut flex = 0usize;
        for (i, child) in children.iter().enumerate() {
            let child_style = self.nodes.get(*child)?.style;
            let dim = if horizontal {
                child_style.width
            } else {
                child_style.height
            };
            match dim.resolve(main_total) {
                Some(v) => {
                    extents[i] = v;
                    used = used.saturating_add(v);
                }
                None => flex += 1,
            }
        }
        if flex > 0 {
            let remaining = main_total.saturating_sub(used);
            let share = remaining / flex as u32;
            let mut extra = remaining - share * flex as u32;
            for (i, child) in children.iter().enumerate() {
                let child_style = self.nodes.get(*child)?.style;
                let dim = if horizontal {
                    child_style.width
                } else {
                    child_style.height
                };
                if matches!(dim, Dimension::Auto) {
                    extents[i] = share + if extra > 0 { extra -= 1; 1 } else { 0 };
                }
            }
        }

        let mut cursor = if horizontal { content.x } else { content.y };
        let main_end = cursor.saturating_add(main_len);
        for (i, child) in children.iter().enumerate() {
            let child_style = self.nodes.get(*child)?.style;
            let cross_dim = if horizontal {
                child_style.height
            } else {
                child_style.width
            };
            let cross = cross_dim
                .resolve(cross_len)
                .unwrap_or(cross_len)
                .min(cross_len);
            // Clamp a child that would spill past the content box's main end.
            let extent = extents[i].min(main_end.saturating_sub(cursor));
            let mut rect = if horizontal {
                Rect::new(cursor, content.y, extent, cross)
            } else {
                Rect::new(content.x, cursor, cross, extent)
            };
            rect = translate(rect, child_style.offset);
            self.layout_node(*child, rect)?;
            cursor = cursor.saturating_add(extent).saturating_add(style.gap);
        }
        Ok(())
    }

    /// Intrinsic size of `handle` ignoring parent constraints: text wraps at
    /// `max_width` cells, containers sum their children on the main axis and
    /// take the largest cross extent, plus insets and gaps.
    pub fn measure(&self, handle: GenHandle, max_width: u32) -> Result<Size, SceneError> {
        let node = self.nodes.get(handle)?;
        let inner_width = max_width.saturating_sub(node.style.padding.horizontal());
        let mut size = match &node.text {
            Some(text) => {
                let rows = text::wrap_cells(text, inner_width.max(1) as usize);
                let width = rows
                    .iter()
                    .map(|row| text::display_width(row))
                    .max()
                    .unwrap_or(0) as u32;
                Size {
                    w: width,
                    h: rows.len() as u32,
                }
            }
            None => Size::default(),
        };
        if !node.children.is_empty() {
            let mut main = 0u32;
            let mut cross = 0u32;
            for child in &node.children {
                let child_size = self.measure(*child, inner_width)?;
                match node.style.direction {
                    Direction::Row => {
                        main = main.saturating_add(child_size.w);
                        cross = cross.max(child_size.h);
                    }
                    Direction::Column => {
                        main = main.saturating_add(child_size.h);
                        cross = cross.max(child_size.w);
                    }
                }
            }
            main = main.saturating_add(
                node.style
                    .gap
                    .saturating_mul(node.children.len().saturating_sub(1) as u32),
            );
            size = match node.style.direction {
                Direction::Row => Size {
                    w: main,
                    h: cross,
                },
                Direction::Column => Size {
                    w: cross,
                    h: main,
                },
            };
        }
        Ok(Size {
            w: size
                .w
                .saturating_add(node.style.padding.horizontal())
                .saturating_add(node.style.border.horizontal()),
            h: size
                .h
                .saturating_add(node.style.padding.vertical())
                .saturating_add(node.style.border.vertical()),
        })
    }

    /// Set keyboard focus. Only a live, focusable node can take it.
    pub fn set_focus(&mut self, handle: GenHandle) -> Result<(), SceneError> {
        let node = self.nodes.get(handle)?;
        if !node.focusable {
            return Err(SceneError::NotFocusable);
        }
        self.focus = Some(handle);
        Ok(())
    }

    #[must_use]
    pub const fn focused(&self) -> Option<GenHandle> {
        self.focus
    }

    pub fn clear_focus(&mut self) {
        self.focus = None;
    }

    /// Focusable nodes in depth-first paint order.
    #[must_use]
    pub fn focus_order(&self) -> Vec<GenHandle> {
        let mut out = Vec::new();
        self.collect_focusable(self.root, &mut out);
        out
    }

    fn collect_focusable(&self, handle: GenHandle, out: &mut Vec<GenHandle>) {
        let Ok(node) = self.nodes.get(handle) else {
            return;
        };
        if node.focusable {
            out.push(handle);
        }
        for child in &node.children {
            self.collect_focusable(*child, out);
        }
    }

    /// Focus the next focusable node after the current one, wrapping. Returns
    /// the new focus (or `None` when nothing is focusable). This is also the
    /// restore path after the focused node was destroyed (FFI-11).
    pub fn focus_next(&mut self) -> Option<GenHandle> {
        self.cycle_focus(true)
    }

    /// Focus the previous focusable node, wrapping.
    pub fn focus_prev(&mut self) -> Option<GenHandle> {
        self.cycle_focus(false)
    }

    fn cycle_focus(&mut self, forward: bool) -> Option<GenHandle> {
        let order = self.focus_order();
        if order.is_empty() {
            self.focus = None;
            return None;
        }
        let next = match self
            .focus
            .and_then(|current| order.iter().position(|h| *h == current))
        {
            Some(index) if forward => order[(index + 1) % order.len()],
            Some(index) => order[(index + order.len() - 1) % order.len()],
            None if forward => order[0],
            None => order[order.len() - 1],
        };
        self.focus = Some(next);
        Some(next)
    }

    /// Deepest visible node containing `(x, y)`, searched in reverse paint
    /// order so the topmost node wins. Clipped subtrees are not hit.
    #[must_use]
    pub fn hit_test(&self, x: u32, y: u32) -> Option<GenHandle> {
        self.hit_node(self.root, x, y, self.viewport)
    }

    fn hit_node(
        &self,
        handle: GenHandle,
        x: u32,
        y: u32,
        clip: Rect,
    ) -> Option<GenHandle> {
        let node = self.nodes.get(handle).ok()?;
        if !node.visible && handle != self.root {
            return None;
        }
        let rect = node.computed;
        let inside = x >= rect.x
            && y >= rect.y
            && x < rect.x.saturating_add(rect.w)
            && y < rect.y.saturating_add(rect.h)
            && x >= clip.x
            && y >= clip.y
            && x < clip.x.saturating_add(clip.w)
            && y < clip.y.saturating_add(clip.h);
        if !inside {
            return None;
        }
        let child_clip = if node.clip_children { intersect(clip, rect) } else { clip };
        for child in node.children.iter().rev() {
            if let Some(hit) = self.hit_node(*child, x, y, child_clip) {
                return Some(hit);
            }
        }
        Some(handle)
    }

    /// Paint the tree into `grid`, honouring clip stacking, opacity and the
    /// viewport filter. Returns the number of cells written.
    pub fn paint(&mut self, grid: &mut CellGrid) -> Result<u32, SceneError> {
        self.paint_node(self.root, grid, 100)
    }

    fn paint_node(
        &mut self,
        handle: GenHandle,
        grid: &mut CellGrid,
        parent_opacity: u32,
    ) -> Result<u32, SceneError> {
        let node = self.nodes.get(handle)?;
        let opacity = parent_opacity * node.opacity_pct as u32 / 100;
        if !node.visible || opacity == 0 {
            return Ok(0);
        }
        let rect = node.computed;
        let bg = node.bg;
        let fg = node.fg.unwrap_or(Cell::BLANK.fg);
        let text = node.text.clone();
        let clip = node.clip_children;
        let content = node.style.content_rect(rect);
        let children = node.children.clone();

        let mut written = 0;
        if let Some(bg) = bg {
            written += grid.fill(rect, Cell::new(' ', fg, bg));
        }
        if let Some(text) = text {
            written += paint_text(grid, content, &text, fg, bg);
        }
        if clip {
            grid.push_clip(intersect(grid.clip(), rect));
        }
        for child in children {
            written += self.paint_node(child, grid, opacity)?;
        }
        if clip {
            grid.pop_clip();
        }
        Ok(written)
    }
}

/// Apply a node translation, saturating negative coordinates to 0.
fn translate(rect: Rect, offset: (i32, i32)) -> Rect {
    let x = if offset.0 < 0 {
        rect.x.saturating_sub(offset.0.unsigned_abs())
    } else {
        rect.x.saturating_add(offset.0 as u32)
    };
    let y = if offset.1 < 0 {
        rect.y.saturating_sub(offset.1.unsigned_abs())
    } else {
        rect.y.saturating_add(offset.1 as u32)
    };
    Rect::new(x, y, rect.w, rect.h)
}

fn intersect(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = a
        .x
        .saturating_add(a.w)
        .min(b.x.saturating_add(b.w));
    let y1 = a
        .y
        .saturating_add(a.h)
        .min(b.y.saturating_add(b.h));
    Rect::new(x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0))
}

/// Paint wrapped text inside `content`; wide glyphs occupy their leading cell
/// and leave the trailing cell untouched (native cell layout), clipped by the
/// grid's scissor stack.
fn paint_text(
    grid: &mut CellGrid,
    content: Rect,
    text: &str,
    fg: RgbaU16,
    bg: Option<RgbaU16>,
) -> u32 {
    if content.is_empty() {
        return 0;
    }
    let bg = bg.unwrap_or(Cell::BLANK.bg);
    let mut written = 0;
    for (row, line) in text::wrap_cells(text, content.w as usize)
        .into_iter()
        .take(content.h as usize)
        .enumerate()
    {
        let mut x = content.x;
        for ch in line.chars() {
            if x >= content.x.saturating_add(content.w) {
                break;
            }
            if ch == '\t' {
                let stop = (x + 4) & !3;
                while x < stop {
                    if grid.set(x, content.y + row as u32, Cell::new(' ', fg, bg)) {
                        written += 1;
                    }
                    x += 1;
                }
                continue;
            }
            if grid.set(x, content.y + row as u32, Cell::new(ch, fg, bg)) {
                written += 1;
            }
            x += text::char_cells(ch) as u32;
        }
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{pack_meta, pack_rgba8, INTENT_RGB};

    fn rgb(r: u8, g: u8, b: u8) -> RgbaU16 {
        pack_rgba8(r, g, b, 255, pack_meta(INTENT_RGB, 0))
    }

    fn scene(w: u32, h: u32) -> Scene {
        Scene::new(Rect::new(0, 0, w, h))
    }

    #[test]
    fn ffi02_scene_create_row_layout_in_order() {
        // Root row of 2 fixed children + gap 1 across 20 cells.
        let mut s = scene(20, 4);
        let root = s.root();
        let a = s.insert(root, Style::sized(5, 2)).unwrap();
        let b = s.insert(root, Style::sized(3, 4)).unwrap();
        s.set_style(root, Style::default().with_gap(1)).unwrap();
        s.layout().unwrap();
        assert_eq!(s.node(a).unwrap().computed(), Rect::new(0, 0, 5, 2));
        assert_eq!(s.node(b).unwrap().computed(), Rect::new(6, 0, 3, 4));
        assert_eq!(s.node(root).unwrap().children(), &[a, b]);
    }

    #[test]
    fn ffi02_styles_percent_padding_border_auto_flex() {
        let mut s = scene(20, 10);
        let root = s.root();
        let style = Style {
            direction: Direction::Column,
            ..Style::default()
                .with_padding(Edges::all(1))
                .with_border(Edges::all(1))
                .with_gap(1)
        };
        s.set_style(root, style).unwrap();
        let half = s
            .insert(
                root,
                Style {
                    height: Dimension::Cells(2),
                    width: Dimension::Percent(50),
                    ..Style::default()
                },
            )
            .unwrap();
        let flex = s.insert(root, Style::default()).unwrap();
        s.layout().unwrap();
        // Border then padding insets: 20-4 x 10-4 of content.
        let content = Rect::new(2, 2, 16, 6);
        assert_eq!(style.content_rect(Rect::new(0, 0, 20, 10)), content);
        // Percent resolves against the content box cross axis (16 -> 8).
        assert_eq!(s.node(half).unwrap().computed(), Rect::new(2, 2, 8, 2));
        // Auto consumes what the fixed child left on the main axis, minus gap.
        assert_eq!(s.node(flex).unwrap().computed(), Rect::new(2, 5, 16, 3));
    }

    #[test]
    fn ffi02_auto_shares_leftover_equally() {
        let mut s = scene(10, 3);
        let root = s.root();
        let a = s.insert(root, Style::default()).unwrap();
        let b = s.insert(root, Style::default()).unwrap();
        let c = s.insert(root, Style::default()).unwrap();
        s.layout().unwrap();
        // 10 cells over 3 equal flex children: 4,3,3 (leftover to leading).
        let widths: Vec<u32> = [a, b, c]
            .iter()
            .map(|h| s.node(*h).unwrap().computed().w)
            .collect();
        assert_eq!(widths, vec![4, 3, 3]);
        assert_eq!(s.node(b).unwrap().computed().x, 4);
        assert_eq!(s.node(c).unwrap().computed().x, 7);
    }

    #[test]
    fn ffi02_ordered_move_preserves_identity_and_tag() {
        let mut s = scene(12, 2);
        let root = s.root();
        let a = s.insert(root, Style::sized(4, 1)).unwrap();
        let b = s.insert(root, Style::sized(4, 1)).unwrap();
        s.set_tag(a, 0xAAAA).unwrap();
        s.set_text(a, Some("A")).unwrap();
        s.layout().unwrap();
        assert_eq!(s.node(a).unwrap().computed().x, 0);

        // Move the second child to the front: same handles, new order.
        s.move_child(root, b, 0).unwrap();
        s.layout().unwrap();
        assert_eq!(s.node(root).unwrap().children(), &[b, a]);
        assert_eq!(s.node(a).unwrap().tag, 0xAAAA, "tag survives the move");
        assert_eq!(s.node(a).unwrap().text.as_deref(), Some("A"));
        assert_eq!(s.node(a).unwrap().computed().x, 4, "geometry follows order");
        assert_eq!(s.node(b).unwrap().computed().x, 0);
    }

    #[test]
    fn ffi02_destroy_subtree_stales_handles() {
        let mut s = scene(20, 5);
        let root = s.root();
        let parent = s.insert(root, Style::default()).unwrap();
        let child = s.insert(parent, Style::default()).unwrap();
        let grandchild = s.insert(child, Style::default()).unwrap();
        assert_eq!(s.len(), 4);
        s.remove(parent).unwrap();
        assert_eq!(s.len(), 1, "root only");
        assert_eq!(s.node(root).unwrap().children(), &[] as &[GenHandle]);
        for stale in [parent, child, grandchild] {
            assert_eq!(
                s.node(stale).unwrap_err(),
                SceneError::Invalid(ResourceError::StaleHandle)
            );
        }
        assert_eq!(s.remove(root), Err(SceneError::RootImmutable));
    }

    #[test]
    fn scene_rejects_cycles_depth_and_overflow() {
        let mut s = scene(10, 10);
        let root = s.root();
        let parent = s.insert(root, Style::default()).unwrap();
        let child = s.insert(parent, Style::default()).unwrap();
        // Re-parenting an ancestor under its own descendant is a cycle.
        assert_eq!(
            s.move_child(child, parent, 0),
            Err(SceneError::Cycle)
        );
        assert_eq!(s.move_child(child, root, 0), Err(SceneError::RootImmutable));
        // Depth bound: MAX_DEPTH levels below root is the limit.
        let mut cursor = root;
        for _ in 0..MAX_DEPTH - 1 {
            cursor = s.insert(cursor, Style::default()).unwrap();
        }
        assert_eq!(
            s.insert(cursor, Style::default()),
            Err(SceneError::DepthExceeded)
        );
    }

    #[test]
    fn ffi02_measure_wrapped_text_in_cells() {
        let mut s = scene(10, 4);
        let node = s.insert(s.root(), Style::default()).unwrap();
        // 5 display cells of CJK wrap into 3 rows at width 4, 2 cells each.
        s.set_text(node, Some("中中中中中")).unwrap();
        assert_eq!(s.measure(node, 4).unwrap(), Size { w: 4, h: 3 });
        // Padding is added around the measured content.
        s.set_style(node, Style::default().with_padding(Edges::all(1)))
            .unwrap();
        assert_eq!(s.measure(node, 6).unwrap(), Size { w: 6, h: 5 });
        assert_eq!(
            s.set_text(node, Some(&"x".repeat(MAX_TEXT_BYTES + 1))),
            Err(SceneError::TextTooLarge)
        );
    }

    #[test]
    fn ffi02_clip_children_and_viewport_culling() {
        let mut s = scene(10, 4);
        let root = s.root();
        let clipper = s.insert(root, Style::sized(4, 2)).unwrap();
        s.set_clip_children(clipper, true).unwrap();
        let wide = s.insert(clipper, Style::sized(20, 2)).unwrap();
        s.set_colors(wide, None, Some(rgb(9, 9, 9))).unwrap();
        // A translated node lands below the viewport: laid out, not painted and
        // not hit-testable (native viewport filter, audit [N3]).
        let culled = s
            .insert(root, Style::sized(2, 2).with_offset(0, 10))
            .unwrap();
        s.set_colors(culled, None, Some(rgb(7, 7, 7))).unwrap();
        s.layout().unwrap();
        assert_eq!(s.node(culled).unwrap().computed(), Rect::new(4, 10, 2, 2));
        assert!(!s.node(culled).unwrap().is_visible(), "outside viewport");
        assert!(s.node(clipper).unwrap().is_visible());

        let mut grid = CellGrid::new(10, 4);
        let written = s.paint(&mut grid).unwrap();
        // Only the clipped 4x2 box is painted, not the 20-column child.
        assert_eq!(written, 8);
        assert_eq!(grid.get(3, 1).unwrap().bg, rgb(9, 9, 9));
        assert_eq!(grid.get(4, 0).unwrap(), Cell::BLANK);
        assert_eq!(s.hit_test(1, 10), None);
    }

    #[test]
    fn ffi11_focus_cycle_and_restore_after_destroy() {
        let mut s = scene(10, 3);
        let root = s.root();
        let a = s.insert(root, Style::default()).unwrap();
        let b = s.insert(root, Style::default()).unwrap();
        let c = s.insert(root, Style::default()).unwrap();
        s.set_focusable(b, true).unwrap();
        s.set_focusable(c, true).unwrap();
        assert_eq!(s.set_focus(a), Err(SceneError::NotFocusable));
        assert_eq!(s.focus_order(), vec![b, c]);
        assert_eq!(s.focus_next(), Some(b));
        assert_eq!(s.focused(), Some(b));
        assert_eq!(s.focus_next(), Some(c));
        assert_eq!(s.focus_next(), Some(b), "wraps");
        assert_eq!(s.focus_prev(), Some(c), "wraps backwards");

        // Destroying the focused node clears focus and restore lands on the
        // next live focusable node.
        s.remove(c).unwrap();
        assert_eq!(s.focused(), None);
        assert_eq!(s.focus_next(), Some(b));
        s.remove(b).unwrap();
        assert_eq!(s.focus_next(), None);
        assert!(s.focus_order().is_empty());
    }

    #[test]
    fn ffi11_hit_test_prefers_deepest_child() {
        let mut s = scene(10, 4);
        let root = s.root();
        let outer = s
            .insert(root, Style::sized(6, 3).with_padding(Edges::all(1)))
            .unwrap();
        let inner = s.insert(outer, Style::sized(2, 1)).unwrap();
        s.layout().unwrap();
        // Inner sits at the outer content origin (1, 1).
        assert_eq!(s.node(inner).unwrap().computed(), Rect::new(1, 1, 2, 1));
        assert_eq!(s.hit_test(1, 1), Some(inner));
        assert_eq!(s.hit_test(5, 2), Some(outer));
        assert_eq!(s.hit_test(9, 3), Some(root));
        assert_eq!(s.hit_test(20, 0), None);
    }

    #[test]
    fn scene_paint_respects_opacity() {
        let mut s = scene(6, 2);
        let node = s.insert(s.root(), Style::sized(2, 1)).unwrap();
        s.set_colors(node, Some(rgb(1, 1, 1)), Some(rgb(2, 2, 2)))
            .unwrap();
        s.set_opacity(node, 0).unwrap();
        s.layout().unwrap();
        let mut grid = CellGrid::new(6, 2);
        assert_eq!(s.paint(&mut grid).unwrap(), 0);
        assert_eq!(grid, CellGrid::new(6, 2));
    }
}
