#![forbid(unsafe_code)]
//! Bounded keyboard-navigable nodes/edges graph view model (TUI-GRAPH).
//!
//! Pure state only: view model for a directed graph of session/subagent nodes
//! with edges, cursor navigation by adjacency, bounded capacity with eviction
//! of non-focused nodes, ASCII/box-drawing render to a bounded frame, and
//! JSON serialization for the native shell.
//!
//! No rendering, no IO, no clock, no threads.
//!
//! [`GraphView::from_family`] projects the composer's session family
//! (`packages/tui/src/routes/session/composer/subagents-tab.tsx:41-65` via
//! `util/session.ts:9-45`) into positioned nodes plus one parent edge per
//! child. This file is also compiled standalone by
//! `crates/cli/tests/native_graph.rs`, so it must stay dependency-free: the
//! tree layout below is a local extension (an explicit x per depth, one row
//! per family member) and is not an upstream claim.
//!
//! v2 (wave-6 reachability): [`GraphPanel`] is the page-visible surface —
//! label gutter, two-row depth axis, one series row per node, legend, the
//! horizontal drag offset and the explicit empty/unsupported/syntax/oversize
//! notes. It is consumed by `crate::native_view_pages::Page::Graph` through
//! `PageState::open_graph`, and [`DiagramKind::detect`] mirrors the pin's
//! Mermaid detection set. The chart layout constants and their upstream
//! `file:line` evidence sit above [`DiagramKind`].

use serde::{Deserialize, Serialize};

/// Upper bound on retained nodes.
pub const MAX_GRAPH_NODES: usize = 256;
/// Upper bound on retained edges.
pub const MAX_GRAPH_EDGES: usize = 512;
/// Max pan coordinate (both axes).
pub const MAX_PAN: i32 = 1000;
/// Horizontal distance between two family depths in [`GraphView::from_family`].
pub const FAMILY_COLUMN_WIDTH: i32 = 24;
/// Vertical distance between two family rows in [`GraphView::from_family`].
pub const FAMILY_ROW_HEIGHT: i32 = 4;
/// Fixed node height used by the family projection.
pub const FAMILY_NODE_HEIGHT: u32 = 3;

/// Classification of a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    Session,
    Subagent,
}

/// Lifecycle status of a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Idle,
    Running,
    Done,
    Error,
}

/// One node in the graph.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeView {
    pub id: String,
    pub kind: NodeKind,
    pub label: String,
    pub status: NodeStatus,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Visual style of an edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeStyle {
    Solid,
    Dashed,
}

/// One edge in the graph.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeView {
    pub from: String,
    pub to: String,
    pub style: EdgeStyle,
}

/// Direction for cursor navigation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavDir {
    Up,
    Down,
    Left,
    Right,
}

/// One session in the composer's family projection. `parent` points at the
/// daemon-supplied parent session id; the projection never invents parents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilyNode {
    pub id: String,
    pub parent: Option<String>,
    pub label: String,
    pub status: NodeStatus,
}

impl FamilyNode {
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>, status: NodeStatus) -> Self {
        Self {
            id: id.into(),
            parent: None,
            label: label.into(),
            status,
        }
    }

    #[must_use]
    pub fn with_parent(mut self, parent: impl Into<String>) -> Self {
        self.parent = Some(parent.into());
        self
    }
}

/// A bounded render frame: `rows` x `cols` grid of characters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub rows: usize,
    pub cols: usize,
    pub grid: Vec<Vec<char>>,
}

/// The graph view model.
#[derive(Clone, Debug)]
pub struct GraphView {
    nodes: Vec<NodeView>,
    edges: Vec<EdgeView>,
    focus_idx: Option<usize>,
    pan_x: i32,
    pan_y: i32,
    frame_w: usize,
    frame_h: usize,
    capacity: usize,
}

/// Serializable snapshot for JSON round-trip.
#[derive(Serialize, Deserialize)]
struct GraphSnapshot {
    nodes: Vec<NodeView>,
    edges: Vec<EdgeView>,
    focus_idx: Option<usize>,
    pan_x: i32,
    pan_y: i32,
    frame_w: usize,
    frame_h: usize,
    capacity: usize,
}

impl GraphView {
    /// Create a graph with default capacity (`MAX_GRAPH_NODES`).
    #[must_use]
    pub fn new(nodes: Vec<NodeView>, edges: Vec<EdgeView>, frame_w: usize, frame_h: usize) -> Self {
        Self::with_capacity(nodes, edges, frame_w, frame_h, MAX_GRAPH_NODES)
    }

    /// Create a graph with an explicit node capacity for testing eviction.
    #[must_use]
    pub fn with_capacity(
        nodes: Vec<NodeView>,
        edges: Vec<EdgeView>,
        frame_w: usize,
        frame_h: usize,
        capacity: usize,
    ) -> Self {
        let cap = capacity.min(MAX_GRAPH_NODES);
        let mut g = Self {
            nodes: Vec::with_capacity(cap),
            edges,
            focus_idx: None,
            pan_x: 0,
            pan_y: 0,
            frame_w,
            frame_h,
            capacity: cap,
        };
        for n in nodes {
            g.insert_node_inner(n);
        }
        if !g.nodes.is_empty() {
            g.focus_idx = Some(0);
        }
        g
    }

    /// Number of visible nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Id of the focused node, if any.
    #[must_use]
    pub fn focused_id(&self) -> Option<&str> {
        self.focus_idx
            .and_then(|i| self.nodes.get(i).map(|n| n.id.as_str()))
    }

    /// Frame width.
    #[must_use]
    pub fn width(&self) -> usize {
        self.frame_w
    }

    /// Frame height.
    #[must_use]
    pub fn height(&self) -> usize {
        self.frame_h
    }

    /// Current pan offset (x, y).
    #[must_use]
    pub fn pan(&self) -> (i32, i32) {
        (self.pan_x, self.pan_y)
    }

    /// Set pan offset (clamped to 0..MAX_PAN on each axis).
    pub fn set_pan(&mut self, x: i32, y: i32) {
        self.pan_x = x.max(0).min(MAX_PAN);
        self.pan_y = y.max(0).min(MAX_PAN);
    }

    /// Navigate cursor in the given direction. Finds the nearest neighbor by
    /// adjacency (closest node in that direction), wrapping cyclically.
    pub fn navigate(&mut self, dir: NavDir) {
        let focus = match self.focus_idx {
            Some(i) => i,
            None => return,
        };
        let fx = self.nodes[focus].x;
        let fy = self.nodes[focus].y;
        if self.nodes.len() <= 1 {
            return;
        }

        let best = match dir {
            NavDir::Right => self.find_nearest(focus, fx, fy, true, true),
            NavDir::Left => self.find_nearest(focus, fx, fy, true, false),
            NavDir::Down => self.find_nearest(focus, fx, fy, false, true),
            NavDir::Up => self.find_nearest(focus, fx, fy, false, false),
        };

        if let Some(idx) = best {
            self.focus_idx = Some(idx);
        }
    }

    /// Find nearest node in a direction, wrapping if none found.
    /// `horizontal=true` means x-axis (left/right), `forward=true` means positive direction.
    fn find_nearest(
        &self,
        focus: usize,
        fx: i32,
        fy: i32,
        horizontal: bool,
        forward: bool,
    ) -> Option<usize> {
        let mut best: Option<(i32, usize)> = None;
        for (i, node) in self.nodes.iter().enumerate() {
            if i == focus {
                continue;
            }
            let (pos, ref_pos) = if horizontal {
                (node.x, fx)
            } else {
                (node.y, fy)
            };
            let dominated = if forward {
                pos > ref_pos
            } else {
                pos < ref_pos
            };
            if dominated {
                let dist = (pos - ref_pos).abs();
                match &best {
                    None => best = Some((dist, i)),
                    Some((bd, _)) if dist < *bd => best = Some((dist, i)),
                    _ => {}
                }
            }
        }
        best.map(|(_, i)| i).or_else(|| {
            // Wrap: pick farthest in the opposite direction (end of list)
            self.nodes
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != focus)
                .min_by_key(|(_, n)| {
                    let pos = if horizontal { n.x } else { n.y };
                    if forward {
                        pos
                    } else {
                        -pos
                    }
                })
                .map(|(i, _)| i)
        })
    }

    /// Insert a node. When the list is full, the non-focused node farthest
    /// from the current focus is evicted.
    pub fn insert_node(&mut self, node: NodeView) {
        self.insert_node_inner(node);
        if self.nodes.len() == 1 {
            self.focus_idx = Some(0);
        }
    }

    fn insert_node_inner(&mut self, node: NodeView) {
        if self.nodes.len() >= self.capacity {
            self.evict_one();
        }
        self.nodes.push(node);
    }

    /// Remove a node by id. Focus shifts to the next node or wraps.
    pub fn remove_node(&mut self, id: &str) {
        if let Some(idx) = self.nodes.iter().position(|n| n.id == id) {
            self.nodes.remove(idx);
            match self.focus_idx {
                Some(f) if f == idx => {
                    if self.nodes.is_empty() {
                        self.focus_idx = None;
                    } else {
                        self.focus_idx = Some(f.min(self.nodes.len() - 1));
                    }
                }
                Some(f) if f > idx => {
                    self.focus_idx = Some(f - 1);
                }
                _ => {}
            }
        }
        self.edges.retain(|e| e.from != id && e.to != id);
    }

    /// Evict the non-focused node farthest from the current focus.
    fn evict_one(&mut self) {
        if self.nodes.is_empty() {
            return;
        }
        let focus = self.focus_idx.unwrap_or(0);
        let fx = self.nodes[focus].x;
        let fy = self.nodes[focus].y;
        let farthest = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != focus)
            .max_by_key(|(_, n)| (n.x - fx).abs() + (n.y - fy).abs())
            .map(|(i, _)| i);
        if let Some(idx) = farthest {
            let removed_id = self.nodes[idx].id.clone();
            self.nodes.remove(idx);
            self.edges
                .retain(|e| e.from != removed_id && e.to != removed_id);
            if let Some(ref mut f) = self.focus_idx {
                if *f > idx {
                    *f -= 1;
                }
            }
        }
    }

    /// Render the graph into a bounded frame of box-drawing characters.
    #[must_use]
    pub fn render(&self) -> Frame {
        self.render_sized(self.frame_w, self.frame_h)
    }

    /// Render into an explicitly sized frame without mutating the stored
    /// frame size. Additive: the page renderer asks for the live content box
    /// (the view itself keeps its construction-time size, so pan/focus state
    /// survives a resize).
    #[must_use]
    pub fn render_sized(&self, width: usize, height: usize) -> Frame {
        let w = width;
        let h = height;
        if w == 0 || h == 0 {
            return Frame {
                rows: h,
                cols: w,
                grid: vec![vec![' '; w]; h],
            };
        }
        let mut grid = vec![vec![' '; w]; h];

        // Draw nodes as box-drawing rectangles
        for (idx, node) in self.nodes.iter().enumerate() {
            let screen_x = (node.x - self.pan_x) as usize;
            let screen_y = (node.y - self.pan_y) as usize;
            let nw = node.width as usize;
            let nh = node.height as usize;

            // Skip nodes entirely outside the frame
            if screen_x + nw <= 1 || screen_x >= w || screen_y + nh <= 1 || screen_y >= h {
                continue;
            }

            // Clamp to frame bounds
            let _x0 = screen_x.max(1);
            let y0 = screen_y.max(1);
            let x1 = (screen_x + nw).min(w);
            let y1 = (screen_y + nh).min(h);

            // Top border
            if screen_y < h && screen_x < w {
                if screen_x > 0 {
                    let col = (screen_x - 1).min(w - 1);
                    if screen_y < h {
                        grid[screen_y][col] = '+';
                    }
                }
                for c in screen_x..x1.min(w) {
                    grid[screen_y.min(h - 1)][c] = '-';
                }
                if x1 < w {
                    grid[screen_y.min(h - 1)][x1] = '+';
                }
            }

            // Side borders and label
            let label = &node.label;
            let label_chars: Vec<char> = label.chars().collect();
            for row in y0..y1 {
                if screen_x > 0 && screen_x - 1 < w {
                    grid[row][screen_x - 1] = '|';
                }
                if x1 < w {
                    grid[row][x1] = '|';
                }
                // Center label in the node
                if row == (screen_y + 1).min(h - 1) && screen_x + 1 < w {
                    let label_start = screen_x + 1;
                    for (ci, &ch) in label_chars.iter().enumerate() {
                        let col = label_start + ci;
                        if col < x1 && col < w {
                            grid[row][col] = ch;
                        }
                    }
                }
            }

            // Bottom border
            if screen_y + nh <= h && screen_x < w {
                let bottom_y = (screen_y + nh - 1).min(h - 1);
                if screen_x > 0 {
                    let col = (screen_x - 1).min(w - 1);
                    grid[bottom_y][col] = '+';
                }
                for c in screen_x..x1.min(w) {
                    grid[bottom_y][c] = '-';
                }
                if x1 < w {
                    grid[bottom_y][x1] = '+';
                }
            }

            // Focus indicator: bracket the focused node
            if idx == self.focus_idx.unwrap_or(usize::MAX) {
                if screen_x > 0 && screen_x - 1 < w && screen_y > 0 {
                    let mark_y = (screen_y - 1).min(h - 1);
                    let col = (screen_x - 1).min(w - 1);
                    grid[mark_y][col] = '>';
                }
            }
        }

        // Draw edges as dashed lines between connected nodes
        for edge in &self.edges {
            let from_node = self.nodes.iter().find(|n| n.id == edge.from);
            let to_node = self.nodes.iter().find(|n| n.id == edge.to);
            if let (Some(from_n), Some(to_n)) = (from_node, to_node) {
                let fx_screen = from_n.x + from_n.width as i32 / 2 - self.pan_x;
                let fy_screen = from_n.y + from_n.height as i32 / 2 - self.pan_y;
                let tx_screen = to_n.x + to_n.width as i32 / 2 - self.pan_x;
                let ty_screen = to_n.y + to_n.height as i32 / 2 - self.pan_y;

                // Draw horizontal then vertical (L-shaped)
                let _mid_x = fx_screen;
                let ch = match edge.style {
                    EdgeStyle::Solid => '-',
                    EdgeStyle::Dashed => '~',
                };
                let x_start = fx_screen.min(tx_screen);
                let x_end = fx_screen.max(tx_screen);
                for x in x_start..=x_end {
                    let ux = x as usize;
                    let uy = fy_screen as usize;
                    if ux < w && uy < h && uy > 0 {
                        grid[uy][ux] = ch;
                    }
                }
                let y_start = fy_screen.min(ty_screen);
                let y_end = fy_screen.max(ty_screen);
                for y in y_start..=y_end {
                    let ux = tx_screen as usize;
                    let uy = y as usize;
                    if ux < w && uy < h && ux > 0 {
                        grid[uy][ux] = '|';
                    }
                }
            }
        }

        Frame {
            rows: h,
            cols: w,
            grid,
        }
    }

    /// Project a session family into a graph: one node per family member in
    /// input order, positioned by ancestor depth, plus one solid edge from each
    /// node to its parent when both are present.
    ///
    /// Input order is authoritative (the composer's `sessionFamily` is a
    /// depth-first walk), so nodes keep their roster order vertically. A
    /// cyclic `parent` chain cannot place a depth: such nodes become roots at
    /// depth 0 instead of recursing forever.
    #[must_use]
    pub fn from_family(family: &[FamilyNode], frame_w: usize, frame_h: usize) -> Self {
        let mut nodes = Vec::with_capacity(family.len());
        let mut edges = Vec::new();
        for (row, member) in family.iter().enumerate() {
            let depth = family_depth(family, &member.id);
            let width = member.label.chars().count() as u32 + 2;
            nodes.push(NodeView {
                id: member.id.clone(),
                kind: if depth == 0 {
                    NodeKind::Session
                } else {
                    NodeKind::Subagent
                },
                label: member.label.clone(),
                status: member.status,
                x: depth as i32 * FAMILY_COLUMN_WIDTH,
                y: row as i32 * FAMILY_ROW_HEIGHT,
                width,
                height: FAMILY_NODE_HEIGHT,
            });
            if let Some(parent) = member.parent.as_deref() {
                if family.iter().any(|candidate| candidate.id == parent) {
                    edges.push(EdgeView {
                        from: parent.to_owned(),
                        to: member.id.clone(),
                        style: EdgeStyle::Solid,
                    });
                }
            }
        }
        Self::new(nodes, edges, frame_w, frame_h)
    }

    /// Serialize to JSON string.
    #[must_use]
    pub fn to_json(&self) -> String {
        let snap = GraphSnapshot {
            nodes: self.nodes.clone(),
            edges: self.edges.clone(),
            focus_idx: self.focus_idx,
            pan_x: self.pan_x,
            pan_y: self.pan_y,
            frame_w: self.frame_w,
            frame_h: self.frame_h,
            capacity: self.capacity,
        };
        serde_json::to_string(&snap).expect("GraphSnapshot must serialize")
    }

    /// Deserialize from JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let snap: GraphSnapshot = serde_json::from_str(json)?;
        Ok(Self {
            nodes: snap.nodes,
            edges: snap.edges,
            focus_idx: snap.focus_idx,
            pan_x: snap.pan_x,
            pan_y: snap.pan_y,
            frame_w: snap.frame_w,
            frame_h: snap.frame_h,
            capacity: snap.capacity,
        })
    }
}

// ── Diagram/plot panel surface (`packages/merman`) ────────────────────
//
// The graph page paints the node/edge frame with the chart layout this pin
// uses for terminal diagrams: a label gutter, a two-row axis, one row per
// entry, a legend row and explicit empty/error notes. Upstream evidence
// (pin 450e7b92412c780a7b5a75226bb8d49aaca24e22):
// - `packages/merman/src/gantt/drawing.ts:13-15` — `LABEL_GAP = 2`,
//   `MIN_CHART_WIDTH = 24`, `MAX_CHART_WIDTH = 64`;
// - `packages/merman/src/gantt/drawing.ts:21-22,42-46` — gutter width from the
//   widest label, `chartWidth = clamp(layoutMaxWidth - labelWidth - gap,
//   min, max)`, `titleHeight = 2`, `axisHeight = 2`;
// - `packages/merman/src/gantt/drawing.ts:78-95` — the axis is a `─` rail with
//   `┬` at `round((value - minimum) / span * (width - 1))`, labels clamped into
//   the chart;
// - `packages/merman/src/gantt/drawing.ts:19` — an empty diagram draws a zero
//   canvas instead of inventing rows;
// - `packages/merman/src/markdown.ts:262-305` — `MermaidSyntaxError` and
//   `DiagramCanvasSizeError` fall back instead of crashing the transcript, and
//   an undetected fence yields no renderable so the plain code block stays
//   (`:281-283`);
// - `packages/merman/src/detect.ts:10-17` — the supported kind set and its
//   detection order.
//
// The axis here measures family depth columns (`FAMILY_COLUMN_WIDTH`) rather
// than time. Like the tree layout above, that projection is a local extension
// and is not an upstream claim.

/// Label gutter gap (`gantt/drawing.ts:13`).
pub const GUTTER_GAP: u16 = 2;
/// Minimum chart width (`gantt/drawing.ts:14`).
pub const CHART_MIN_WIDTH: u16 = 24;
/// Maximum chart width (`gantt/drawing.ts:15`).
pub const CHART_MAX_WIDTH: u16 = 64;
/// Title + axis rows reserved above the body (`gantt/drawing.ts:45-46`).
pub const CHART_AXIS_ROWS: u16 = 2;
/// Default responsive diagram layout width (`merman/markdown.ts:284-287`).
pub const DIAGRAM_LAYOUT_MAX_WIDTH: u16 = 120;
/// Longest status message retained by the panel.
pub const MAX_PANEL_MESSAGE: usize = 200;

/// Mermaid diagram kinds detected at this pin, in upstream detection order
/// (`packages/merman/src/detect.ts:10-17`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagramKind {
    Flowchart,
    Gantt,
    GitGraph,
    Sequence,
    State,
    Timeline,
}

impl DiagramKind {
    /// Kind of a ```mermaid fence body, mirroring the per-kind header
    /// predicates (`flowchart/parser.ts:277-279`,
    /// `gantt/parser.ts:11-13`, `gitgraph/parser.ts:8-10`,
    /// `sequence/parser.ts:120-122`, `state/parser.ts:30-32,77-79`,
    /// `timeline/parser.ts:10-12`) on the first meaningful line
    /// (`core/mermaid.ts:19-25`: trimmed, non-empty, not a `%%` directive).
    #[must_use]
    pub fn detect(source: &str) -> Option<Self> {
        let line = first_meaningful_line(source)?;
        if is_flowchart_header(line) {
            return Some(Self::Flowchart);
        }
        if line.eq_ignore_ascii_case("gantt") {
            return Some(Self::Gantt);
        }
        if is_gitgraph_header(line) {
            return Some(Self::GitGraph);
        }
        if line.eq_ignore_ascii_case("sequencediagram") {
            return Some(Self::Sequence);
        }
        if line.eq_ignore_ascii_case("statediagram-v2") || line.eq_ignore_ascii_case("statediagram")
        {
            return Some(Self::State);
        }
        if is_timeline_header(line) {
            return Some(Self::Timeline);
        }
        None
    }

    /// Renderer-facing kind name (`detect.ts:11-16`, `markdown.ts:104`).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Flowchart => "flowchart",
            Self::Gantt => "gantt",
            Self::GitGraph => "gitGraph",
            Self::Sequence => "sequence",
            Self::State => "state",
            Self::Timeline => "timeline",
        }
    }
}

/// Whether the panel body can be painted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelStatus {
    /// At least one node: the frame is drawn.
    Ready,
    /// No nodes at all (`gantt/drawing.ts:19`: a zero canvas, no fake rows).
    Empty,
    /// The fence did not match a supported kind (`markdown.ts:281-283`).
    Unsupported,
    /// Parse failure (`MermaidSyntaxError`, `markdown.ts:262-277`).
    SyntaxError,
    /// Canvas cell-limit failure (`DiagramCanvasSizeError`,
    /// `core/canvas.ts:50-63`, `markdown.ts:304`).
    CanvasTooLarge,
}

/// Role of one panel row (mapped to the page palette by the caller).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelRole {
    Heading,
    Axis,
    Gutter,
    Body,
    Legend,
    Note,
    Error,
    Hint,
}

/// One rendered panel row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelLine {
    pub text: String,
    pub role: PanelRole,
}

/// Legend entry: node kind and how many nodes carry it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegendEntry {
    pub label: String,
    pub count: usize,
}

/// The page-visible graph surface: the frame plus its gutter, axis, legend,
/// horizontal scroll and status note. Geometry is computed once per render.
#[derive(Clone, Debug)]
pub struct GraphPanel {
    view: GraphView,
    status: PanelStatus,
    message: Option<String>,
    legend: Vec<LegendEntry>,
    scroll_x: i32,
}

impl GraphPanel {
    /// Wrap a projected graph. `Ready` with nodes, `Empty` without; the legend
    /// lists only kinds that are actually present.
    #[must_use]
    pub fn new(view: GraphView) -> Self {
        let mut legend = Vec::new();
        for (kind, label) in [
            (NodeKind::Session, "Session"),
            (NodeKind::Subagent, "Subagent"),
        ] {
            let count = view.nodes.iter().filter(|node| node.kind == kind).count();
            if count > 0 {
                legend.push(LegendEntry {
                    label: label.to_owned(),
                    count,
                });
            }
        }
        let status = if view.node_count() == 0 {
            PanelStatus::Empty
        } else {
            PanelStatus::Ready
        };
        Self {
            view,
            status,
            message: None,
            legend,
            scroll_x: 0,
        }
    }

    /// Annotate the surface (unsupported/syntax/size failure). The last good
    /// frame is retained underneath, mirroring the pin's last-good diagram
    /// cache (`markdown.ts:262-270`).
    #[must_use]
    pub fn with_status(mut self, status: PanelStatus, message: Option<&str>) -> Self {
        self.set_status(status, message);
        self
    }

    /// In-place form of [`Self::with_status`] for a surface already owned by
    /// the page state.
    pub fn set_status(&mut self, status: PanelStatus, message: Option<&str>) {
        self.status = status;
        self.message = message.map(|value| clip_columns(value, MAX_PANEL_MESSAGE as u16));
    }

    #[must_use]
    pub fn status(&self) -> PanelStatus {
        self.status
    }

    #[must_use]
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    #[must_use]
    pub fn view(&self) -> &GraphView {
        &self.view
    }

    pub fn view_mut(&mut self) -> &mut GraphView {
        &mut self.view
    }

    #[must_use]
    pub fn legend(&self) -> &[LegendEntry] {
        &self.legend
    }

    /// Horizontal drag offset in cells (`audit-messages.md:82`: dragging the
    /// diagram changes `scrollX`; no wrapping).
    #[must_use]
    pub fn scroll_x(&self) -> i32 {
        self.scroll_x
    }

    /// Set the horizontal drag offset; clamps to `0..=MAX_PAN` and pans the
    /// retained frame with it.
    pub fn set_scroll_x(&mut self, cells: i32) {
        self.scroll_x = cells.max(0).min(MAX_PAN);
        self.view.set_pan(self.scroll_x, self.view.pan_y);
    }

    pub fn scroll_x_by(&mut self, delta: i32) {
        self.set_scroll_x(self.scroll_x.saturating_add(delta));
    }

    /// Widest series label plus [`GUTTER_GAP`] (`gantt/drawing.ts:21-22`),
    /// capped at half the available width so the frame always keeps room.
    #[must_use]
    pub fn gutter_width(&self, width: u16) -> u16 {
        let widest = self
            .view
            .nodes
            .iter()
            .map(|node| node.label.chars().count())
            .max()
            .unwrap_or(0);
        u16::try_from(widest)
            .unwrap_or(u16::MAX)
            .saturating_add(GUTTER_GAP)
            .min(width / 2)
    }

    /// `clamp(layoutMaxWidth - gutter, MIN_CHART_WIDTH, MAX_CHART_WIDTH)`,
    /// then bounded by the row that actually remains (`gantt/drawing.ts:42-44`).
    #[must_use]
    pub fn chart_width(&self, width: u16, gutter: u16) -> u16 {
        let available = width.min(DIAGRAM_LAYOUT_MAX_WIDTH).saturating_sub(gutter);
        available
            .clamp(CHART_MIN_WIDTH, CHART_MAX_WIDTH)
            .min(width.saturating_sub(gutter))
    }

    /// Axis ticks as `(screen offset, depth label)` for the depth columns
    /// currently visible; offsets follow `gantt/drawing.ts:88-95`.
    #[must_use]
    pub fn axis_ticks(&self, chart_width: u16) -> Vec<(u16, String)> {
        let mut depths: Vec<i32> = self.view.nodes.iter().map(|node| node.x).collect();
        depths.sort_unstable();
        depths.dedup();
        let mut ticks = Vec::new();
        for depth in depths {
            let offset = depth - self.view.pan_x;
            if offset < 0 || offset >= i32::from(chart_width) {
                continue;
            }
            ticks.push((offset as u16, (depth / FAMILY_COLUMN_WIDTH).to_string()));
        }
        ticks
    }

    /// The two axis rows: tick labels, then the `─`/`┬` rail.
    fn axis_rows(&self, chart_width: u16) -> [String; 2] {
        let width = usize::from(chart_width);
        if width == 0 {
            return [String::new(), String::new()];
        }
        let mut lanes: Vec<char> = vec![' '; width];
        let mut rail: Vec<char> = vec!['─'; width];
        for (offset, label) in self.axis_ticks(chart_width) {
            let column = usize::from(offset).min(width - 1);
            rail[column] = '┬';
            let glyphs: Vec<char> = label.chars().collect();
            let start = column
                .saturating_sub(glyphs.len() / 2)
                .min(width.saturating_sub(glyphs.len()));
            for (index, glyph) in glyphs.into_iter().enumerate() {
                if start + index < width {
                    lanes[start + index] = glyph;
                }
            }
        }
        [lanes.into_iter().collect(), rail.into_iter().collect()]
    }

    /// Series label for a body row: the node whose box starts on that row.
    fn gutter_label(&self, row: usize) -> Option<&str> {
        let target = i32::try_from(row).ok()?.saturating_add(self.view.pan_y);
        self.view
            .nodes
            .iter()
            .find(|node| node.y == target)
            .map(|node| node.label.as_str())
    }

    fn heading(&self) -> String {
        format!(
            "Graph · {} nodes · {} edges",
            self.view.node_count(),
            self.view.edge_count()
        )
    }

    fn legend_text(&self) -> String {
        let entries: Vec<String> = self
            .legend
            .iter()
            .map(|entry| format!("{} {}", entry.label, entry.count))
            .collect();
        format!("Legend: {}", entries.join(" · "))
    }

    fn status_note(&self, status: PanelStatus) -> String {
        match status {
            PanelStatus::Ready => String::new(),
            PanelStatus::Empty => "no graph nodes".to_owned(),
            PanelStatus::Unsupported => "diagram type is not supported at this pin".to_owned(),
            PanelStatus::SyntaxError => match self.message.as_deref() {
                Some(message) => format!("diagram error: {message}"),
                None => "diagram error".to_owned(),
            },
            PanelStatus::CanvasTooLarge => "diagram exceeds the terminal canvas".to_owned(),
        }
    }

    /// Footer bindings: vertical navigation moves the focus, horizontal drag
    /// pans the frame; Tab/Esc are the shell's.
    pub const FOOTER: &'static str = "↑/↓ move · ←/→ pan · Tab page · Esc close";

    /// Render the panel into at most `height` rows of at most `width` columns.
    #[must_use]
    pub fn rows(&self, width: u16, height: u16) -> Vec<PanelLine> {
        if width == 0 || height == 0 {
            return Vec::new();
        }
        let mut out = vec![PanelLine {
            text: clip_columns(&self.heading(), width),
            role: PanelRole::Heading,
        }];
        if height == 1 {
            return out;
        }
        if !self.legend.is_empty() {
            out.push(PanelLine {
                text: clip_columns(&self.legend_text(), width),
                role: PanelRole::Legend,
            });
            if out.len() == usize::from(height) {
                return out;
            }
        }
        match self.status {
            PanelStatus::Ready => {
                let gutter = self.gutter_width(width);
                let chart = self.chart_width(width, gutter);
                let reserved = usize::from(CHART_AXIS_ROWS) + 1;
                let body = usize::from(height).saturating_sub(out.len() + reserved);
                if gutter == 0 || chart == 0 || body == 0 {
                    out.push(PanelLine {
                        text: clip_columns("graph is too narrow for the diagram frame", width),
                        role: PanelRole::Note,
                    });
                } else {
                    for text in self.axis_rows(chart) {
                        out.push(PanelLine {
                            text: clip_columns(
                                &format!("{}{}", " ".repeat(usize::from(gutter)), text),
                                width,
                            ),
                            role: PanelRole::Axis,
                        });
                    }
                    let frame = self.view.render_sized(usize::from(chart), body);
                    for row in 0..body {
                        let label = self
                            .gutter_label(row)
                            .map(|label| clip_columns(label, gutter))
                            .unwrap_or_default();
                        let diagram: String = frame
                            .grid
                            .get(row)
                            .map(|cells| cells.iter().collect())
                            .unwrap_or_default();
                        out.push(PanelLine {
                            text: clip_columns(
                                &format!("{label:<gutter$}{diagram}", gutter = usize::from(gutter)),
                                width,
                            ),
                            role: if label.is_empty() {
                                PanelRole::Body
                            } else {
                                PanelRole::Gutter
                            },
                        });
                    }
                }
            }
            status => out.push(PanelLine {
                text: clip_columns(&self.status_note(status), width),
                role: if matches!(status, PanelStatus::SyntaxError | PanelStatus::CanvasTooLarge) {
                    PanelRole::Error
                } else {
                    PanelRole::Note
                },
            }),
        }
        while out.len() + 1 < usize::from(height) {
            out.push(PanelLine {
                text: String::new(),
                role: PanelRole::Body,
            });
        }
        out.push(PanelLine {
            text: clip_columns(Self::FOOTER, width),
            role: PanelRole::Hint,
        });
        out.truncate(usize::from(height));
        out
    }
}

/// First trimmed, non-empty line that is not a `%%` directive
/// (`core/mermaid.ts:19-25`).
fn first_meaningful_line(source: &str) -> Option<&str> {
    source
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("%%"))
}

/// `^(flowchart|graph)(?:\s+(TB|TD|BT|LR|RL))?$` (`flowchart/parser.ts:17`).
fn is_flowchart_header(line: &str) -> bool {
    let mut tokens = line.split_whitespace();
    let Some(head) = tokens.next() else {
        return false;
    };
    if !head.eq_ignore_ascii_case("flowchart") && !head.eq_ignore_ascii_case("graph") {
        return false;
    }
    match tokens.next() {
        None => true,
        Some(direction) => {
            matches!(
                direction.to_ascii_uppercase().as_str(),
                "TB" | "TD" | "BT" | "LR" | "RL"
            ) && tokens.next().is_none()
        }
    }
}

/// `^gitGraph(?:\s+(LR|TB|BT))?\s*:?$` (`gitgraph/parser.ts:5`).
fn is_gitgraph_header(line: &str) -> bool {
    let trimmed = line.trim_end_matches(':').trim_end();
    let mut tokens = trimmed.split_whitespace();
    let Some(head) = tokens.next() else {
        return false;
    };
    if !head.eq_ignore_ascii_case("gitgraph") {
        return false;
    }
    match tokens.next() {
        None => true,
        Some(direction) => {
            matches!(direction.to_ascii_uppercase().as_str(), "LR" | "TB" | "BT")
                && tokens.next().is_none()
        }
    }
}

/// `^timeline(?:\s+(TD|LR))?$` (`timeline/parser.ts:5`).
fn is_timeline_header(line: &str) -> bool {
    let mut tokens = line.split_whitespace();
    let Some(head) = tokens.next() else {
        return false;
    };
    if !head.eq_ignore_ascii_case("timeline") {
        return false;
    }
    match tokens.next() {
        None => true,
        Some(direction) => {
            matches!(direction.to_ascii_uppercase().as_str(), "TD" | "LR")
                && tokens.next().is_none()
        }
    }
}

/// Column-bounded cut by characters (never splits a character).
fn clip_columns(text: &str, width: u16) -> String {
    text.chars().take(usize::from(width)).collect()
}

/// Number of ancestors from the first cycle-free root to `id`; cyclic or
/// unknown chains report 0.
fn family_depth(family: &[FamilyNode], id: &str) -> usize {    let mut depth = 0usize;
    let mut current = id;
    let mut seen = std::collections::HashSet::new();
    seen.insert(current);
    while let Some(node) = family.iter().find(|node| node.id == current) {
        let Some(parent) = node.parent.as_deref() else {
            return depth;
        };
        if !family.iter().any(|candidate| candidate.id == parent) || !seen.insert(parent) {
            return depth;
        }
        depth += 1;
        if depth > 64 {
            return 0;
        }
        current = parent;
    }
    depth
}

#[cfg(test)]
mod native_graph_tests {
    use super::*;

    fn family() -> Vec<FamilyNode> {
        vec![
            FamilyNode::new("a", "A", NodeStatus::Running),
            FamilyNode::new("a1", "A1", NodeStatus::Running).with_parent("a"),
            FamilyNode::new("a1x", "A1X", NodeStatus::Idle).with_parent("a1"),
            FamilyNode::new("b", "B", NodeStatus::Done),
        ]
    }

    // P11: the composer family projects to graph nodes/edges with parent
    // identity preserved and roster order authoritative.
    #[test]
    fn family_projection_keeps_order_depth_and_parent_edges() {
        let graph = GraphView::from_family(&family(), 80, 24);
        assert_eq!(graph.node_count(), 4);
        assert_eq!(graph.edge_count(), 2);
        assert_eq!(graph.focused_id(), Some("a"));
        let json = graph.to_json();
        let restored = GraphView::from_json(&json).expect("round-trip");
        assert_eq!(restored.node_count(), 4);
        assert_eq!(restored.edge_count(), 2);
    }

    #[test]
    fn family_projection_positions_by_ancestor_depth() {
        let graph = GraphView::from_family(&family(), 80, 24);
        let restored = GraphView::from_json(&graph.to_json()).unwrap();
        let snapshot: serde_json::Value = serde_json::from_str(&restored.to_json()).unwrap();
        let nodes = snapshot["nodes"].as_array().unwrap();
        assert_eq!(nodes[0]["kind"], "Session");
        assert_eq!(nodes[1]["kind"], "Subagent");
        assert_eq!(nodes[3]["kind"], "Session");
        assert_eq!(nodes[0]["x"], 0);
        assert_eq!(nodes[1]["x"], FAMILY_COLUMN_WIDTH);
        assert_eq!(nodes[2]["x"], 2 * FAMILY_COLUMN_WIDTH);
        assert_eq!(nodes[3]["x"], 0);
        assert_eq!(nodes[2]["y"], 2 * FAMILY_ROW_HEIGHT);
        assert_eq!(nodes[2]["height"], FAMILY_NODE_HEIGHT);
    }

    #[test]
    fn family_projection_skips_parents_outside_the_roster_and_guards_cycles() {
        let orphan = vec![FamilyNode::new("solo", "Solo", NodeStatus::Idle).with_parent("gone")];
        let graph = GraphView::from_family(&orphan, 20, 5);
        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.edge_count(), 0);
        let cyclic = vec![
            FamilyNode::new("x", "X", NodeStatus::Running).with_parent("y"),
            FamilyNode::new("y", "Y", NodeStatus::Running).with_parent("x"),
        ];
        let graph = GraphView::from_family(&cyclic, 20, 5);
        // Both stay placeable (depth 0) and the parent edges are preserved.
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 2);
        let empty = GraphView::from_family(&[], 10, 2);
        assert_eq!(empty.node_count(), 0);
        assert_eq!(empty.focused_id(), None);
    }

    // The frame stays exactly the requested size for a family graph, including
    // when a node lies outside it.
    #[test]
    fn family_projection_render_stays_bounded() {
        let graph = GraphView::from_family(&family(), 12, 4);
        let frame = graph.render();
        assert_eq!(frame.rows, 4);
        assert_eq!(frame.cols, 12);
        assert!(frame.grid.iter().all(|row| row.len() == 12));
    }

    // `render_sized` is the page geometry path: it never mutates the stored
    // frame size and degenerates safely at a zero edge.
    #[test]
    fn render_sized_matches_render_and_keeps_state() {
        let graph = GraphView::from_family(&family(), 80, 24);
        let sized = graph.render_sized(20, 6);
        assert_eq!((sized.rows, sized.cols), (6, 20));
        assert!(sized.grid.iter().all(|row| row.len() == 20));
        // Stored frame size and focus are untouched by a sized render.
        assert_eq!((graph.width(), graph.height()), (80, 24));
        assert_eq!(graph.focused_id(), Some("a"));
        assert_eq!(graph.render(), graph.render_sized(80, 24));
        for (width, height) in [(0, 5), (5, 0), (0, 0), (1, 1)] {
            let frame = graph.render_sized(width, height);
            assert_eq!((frame.rows, frame.cols), (height, width));
        }
    }

    // MD-008 surface: detection mirrors the pin's supported Mermaid kinds and
    // their order (`merman/src/detect.ts:10-17`).
    #[test]
    fn diagram_kind_detection_matches_the_pin_set() {
        assert_eq!(DiagramKind::detect("flowchart LR\n A --> B"), Some(DiagramKind::Flowchart));
        assert_eq!(DiagramKind::detect("%% note\ngraph TD\n A --> B"), Some(DiagramKind::Flowchart));
        assert_eq!(DiagramKind::detect("gantt\n title x"), Some(DiagramKind::Gantt));
        assert_eq!(DiagramKind::detect("gitGraph LR:\n commit"), Some(DiagramKind::GitGraph));
        assert_eq!(DiagramKind::detect("sequenceDiagram\n A->>B: hi"), Some(DiagramKind::Sequence));
        assert_eq!(DiagramKind::detect("stateDiagram-v2\n [*] --> A"), Some(DiagramKind::State));
        assert_eq!(DiagramKind::detect("timeline TD\n 2024 : x"), Some(DiagramKind::Timeline));
        assert_eq!(DiagramKind::detect("pie\n \"a\": 1"), None, "unsupported kind");
        assert_eq!(DiagramKind::detect("   \n%% only comment"), None);
        assert_eq!(DiagramKind::detect("flowchart LR extra"), None, "header must end after the direction");
        assert_eq!(DiagramKind::detect("gantt chart"), None);
        assert_eq!(DiagramKind::detect("timelines"), None);
        assert_eq!(DiagramKind::detect("graph"), Some(DiagramKind::Flowchart));
        // Kind labels are the renderer's own names.
        assert_eq!(DiagramKind::GitGraph.label(), "gitGraph");
        assert_eq!(DiagramKind::Sequence.label(), "sequence");
    }

    // MD-008 panel: gutter width, chart width clamp, axis ticks, one series
    // row per node, legend counts and the bounded row budget.
    #[test]
    fn graph_panel_lays_out_gutter_axis_series_and_legend() {
        let panel = GraphPanel::new(GraphView::from_family(&family(), 80, 24));
        assert_eq!(panel.status(), PanelStatus::Ready);
        assert_eq!(panel.scroll_x(), 0);
        // Widest label ("A1X", 3) + GUTTER_GAP, capped at half the width.
        assert_eq!(panel.gutter_width(80), 5);
        assert_eq!(panel.gutter_width(4), 2);
        // `clamp(80 - 5, 24, 64)`.
        assert_eq!(panel.chart_width(80, 5), 64);
        assert_eq!(panel.chart_width(30, 5), 25);
        assert_eq!(panel.chart_width(20, 5), 15, "never wider than the row");
        // Depth columns: 0, 24, 48 for depths 0/1/2.
        let ticks = panel.axis_ticks(64);
        assert_eq!(ticks, vec![(0, "0".to_owned()), (24, "1".to_owned()), (48, "2".to_owned())]);
        // Legend counts only kinds that are present, in kind order.
        assert_eq!(
            panel.legend(),
            &[
                LegendEntry { label: "Session".to_owned(), count: 2 },
                LegendEntry { label: "Subagent".to_owned(), count: 2 },
            ][..]
        );

        let rows = panel.rows(80, 20);
        assert_eq!(rows.len(), 20);
        assert_eq!(rows[0].text, "Graph · 4 nodes · 2 edges");
        assert_eq!(rows[0].role, PanelRole::Heading);
        assert_eq!(rows[1].text, "Legend: Session 2 · Subagent 2");
        assert_eq!(rows[1].role, PanelRole::Legend);
        // Two axis rows, gutter-indented, with the rail and its ticks.
        assert_eq!(rows[2].role, PanelRole::Axis);
        assert!(rows[3].text.contains('┬'));
        assert!(rows[3].text.contains('─'));
        // Series rows carry each node label in the gutter.
        let gutter_rows: Vec<&PanelLine> = rows
            .iter()
            .filter(|row| row.role == PanelRole::Gutter)
            .collect();
        assert_eq!(gutter_rows.len(), 4, "one series row per family member");
        assert!(gutter_rows[0].text.starts_with("A"));
        assert!(gutter_rows[1].text.starts_with("A1"), "{:?}", gutter_rows[1].text);
        // Footer is the last row.
        assert_eq!(rows[19].text, GraphPanel::FOOTER);
        assert_eq!(rows[19].role, PanelRole::Hint);
        // Every row is inside the requested bounds.
        for width in [1u16, 3, 9, 40, 200] {
            for height in [1u16, 2, 3, 7, 40] {
                let rows = panel.rows(width, height);
                assert!(rows.len() <= usize::from(height));
                for row in &rows {
                    assert!(row.text.chars().count() <= usize::from(width), "{:?}", row.text);
                }
            }
        }
        assert!(panel.rows(0, 10).is_empty());
        assert!(panel.rows(10, 0).is_empty());
    }

    // MD-008 panel states: horizontal drag pans and clamps; empty, unsupported,
    // syntax and oversize surfaces show bounded notes instead of a fake frame.
    #[test]
    fn graph_panel_scroll_and_status_states() {
        let mut panel = GraphPanel::new(GraphView::from_family(&family(), 80, 24));
        panel.scroll_x_by(5);
        assert_eq!(panel.scroll_x(), 5);
        assert_eq!(panel.view().pan().0, 5);
        // The depth-0 tick scrolled out of view; depth 1 moved left by 5.
        assert_eq!(
            panel.axis_ticks(64),
            vec![(19, "1".to_owned()), (43, "2".to_owned())]
        );
        panel.scroll_x_by(-5);
        assert_eq!(panel.scroll_x(), 0, "scroll clamps at zero");
        panel.scroll_x_by(MAX_PAN + 10);
        assert_eq!(panel.scroll_x(), MAX_PAN);
        panel.set_scroll_x(0);

        // Empty graph: no frame, an explicit note (`gantt/drawing.ts:19`).
        let empty = GraphPanel::new(GraphView::new(Vec::new(), Vec::new(), 40, 10));
        assert_eq!(empty.status(), PanelStatus::Empty);
        assert!(empty.legend().is_empty());
        let rows = empty.rows(40, 8);
        assert_eq!(rows[0].text, "Graph · 0 nodes · 0 edges");
        assert!(rows.iter().any(|row| row.text == "no graph nodes" && row.role == PanelRole::Note));

        // Unsupported fence: the plain-code fallback is stated, not painted
        // (`merman/markdown.ts:281-283`).
        let unsupported = GraphPanel::new(GraphView::from_family(&family(), 40, 10))
            .with_status(PanelStatus::Unsupported, None);
        assert!(unsupported
            .rows(60, 8)
            .iter()
            .any(|row| row.text.contains("not supported at this pin")));
        // The same note is width-bounded on a narrow panel.
        assert!(unsupported
            .rows(20, 8)
            .iter()
            .any(|row| row.text == "diagram type is not "));

        // Syntax error: message retained (bounded), error role.
        let syntax = GraphPanel::new(GraphView::from_family(&family(), 40, 10))
            .with_status(PanelStatus::SyntaxError, Some("unexpected token on line 3"));
        assert_eq!(syntax.message(), Some("unexpected token on line 3"));
        let rows = syntax.rows(60, 8);
        let error = rows
            .iter()
            .find(|row| row.role == PanelRole::Error)
            .expect("error row");
        assert!(error.text.contains("diagram error: unexpected token on line 3"));

        // Oversize canvas: the failure is named, the note is width-bounded.
        let oversized = GraphPanel::new(GraphView::from_family(&family(), 40, 10))
            .with_status(PanelStatus::CanvasTooLarge, None);
        let rows = oversized.rows(12, 6);
        assert!(rows.iter().any(|row| row.role == PanelRole::Error));
        for row in &rows {
            assert!(row.text.chars().count() <= 12, "{:?}", row.text);
        }
        // A very long message stays bounded by the row width.
        let long = GraphPanel::new(GraphView::from_family(&family(), 40, 10))
            .with_status(PanelStatus::SyntaxError, Some(&"x".repeat(MAX_PANEL_MESSAGE + 50)));
        assert_eq!(long.message().map(str::len), Some(MAX_PANEL_MESSAGE));
        assert!(long.rows(20, 6).iter().all(|row| row.text.chars().count() <= 20));
    }
}
