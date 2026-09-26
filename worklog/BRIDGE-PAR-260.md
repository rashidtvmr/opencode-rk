# BRIDGE-PAR-260: home_route_full

Claim: BRIDGE-PAR-260, session ses_par260.
Source evidence:
- crates/opentui-bridge/src/home_route.rs:40 HomeRoute struct (cwd, recent, dest)
- crates/opentui-bridge/src/home_screen.rs:19 home_lines(route, footer, width, height)->Vec<String>
- crates/opentui-bridge/src/home_footer.rs:20 HomeFooter struct
- crates/opentui-bridge/src/fork_route_full.rs:1 style ref (bounded, forbid unsafe, compact tests)
Observed: no home_route_full.rs; HomeFlow missing.
Target boundary: ONE new file crates/opentui-bridge/src/home_route_full.rs. No lib.rs/Cargo.toml/home_route.rs/home_screen.rs edits. No cargo/commit.
Tests: 4 unit tests in-file (new zero, render matches home_lines, views bump x2, route mutation reflected).
Decisions: HomeFlow { pub route, views: u64 priv } + new(route) + render(&mut self, footer, w, h) saturating bump + views(). forbid(unsafe_code), std-only, <110 lines.
Unknowns: lib.rs wiring left to orchestrator (out of scope).
