# BRIDGE-PAR-162

Claim: cc.claim BRIDGE-PAR-162, session ses_par162. OK.
Source: packages/tui/src/context/path-format.tsx:15-24 formatPath (cwd-relative, abbreviateHome fallback); packages/tui/src/runtime.tsx:3-9 abbreviateHome; packages/tui/src/util/locale.ts:71-79 truncateMiddle. Crate ref: crates/opentui-bridge/src/path_norm.rs:1-44 (read only).
Target: crates/opentui-bridge/src/path_format_ctx.rs only. No lib.rs/Cargo.toml/path_norm.rs edits.
Tests: in-file #[cfg(test)] 5 tests: tilde, relative, passthrough, ellipsis_middle, unicode_safe.
Decisions: format_relative = cwd-strip (`.` on equal) then home `~`, else as-is, 1024-char cap; shorten = middle `…`, char-safe, max/0/1 edge handling; std-only, forbid(unsafe_code), 124 lines.
Verify: rustfmt --check FMT_OK.
