#![forbid(unsafe_code)]
//! Infinite canvas node/edge graph (WF-TUI-GRAPH substrate: parent -> child).
pub const MAX_NODE_ID: usize = 64;
pub const MAX_NODES: usize = 256;
pub const MAX_EDGES: usize = 512;
fn ok(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_NODE_ID
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub id: String,
    pub x: i32,
    pub y: i32,
}
impl Node {
    pub fn new(id: &str, x: i32, y: i32) -> Option<Self> {
        if !ok(id) {
            return None;
        }
        Some(Self {
            id: id.into(),
            x,
            y,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub from: String,
    pub to: String,
}
impl Edge {
    pub fn new(from: &str, to: &str) -> Option<Self> {
        if !ok(from) || !ok(to) {
            return None;
        }
        Some(Self {
            from: from.into(),
            to: to.into(),
        })
    }
}
#[derive(Debug, Clone, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}
impl Graph {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add_node(&mut self, n: Node) -> bool {
        if self.nodes.len() >= MAX_NODES || self.nodes.iter().any(|x| x.id == n.id) {
            return false;
        }
        self.nodes.push(n);
        true
    }
    pub fn add_edge(&mut self, e: Edge) -> bool {
        if self.edges.len() >= MAX_EDGES {
            return false;
        }
        if self.edges.iter().any(|x| x.from == e.from && x.to == e.to) {
            return false;
        }
        let (f, t) = (
            self.nodes.iter().any(|x| x.id == e.from),
            self.nodes.iter().any(|x| x.id == e.to),
        );
        if !f || !t {
            return false;
        }
        self.edges.push(e);
        true
    }
    pub fn neighbors(&self, id: &str) -> Vec<&Node> {
        self.edges
            .iter()
            .filter(|e| e.from == id)
            .filter_map(|e| self.nodes.iter().find(|n| n.id == e.to))
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn happy() {
        let mut g = Graph::new();
        g.add_node(Node::new("a", 0, 0).unwrap());
        g.add_node(Node::new("b", 1, 1).unwrap());
        assert!(g.add_edge(Edge::new("a", "b").unwrap()));
        assert_eq!(g.neighbors("a").len(), 1);
    }
    #[test]
    fn dup() {
        let mut g = Graph::new();
        g.add_node(Node::new("a", 0, 0).unwrap());
        assert!(!g.add_node(Node::new("a", 1, 1).unwrap()));
        assert!(g.add_edge(Edge::new("a", "a").unwrap()));
        assert!(!g.add_edge(Edge::new("a", "a").unwrap()));
    }
    #[test]
    fn missing() {
        let mut g = Graph::new();
        g.add_node(Node::new("a", 0, 0).unwrap());
        assert!(!g.add_edge(Edge::new("a", "z").unwrap()));
        assert!(g.neighbors("z").is_empty() && Node::new("", 0, 0).is_none());
    }
    #[test]
    fn cap() {
        let mut g = Graph::new();
        for i in 0..300 {
            g.add_node(Node::new(&format!("n{i}"), 0, 0).unwrap());
        }
        assert_eq!(g.nodes.len(), MAX_NODES);
    }
}
