# BRG-FFI — portable FFI surface mirror

## Claim
- Task: BRG-FFI, session ses_brg_ffi, owned file ONLY: crates/opentui-bridge/src/ffi.rs
- Ledger: claimed in-progress via completion_claims.cc.claim.

## Source evidence
- `/Users/mymac/Projects/opentui/packages/core/src/platform/ffi.ts` (full file, 788 lines):
  - `Pointer` = branded `number | bigint` (Bun numeric, Node bigint); normalized at backend boundary only (`toPointer`, `toBunPointer`, `toBigIntPointer`).
  - `FFIType` const map, 33 tags: char/int8_t/i8/uint8_t/u8/int16_t/i16/uint16_t/u16/int32_t/i32/int/uint32_t/u32/int64_t/i64/uint64_t/u64/double/f64/float/f32/bool/ptr/pointer/void/cstring/function/usize/callback/napi_env/napi_value/buffer (lines 26-60).
  - `FFIFunction` { args?, returns?, ptr?, threadsafe? } (lines 68-73).
  - `FFICallbackInstance` { ptr: Pointer|null, threadsafe, close() clears ptr in finally } (lines 77-81, 274-305).
  - Node guards: `NODE_POINTER_OVERRIDE`, `NODE_CALLBACK_THREADSAFE`, `NODE_USIZE_UNSUPPORTED`, `NODE_NAPI_UNSUPPORTED`, `NODE_STRING_RETURN` (result cstring), `POINTER_NEGATIVE`/`POINTER_UNSAFE`/`POINTER_OFFSET_NEGATIVE`/`POINTER_OFFSET_UNSAFE` (lines 149-164).
  - `isNodePointerArgumentType`: ptr/pointer/function/callback only (line 606).
  - `toNodeFFIType`: cstring param→"string", function/callback/buffer→"pointer", usize/napi→throw (lines 685-767).
  - `LIBRARY_CLOSED` on createCallback after close (lines 346, 402).
- `zig.ts` lines ~6910-6944 (`grep setRenderLibPath|resolveRenderLib` hit 6914/6917):
  - `setRenderLibPath`: same-path no-op; different path after `renderLibResolved` → throw; disposes FFIRenderLib.
  - `resolveRenderLib`: lazy construct, wraps error as "Failed to initialize OpenTUI render library: …", sets resolved flag.
  - Eager-load tail: `try { new FFIRenderLib } catch {}` — failure swallowed.
- `zig.ts` path was NOT at `packages/core/src/platform/zig.ts` (task hint wrong); actual: `packages/core/src/zig.ts`.
- Sibling style (`handle.rs`): `#![forbid(unsafe_code)]`, single-owner no-Clone handle, const asserts, `#[cfg(test)]` with real assertions.

## Target boundary
- `ffi.rs` self-contained (standalone `rustc --test`, no sibling imports), std-only, `forbid(unsafe_code)`.
- Gate markers: `pub enum FfiType`, `pub struct FfiFunction`, `Pointer`, `pub struct CallbackHandle`; ≥60 lines, ≥5 tests, no todo!/unimplemented!.
- Hot path: const-fn tags + pointer checks, borrowed static arg slices, no heap.

## Tests (frozen, RED-first)
- Command: `rustc --edition 2021 --test crates/opentui-bridge/src/ffi.rs -o /tmp/opencode/brg_ffi_test && /tmp/opencode/brg_ffi_test`
- RED v1: `MAX_SAFE_POINTER = (1<<53)` (off-by-one) → `max_safe_pointer_value`, `pointer_rejects_unsafe_integer` fail.
- GREEN fix (impl only): const → `(1<<53)-1`.

## Decisions
- `Pointer(u64)` newtype; negativity via `from_i64`; safe-integer ceiling mirrors JS boundary.
- `FfiFunction.args: Option<&'static [FfiType]>`, returns defaults Void (mirrors `definition.returns ?? void`).
- `RenderLibConfig` + `RenderLibError::AlreadyResolved/InitFailed` + `eager_resolve()` swallowing init failure.

## Unknowns
- None blocking. Native symbol table (`RenderLib`) is another lane's scope (renderer/handle).

## Result (GREEN)
- `rustc --edition 2021 --test crates/opentui-bridge/src/ffi.rs -o /tmp/opencode/brg_ffi_test && /tmp/opencode/brg_ffi_test` → 10 passed, 0 failed.
- Gate markers: `pub enum FfiType`, `pub struct FfiFunction`, `Pointer`, `pub struct CallbackHandle` all present; 517 lines; 10 tests; no todo!/unimplemented!.
- Ledger: `completed` set with test evidence note. No commit/push per task instruction (overrides WORKER.md §5).
