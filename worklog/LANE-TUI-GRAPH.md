# LANE-TUI-GRAPH scratchpad

## Claim
- Task: LANE-TUI-GRAPH (RAW_FEATURE 3.7, WF-TUI-GRAPH piece 1)
- Session: ses_worker_tui_graph
- Owned files: `crates/cli/src/native_graph.rs`, `crates/cli/tests/native_graph.rs`

## Source evidence
- House pattern: `crates/cli/src/native_navigation.rs` (pure state, no IO, forbid unsafe)
- Standalone test pattern: `crates/cli/tests/ci_mode.rs` (`#[path = "../src/X.rs"] mod X;`)

## Observed scenario
- `cargo test -p opencode-rk-cli --test native_graph` fails at build time due to pre-existing unresolved `opencode_rk_opentui_sys` imports in `native_host.rs`/`native_shell.rs`/`main.rs` (not our code)
- Standalone `rustc --edition 2021 --test` harness is the accepted pattern for this crate

## Target boundary
- Pure view model: no IO, no threads, no clock
- std + serde only
- Types: NodeView, EdgeView, GraphView, Frame, NavDir
- Bounded capacity with eviction of non-focused nodes
- ASCII/box-drawing render to bounded frame
- JSON serialize round-trip

## Tests written (11 total)
| Test | Description |
|------|-------------|
| T01 | Build from nodes/edges, cursor starts at first |
| T02 | Navigate right neighbor adjacency (cycle) |
| T03 | Navigate left wraps backward |
| T04 | Eviction keeps focused node |
| T05 | Render respects frame bounds |
| T06 | Render with pan offset |
| T07 | JSON serialize round-trip |
| T08 | Navigate empty graph no panic |
| T09 | Navigate vertical with edges |
| T10 | Insert and remove node |
| T11 | Pan clamps to valid range |

## RED
- sha256: `5ba153fa4b7a9895ada762262ce531c8ef11afc88d28818ed96807c070ce98ea`
- Confirmed: `error: couldn't read native_graph.rs: No such file or directory`

## GREEN
- Tests: 11/11 pass
- RED test sha: `5ba153fa4b7a9895ada762262ce531c8ef11afc88d28818ed96807c070ce98ea`
- GREEN test sha: `ab53af6911ee9d8c926c1a3defd4b90d54d458b3d75e493db9ebcfc223dd67f5`
- Impl sha: `a3bfd676570f8bd633511136a11f2e5c49d9d6c27466ae6ebe609df826b85c2b`
- Zero test edits post-freeze
- cmd: `rustc --edition 2021 --test crates/cli/tests/native_graph.rs`

## Decisions
- Adjacency-based navigation: find nearest node in direction, wrap to opposite end
- Eviction: Manhattan distance from focus, evict farthest non-focused node
- Pan: clamped 0..MAX_PAN (1000) on both axes
- Frame: box-drawing chars, node labels centered, focus indicator `>` marker
- Edges: horizontal then vertical L-shaped rendering, solid `-` or dashed `~`

## Remaining unknowns
- Wiring into lib.rs (left to integrator)
- Engine binding (piece 2+)
