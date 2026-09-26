# BRIDGE-PAR-142 scratchpad

Claim: BRIDGE-PAR-142 via cc.claim, session ses_par142. OK.
Source evidence:
- TS truth /home/rashid/projects/opencode/packages/tui/src/routes/session/sidebar.tsx:12-103 (session title/share/workspace sidebar, no per-panel counts; panels live in feature-plugins/sidebar per sidebar.rs doc).
- Panel naming crate::sidebar.rs:18-26 enum Panel {Files, Context, Lsp, Mcp, Todo}; mirrored as SidePanel2 (Files, Context, Todo, Mcp, Lsp) to avoid clash. Did NOT edit sidebar.rs/lib.rs/Cargo.toml.
Target boundary: ONE new file crates/opentui-bridge/src/sidebar_panels.rs, std-only, forbid(unsafe_code), <150 lines (139).
Tests: 6 in-file #[cfg(test)] (labels, missing zero, set/overwrite, summary parts, trunc, cap-8).
Decisions: counts keyed by truncated (32-char) name string, not enum, per spec signature set_count(name:&str); overwrite when key exists even at cap; count_of truncates query so long names match.
Verification: rustfmt --check PASS. No cargo per scope.
