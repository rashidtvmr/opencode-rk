# TUI-001 ABI re-verification

Status: blocked verifier; no product/test edits.

## Identity and frozen inputs

- Worktree: `/Users/mymac/Projects/opencode-rk-web006-integrate`
- Candidate before evidence: `509c4529eb78367fc3868be44b1d42f5ef8a61c2`
- Pinned source: `c01292fd0837bafd07ce458c74416b2b375a41ab`
- Artifact: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/opentui-pinned/packages/native/lib/aarch64-macos/libopentui.dylib`
- Artifact SHA-256: `de3564d496a92fe48fd6ca24a975b1b886cbb654ad5b7eea15bfe81afbf80b50`
- Artifact type: Mach-O 64-bit dynamically linked shared library arm64
- Frozen Rust test source SHA-256: `b174441106759f213a323268179c78302a1589ef9c4ca815e06d2d9a7cbe6034`
- `git diff -- crates/opentui-bridge/src/safe_renderer.rs` empty; no test edits. Working diff initially contained only the claim-ledger update plus pre-existing untracked disposable dylib symlink.

## Exact RED

Command (single build/test worker, one test thread):

```text
DYLD_LIBRARY_PATH=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/opentui-pinned/packages/native/lib/aarch64-macos CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-opentui-bridge --features native --lib safe_renderer::tests::render_once_content_present -- --exact --test-threads=1
```

Build completed. Test terminated with signal 11, `SIGSEGV: invalid memory reference`; exit 101. No parallel workload. A bounded LLDB attempt was inconclusive (pending symbol breakpoints; timed out), so root cause rests on static ABI evidence plus reproducible RED, not debugger inference.

## Call-path ABI comparison

Rust call path: `safe_renderer.rs:561-570` create memory renderer, `:568` draw; `:404-438` obtains current buffer then calls `bufferDrawText`; `:515-543` obtains current buffer then calls `bufferWriteResolvedChars`; Drop `:574-577` calls `release`, whose destroy is `:186-201`.

Pinned Zig `lib.zig`:

- `createRenderer`: `:1154-1208`; `(u32,u32,u8,u8,?*native_span_feed.Stream) NativeHandle`. Rust `safe_renderer.rs:88-94` has same machine widths; null feed is valid. Memory destination `1` is correct.
- `getCurrentBuffer`: `:1342-1345`; `(NativeHandle) NativeHandle`. Rust `:108` matches.
- `bufferDrawText`: `:1800-1810`; `(NativeHandle, ?[*]const u8, u32, u32, u32, [*]const u16, ?[*]const u16, u32) void`. Rust `:110-119` incorrectly models `fg` as nullable at the call site: `safe_renderer.rs:432` passes `std::ptr::null()`.
- `bufferWriteResolvedChars`: `:1791-1798`; `(NativeHandle, ?[*]u8, u32, bool) u32`. Rust `:128-133` matches; snapshot output is allocated and non-null.
- `destroyRenderer`: `:1227-1233`; `(NativeHandle,bool) void`. Rust `:95` matches.

First invalid operation: `safe_renderer.rs:432` passes null foreground RGBA to `bufferDrawText`. Zig `lib.zig:1806` immediately calls `ptrToRGBA(fg)`; `ptrToRGBA` at `:107-109` unconditionally reads `color[0..3]`. `fg` is therefore not nullable. The nearby Rust comment (`:419-420`) claiming null foreground selects native defaults is false. `bg` is genuinely optional: Zig `:1807` calls `optionalPtrToRGBA(bg)`, `:111-117`; preserving null background is valid. The SIGSEGV is consequently localized to the draw call's null `fg`, before snapshot or drop. No claim made about later operations beyond matching signatures.

## Export verification

Filtered command:

```text
nm -gU /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/opentui-pinned/packages/native/lib/aarch64-macos/libopentui.dylib | grep -E '(_)?(createRenderer|getCurrentBuffer|bufferDrawText|bufferWriteResolvedChars|destroyRenderer)$'
```

Observed exports: `_createRenderer`, `_getCurrentBuffer`, `_bufferDrawText`, `_bufferWriteResolvedChars`, `_destroyRenderer`.

## Minimal later GREEN contract

One-file repair in `crates/opentui-bridge/src/safe_renderer.rs` only: retain a local `[u16; 4]` foreground RGBA array alive through each `bufferDrawText` call; pass its non-null pointer; retain `bg == null` unless an explicit background is requested. Correct channels are four `u16` lanes in RGBA order, per pinned `ansi.zig:20`, `lib.zig:107-109`, `:1806-1807`; a deterministic opaque default (for example `[0,0,0,65535]` or caller-established native default) must be selected from the approved caller contract before implementation. Do not expand unsafe lifetimes, alter tests, or replace the real create/draw/snapshot/drop journey.

Proposed GREEN command is the exact RED command above, unchanged, after the repair; expected `render_once_content_present` passes and snapshot contains `hello` and `world`. Then run the focused bridge native library tests with the same bounded settings.

## Remaining uncertainty

Debugger localization was not obtained due to LLDB timeout. Static source evidence is sufficient for the nullability violation; channel default choice remains for the implementation owner to resolve from caller semantics. Resource bounds: one Cargo job, one test thread, one test target; no broad suite run.
