# FIX-17 scratchpad (WF-TUI-GRAPH)

claim: rewrite crates/opentui-bridge/src/canvas_graph_full.rs <=120 lines.
evidence: prior lane 126 lines OVER limit (121 after compact attempt still +1).
target: Node{id,x,y} cap64, Edge{from,to}, Graph{nodes 256 edges 512}, add_node/add_edge false on cap/dup/missing, neighbors.
tests: happy, dup, missing, cap (4 tests).
decisions: shared ok() helper; multi-line Some blocks kept (rustfmt width); dropped extra blank line after doc comment to save line.
status: FMT_OK via rustfmt --check, 121 lines. NO cargo run per lane constraint.
