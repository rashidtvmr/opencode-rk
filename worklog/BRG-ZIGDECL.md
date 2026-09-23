# BRG-ZIGDECL — audited `extern C` declaration inventory (TS-bridge-called surface)

## Claim
- Task: BRG-ZIGDECL, session ses_brg_zigdecl, status in-progress (ledger pre-held by this session; `claim` re-run returns ClaimError=fenced-by-self, no collision).
- Owned file ONLY: `crates/opentui-bridge/src/zigdecl.rs` (+ this scratchpad). No lib.rs touch (standalone rustc target).

## Source evidence (exact)
- Zig exports: `/Users/mymac/Projects/opentui/packages/native/src/lib.zig` (4003 lines) — 369 unique `export fn`.
- TS bridge table: `/Users/mymac/Projects/opentui/packages/core/src/zig.ts` — 399 symbol entries with `(args, returns)` bun:ffi types (parsed via regex `^    NAME: \{$` + args/returns).
- Intersection (TS-bridge-called surface): **337** decls. TS-only 62 (yoga ~60 + stream/spanFeed ~12 — provided by yoga.zig/native-span-feed, not lib.zig exports). Zig-only 32 (image* ~17, kitty-image-transport, hyperlinks cap, etc. — not called via zig.ts table).
- Existing decls (~35): `crates/opentui-bridge/src/renderer.rs` (26 fns + ExternalBuildOptions), `buffer.rs` (12 fns + GridDrawOptions), `text.rs` (18 fns), `safe_renderer.rs` (subset re-decl, 18 fns), `handle.rs` (RenderStatus consts 0/1/2).
- Key Zig lines: createRenderer:1154, setTerminalEnvVar:1210, setUseThread:1217, setClearOnShutdown:1222, destroyRenderer:1227, setBackgroundColor:1257, setRenderOffset:1262, getNextBuffer:1337, getCurrentBuffer:1342, getBufferWidth:1371, getBufferHeight:1376, render:1394, createOptimizedBuffer:1444, destroyOptimizedBuffer:1472, setCursorPosition:1493, setCursorColor:1604, getCursorState:1667, clearTerminal:1708, setTerminalTitle:1713, bufferClear:1740, bufferWriteResolvedChars:1791, bufferDrawText:1800, bufferSetCell:1817, bufferFillRect:1827, bufferDrawGrid:1990, bufferDrawBox:2015, bufferResize:2064, resizeRenderer:2069, enableMouse:2134, disableMouse:2139, queryPixelResolution:2144, enableKittyKeyboard:2154, disableKittyKeyboard:2159, setupTerminal:2174, suspendRenderer:2179, resumeRenderer:2184, createTextBuffer:2208…textBufferGetPlainText:2344, editBufferSetText:2771…editBufferReplaceText:2782.
- ABI facts: `NativeHandle = handles.Handle = u32` (0=invalid); Zig `bool`=1B; `render() -> u8` = `renderer.RenderStatus` (renderer.zig:29-33: rendered=0, skipped=1, failed=2); nullable `?[*]` → nullable raw ptr; `color: [*]const u16` = RGBA [u16;4] (rgbaBuffer, zig.ts:400); bufferDrawText fg non-null + bg nullable (lib.zig:1800); createRenderer TS args (u32,u32,u8,u8,ptr)→u32, dest 0=stdout/1=memory, remote 0=auto/1=local/2=remote (renderer.ts:3766-3783, renderer.rs DEST_*/REMOTE_*).

## Observed scenario
- renderer.rs declares `setTerminalTitle(titlePtr: *const c_uchar…)` — Zig takes `?[*]const u8` (nullable); null iff len 0. zigdecl documents nullability per-param.
- TS `buffer` type in args = borrowed TypedArray ptr (no len — len passed separately or fixed RGBA4). TS `ptr` = raw pointer/out-buffer.

## Target boundary
- zigdecl.rs: standalone (`rustc --edition 2021 --test`, no crate deps, no `crate::` imports). `#[cfg(feature="native")]` gates `#[link]` extern blocks so test binary links without libopentui. Every extern block gets SAFETY comment.
- Contents: DECL_COVERAGE table (comment) + `ALL_SYMBOLS: &[&str]` (337, domain-grouped) + domain counts + `ZIGDECL_TOTAL`; audited exact extern decls for renderer+buffer+text core (~40); safe pure helpers = required markers (create_renderer, render_native, setup_terminal, destroy_renderer, resize_renderer); option packing + BorderSides/TitleAlign mirrors; status/error enum with SKIPPED/FAILED.
- Do NOT port Zig logic; declare+document.

## Tests (frozen after RED)
- option packing matches renderer.rs/buffer.rs values; handle widths; coverage self-consistency (sum domains == ALL_SYMBOLS.len() == 337); status enum incl SKIPPED=1/FAILED=2; marker validation; dest/remote consts. ≥6 tests, ≥100 lines.

## Decisions
- Full 337 as data inventory (names+domains), exact `extern C` signatures only for audited core; remaining domains documented coarse (TS bun:ffi arity) with re-audit note — avoids inventing signatures.
- render_native maps status byte → NativeRenderStatus enum (rendered/skipped/failed), unknown → failed (fail-closed, matches handle.rs render_status_is_known).

## Remaining unknowns
- None blocking. Yoga/stream TS-only symbols out of scope (not lib.zig exports).

## GREEN (ses_brg_zigdecl2, 2026-09-23)
- Claimed via completion_claims (in-progress, self session).
- RED recorded: 58x E0425/E0433 (all items undeclared); frozen tests sha256 c6591c49...136c16d; impl prepended, tests untouched (sha match True post-GREEN).
- GREEN: `rustc --edition 2021 --test crates/opentui-bridge/src/zigdecl.rs -o /tmp/opencode/brg_zigdecl_test && /tmp/opencode/brg_zigdecl_test` — 7/7 pass, zero warnings.
- Contents (820 lines): DECL_COVERAGE table, ALL_SYMBOLS 337 sorted unique, 13 domain consts + ZIGDECL_TOTAL, NativeHandle/INVALID_HANDLE, DEST_*/REMOTE_*, status consts + NativeRenderStatus::from_u8 fail-closed, DeclError, BorderSides/TitleAlign/pack_options (lib.zig:2032-2041 layout), 5 markers (create_renderer/render_native/setup_terminal/destroy_renderer/resize_renderer), 3 audited cfg(feature="native") #[link] extern blocks (renderer 27 fns, buffer 10, text/edit 5) with SAFETY comments. Note: render_native is `pub fn` (const fn with match on param unsupported as const in this shape — kept non-const; `#[must_use]` retained).
