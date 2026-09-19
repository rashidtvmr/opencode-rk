#![forbid(unsafe_code)]
//! LANE-TUI-GRAPH frozen tests: bounded graph view model.
//!
//! Scenario A: build from nodes/edges, cursor starts at first node.
//! Scenario B: navigate to neighbor by adjacency (cycle-safe).
//! Scenario C: eviction keeps focused node.
//! Scenario D: render bounds (frame size cap).
//! Scenario E: JSON serialize round-trip.

#[path = "../src/native_graph.rs"]
mod native_graph;

use native_graph::*;

// ── helpers ──

fn node(id: &str, kind: NodeKind, label: &str, x: i32, y: i32) -> NodeBuilder {
    NodeBuilder {
        id: id.to_string(),
        kind,
        label: label.to_string(),
        x,
        y,
    }
}

struct NodeBuilder {
    id: String,
    kind: NodeKind,
    label: String,
    x: i32,
    y: i32,
}

impl NodeBuilder {
    fn build(self) -> NodeView {
        let width = self.label.len() as u32 + 2;
        NodeView {
            id: self.id,
            kind: self.kind,
            label: self.label,
            status: NodeStatus::Idle,
            x: self.x,
            y: self.y,
            width,
            height: 3,
        }
    }
}

fn edge(from: &str, to: &str) -> EdgeView {
    EdgeView {
        from: from.to_string(),
        to: to.to_string(),
        style: EdgeStyle::Solid,
    }
}

// ── T01: build from nodes/edges, cursor starts at first node ──

#[test]
fn t01_build_from_nodes_and_edges() {
    let nodes = vec![
        node("a", NodeKind::Session, "Session A", 0, 0).build(),
        node("b", NodeKind::Subagent, "Sub B", 10, 0).build(),
    ];
    let edges = vec![edge("a", "b")];
    let graph = GraphView::new(nodes, edges, 80, 24);
    assert_eq!(graph.node_count(), 2);
    assert_eq!(graph.edge_count(), 1);
    assert_eq!(graph.focused_id(), Some("a"));
}

// ── T02: navigate to neighbor by adjacency, cycle-safe ──

#[test]
fn t02_navigate_to_neighbor_adjacency() {
    let nodes = vec![
        node("a", NodeKind::Session, "A", 0, 0).build(),
        node("b", NodeKind::Subagent, "B", 10, 0).build(),
        node("c", NodeKind::Session, "C", 20, 0).build(),
    ];
    let edges = vec![edge("a", "b"), edge("b", "c")];
    let mut graph = GraphView::new(nodes, edges, 80, 24);
    assert_eq!(graph.focused_id(), Some("a"));
    graph.navigate(NavDir::Right);
    assert_eq!(graph.focused_id(), Some("b"));
    graph.navigate(NavDir::Right);
    assert_eq!(graph.focused_id(), Some("c"));
    // Cycle: right from last goes to first
    graph.navigate(NavDir::Right);
    assert_eq!(graph.focused_id(), Some("a"));
}

// ── T03: navigate left wraps backward ──

#[test]
fn t03_navigate_left_wraps() {
    let nodes = vec![
        node("a", NodeKind::Session, "A", 0, 0).build(),
        node("b", NodeKind::Subagent, "B", 10, 0).build(),
    ];
    let edges = vec![edge("a", "b")];
    let mut graph = GraphView::new(nodes, edges, 80, 24);
    assert_eq!(graph.focused_id(), Some("a"));
    graph.navigate(NavDir::Left);
    assert_eq!(graph.focused_id(), Some("b"));
}

// ── T04: eviction keeps focused node ──

#[test]
fn t04_eviction_keeps_focused() {
    let mut nodes = Vec::new();
    for i in 0..100 {
        nodes.push(node(
            &format!("n{i}"),
            NodeKind::Session,
            &format!("N{i}"),
            i * 10,
            0,
        )
        .build());
    }
    let edges: Vec<EdgeView> = Vec::new();
    let mut graph = GraphView::with_capacity(nodes, edges, 80, 24, 10);
    assert_eq!(graph.node_count(), 10);
    // Focus should be on first visible
    let focused = graph.focused_id().unwrap().to_string();
    // Evict more nodes
    for i in 10..50 {
        graph.insert_node(node(
            &format!("n{i}"),
            NodeKind::Session,
            &format!("N{i}"),
            i * 10,
            0,
        )
        .build());
    }
    assert_eq!(graph.node_count(), 10);
    // Focused node must survive eviction
    assert_eq!(graph.focused_id(), Some(focused.as_str()));
}

// ── T05: render bounds — frame size cap ──

#[test]
fn t05_render_respects_frame_bounds() {
    let nodes = vec![
        node("a", NodeKind::Session, "Session Alpha", 0, 0).build(),
        node("b", NodeKind::Subagent, "Sub Beta", 20, 5).build(),
    ];
    let edges = vec![edge("a", "b")];
    let graph = GraphView::new(nodes, edges, 40, 10);
    let frame = graph.render();
    // Frame must be exactly the requested dimensions
    assert_eq!(frame.rows, 10);
    assert_eq!(frame.cols, 40);
    // Every row must be exactly cols wide
    for row in &frame.grid {
        assert_eq!(row.len(), 40);
    }
}

// ── T06: render with pan offset ──

#[test]
fn t06_render_with_pan_offset() {
    let nodes = vec![
        node("a", NodeKind::Session, "A", 0, 0).build(),
        node("b", NodeKind::Subagent, "B", 50, 0).build(),
    ];
    let edges = vec![edge("a", "b")];
    let mut graph = GraphView::new(nodes, edges, 20, 5);
    graph.set_pan(30, 0);
    let frame = graph.render();
    assert_eq!(frame.cols, 20);
    // Node A is at x=0, pan=30, so A is off-screen left
    // Node B is at x=50, pan=30, so B is visible at screen x=20
    // We just verify the frame renders without panic and is bounded
    for row in &frame.grid {
        assert_eq!(row.len(), 20);
    }
}

// ── T07: JSON serialize round-trip ──

#[test]
fn t07_json_serialize_round_trip() {
    let nodes = vec![
        node("a", NodeKind::Session, "Session A", 0, 0).build(),
        node("b", NodeKind::Subagent, "Sub B", 10, 5).build(),
    ];
    let edges = vec![edge("a", "b")];
    let graph = GraphView::new(nodes, edges, 80, 24);
    let json = graph.to_json();
    let restored = GraphView::from_json(&json).expect("deserialize must succeed");
    assert_eq!(restored.node_count(), 2);
    assert_eq!(restored.edge_count(), 1);
    assert_eq!(restored.focused_id(), Some("a"));
    assert_eq!(restored.width(), 80);
    assert_eq!(restored.height(), 24);
}

// ── T08: navigate empty graph — no panic ──

#[test]
fn t08_navigate_empty_graph() {
    let mut graph = GraphView::new(vec![], vec![], 80, 24);
    graph.navigate(NavDir::Right);
    graph.navigate(NavDir::Left);
    graph.navigate(NavDir::Up);
    graph.navigate(NavDir::Down);
    assert_eq!(graph.focused_id(), None);
}

// ── T09: navigate vertical with edges ──

#[test]
fn t09_navigate_vertical_with_edges() {
    let nodes = vec![
        node("top", NodeKind::Session, "Top", 0, 0).build(),
        node("bot", NodeKind::Subagent, "Bot", 0, 10).build(),
    ];
    let edges = vec![edge("top", "bot")];
    let mut graph = GraphView::new(nodes, edges, 80, 24);
    assert_eq!(graph.focused_id(), Some("top"));
    graph.navigate(NavDir::Down);
    assert_eq!(graph.focused_id(), Some("bot"));
    graph.navigate(NavDir::Up);
    assert_eq!(graph.focused_id(), Some("top"));
}

// ── T10: insert and remove node ──

#[test]
fn t10_insert_and_remove_node() {
    let nodes = vec![node("a", NodeKind::Session, "A", 0, 0).build()];
    let mut graph = GraphView::new(nodes, vec![], 80, 24);
    assert_eq!(graph.node_count(), 1);
    graph.insert_node(node("b", NodeKind::Subagent, "B", 10, 0).build());
    assert_eq!(graph.node_count(), 2);
    graph.remove_node("a");
    assert_eq!(graph.node_count(), 1);
    assert_eq!(graph.focused_id(), Some("b"));
}

// ── T11: pan clamps to valid range ──

#[test]
fn t11_pan_clamps() {
    let nodes = vec![node("a", NodeKind::Session, "A", 0, 0).build()];
    let mut graph = GraphView::new(nodes, vec![], 80, 24);
    graph.set_pan(-100, -100);
    let (px, py) = graph.pan();
    assert!(px >= 0);
    assert!(py >= 0);
    graph.set_pan(10000, 10000);
    let (px, py) = graph.pan();
    assert!(px <= 1000);
    assert!(py <= 1000);
}
