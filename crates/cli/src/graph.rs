#![forbid(unsafe_code)]
//! Bounded graph state + paint to opentui buffers.
//!
//! Pure state only: nodes/edges with dup/missing-endpoint errors,
//! hard caps, adjacency lookup, clipped paint into a char buffer.
//! No rendering, no IO, no threads; caller owns the native buffer.

/// Max nodes retained.
pub const MAX_NODES: usize = 256;
/// Max edges retained.
pub const MAX_EDGES: usize = 512;
/// Max node id chars (truncated on insert).
pub const MAX_ID_LEN: usize = 64;
/// Max label chars (truncated on insert).
pub const MAX_LABEL_LEN: usize = 64;

/// Graph mutation failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphError {
    DuplicateNode,
    UnknownNode,
    NodeCap,
    EdgeCap,
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateNode => write!(f, "duplicate node"),
            Self::UnknownNode => write!(f, "unknown node"),
            Self::NodeCap => write!(f, "node cap reached"),
            Self::EdgeCap => write!(f, "edge cap reached"),
        }
    }
}

impl std::error::Error for GraphError {}

/// One graph node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub x: i32,
    pub y: i32,
}

/// One directed edge by node id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    pub from: String,
    pub to: String,
}

/// Bounded directed graph.
#[derive(Clone, Debug, Default)]
pub struct Graph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

fn cap(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

impl Graph {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    #[must_use]
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Insert node; id/label truncated to cap. Dup id errors.
    pub fn add_node(&mut self, id: &str, label: &str, x: i32, y: i32) -> Result<(), GraphError> {
        let id = cap(id, MAX_ID_LEN);
        if self.nodes.iter().any(|n| n.id == id) {
            return Err(GraphError::DuplicateNode);
        }
        if self.nodes.len() >= MAX_NODES {
            return Err(GraphError::NodeCap);
        }
        self.nodes.push(Node { id, label: cap(label, MAX_LABEL_LEN), x, y });
        Ok(())
    }

    /// Insert edge; both endpoints must exist.
    pub fn add_edge(&mut self, from: &str, to: &str) -> Result<(), GraphError> {
        if self.get(from).is_none() || self.get(to).is_none() {
            return Err(GraphError::UnknownNode);
        }
        if self.edges.len() >= MAX_EDGES {
            return Err(GraphError::EdgeCap);
        }
        self.edges.push(Edge { from: from.to_string(), to: to.to_string() });
        Ok(())
    }

    /// Outgoing neighbor ids.
    #[must_use]
    pub fn neighbors(&self, id: &str) -> Vec<&str> {
        self.edges
            .iter()
            .filter(|e| e.from == id)
            .filter_map(|e| self.get(&e.to).map(|n| n.id.as_str()))
            .collect()
    }

    /// Paint into `buf` (row-major, `w*h` cells). Clipped, bounded.
    pub fn paint_into(&self, buf: &mut [char], w: usize, h: usize) {
        if buf.len() < w.saturating_mul(h) {
            return;
        }
        for c in buf.iter_mut().take(w * h) {
            *c = ' ';
        }
        let mut put = |x: i32, y: i32, ch: char| {
            if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
                buf[y as usize * w + x as usize] = ch;
            }
        };
        for e in &self.edges {
            let (Some(a), Some(b)) = (self.get(&e.from), self.get(&e.to)) else { continue };
            let (x0, y0, x1, y1) = (a.x, a.y, b.x, b.y);
            for x in x0.min(x1)..=x0.max(x1) {
                put(x, y0, '-');
            }
            for y in y0.min(y1)..=y0.max(y1) {
                put(x1, y, '|');
            }
            put(x1, y0, '+');
        }
        for n in &self.nodes {
            put(n.x, n.y, 'O');
            for (i, ch) in n.label.chars().enumerate() {
                put(n.x + 1 + i as i32, n.y, ch);
            }
        }
    }

    /// Paint to owned rows (convenience over `paint_into`).
    #[must_use]
    pub fn render(&self, w: usize, h: usize) -> Vec<String> {
        let mut buf = vec![' '; w.saturating_mul(h)];
        self.paint_into(&mut buf, w, h);
        buf.chunks(w.max(1)).map(|r| r.iter().collect()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dup_node_errors() {
        let mut g = Graph::new();
        g.add_node("a", "A", 1, 1).unwrap();
        assert_eq!(g.add_node("a", "A2", 2, 2), Err(GraphError::DuplicateNode));
    }

    #[test]
    fn edge_missing_node_errors() {
        let mut g = Graph::new();
        g.add_node("a", "A", 0, 0).unwrap();
        assert_eq!(g.add_edge("a", "b"), Err(GraphError::UnknownNode));
        assert_eq!(g.add_edge("b", "a"), Err(GraphError::UnknownNode));
    }

    #[test]
    fn caps_reject() {
        let mut g = Graph::new();
        for i in 0..MAX_NODES {
            g.add_node(&format!("n{i}"), "x", 0, 0).unwrap();
        }
        assert_eq!(g.add_node("over", "x", 0, 0), Err(GraphError::NodeCap));
        g.add_edge("n0", "n1").unwrap();
        assert_eq!(g.neighbors("n0"), vec!["n1"]);
        let rows = g.render(8, 2);
        assert_eq!(rows.len(), 2);
    }
}
