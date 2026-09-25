# BRIDGE-PAR-190 home_screen frame

Claim: ledger BRIDGE-PAR-190 ses_par190 in-progress. Owned file only: crates/opentui-bridge/src/home_screen.rs. No edit to lib.rs, Cargo.toml, home_route.rs, home_footer.rs.

Source evidence:
- crates/opentui-bridge/src/home_route.rs:40 HomeRoute { cwd, recent, dest }, :91 title() `home <abbrev>`.
- crates/opentui-bridge/src/home_footer.rs:49 current() Option<&str>, :56 render(width) char-safe clip.
- packages/tui/src/routes/home.tsx:22 Home prompt-first layout, :90 home_footer slot.
- Pattern: crates/opentui-bridge/src/page_adapter.rs:82 page_lines (width.max(1), height.max(1), truncate, char-safe clip).

Target boundary: home_lines(route, footer, width, height)->Vec<String>. Layout: title, `cwd <dir>`, recent list or `recent: none`, blank pad, footer current last. Exact height rows, width char-safe clip, footer survives truncation (height-1 body + footer).

Tests: 6 in-file (height/title, cwd+recent, empty placeholder, footer-last + h=1, trunc-keeps-footer, multibyte clip).

Decisions: footer via current()+clip (not render(), same result, one clip path for body+footer). `  {d}` indent for recent entries. No lib.rs wiring per scope (integrator prewires `pub mod home_screen`).

Unknowns: none. Verification: rustfmt --check only per task (no cargo).
