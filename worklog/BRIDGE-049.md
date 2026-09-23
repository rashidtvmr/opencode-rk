# BRIDGE-049 worklog
- Created crates/opentui-bridge/src/diff_tree.rs (no cargo run, per scope).
- Evidence: utils:76-104/197-216, tsx:148, diff-viewer.tsx:399-406/516-562, diff_viewer.rs:34-66.
- Tests: 8 (order, collapse-skip, depth-cap, filter-ci, filter-empty, toggle/oob, bound-1024, setters).
- Unknowns: lib.rs wiring belongs to integrator (scope forbids).
