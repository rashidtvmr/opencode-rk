#![forbid(unsafe_code)]
//! Bounded keyboard-navigable nodes/edges graph view model (TUI-GRAPH).
//!
//! Pure state only: view model for a directed graph of session/subagent nodes
//! with edges, cursor navigation by adjacency, bounded capacity with eviction
//! of non-focused nodes, ASCII/box-drawing render to a bounded frame, and
//! JSON serialization for the native shell.
//!
//! No rendering, no IO, no clock, no threads.

use serde::{Deserialize, Serialize};

/// Upper bound on retained nodes.
pub const MAX_GRAPH_NODES: usize = 256;
/// Upper bound on retained edges.
pub const MAX_GRAPH_EDGES: usize = 512;
/// Max pan coordinate (both axes).
pub const MAX_PAN: i32 = 1000;

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
            self.edges.retain(|e| e.from != removed_id && e.to != removed_id);
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
        let w = self.frame_w;
        let h = self.frame_h;
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
