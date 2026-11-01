//! Renderer FFI boundary: audited `unsafe extern "C"` declarations only.
//!
//! Upstream: `packages/native/src/lib.zig` (`NativeHandle = handles.Handle = u32`).
//! Zig `bool` maps to Rust `bool` (both 1 byte, 0/1). Nullable Zig pointers map
//! to nullable raw pointers (caller may pass null). Safe items: consts, structs.

#[cfg(feature = "native")]
use crate::buffer::NativeHandle;
#[cfg(feature = "native")]
use std::os::raw::{c_uchar, c_uint, c_void};

/// `bufferedDestinationKind`: process stdout.
pub const DEST_STDOUT: u8 = 0;
/// `bufferedDestinationKind`: in-memory output.
pub const DEST_MEMORY: u8 = 1;

/// `remoteModeValue`: auto-detect.
pub const REMOTE_AUTO: u8 = 0;
/// `remoteModeValue`: force local.
pub const REMOTE_LOCAL: u8 = 1;
/// `remoteModeValue`: force remote.
pub const REMOTE_REMOTE: u8 = 2;

/// Renderer spec placeholder (no unsafe yet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererSpec {
    pub width: u32,
    pub height: u32,
}

impl RendererSpec {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// Mirrors Zig `ExternalBuildOptions` (`extern struct` of two `bool`s).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalBuildOptions {
    pub gpa_safe_stats: bool,
    pub gpa_memory_limit_tracking: bool,
}

/// Mirrors Zig `CursorStyleOptions` (`packages/native/src/lib.zig:1630-1635`).
///
/// `style`/`blinking`/`cursor` use the OpenTUI sentinel `255` for "leave the
/// current value unchanged" (`terminal.zig` `setCursorStyleOptions` clamps via
/// `<= 3` / `<= 1` / `< tags.len()`). `color` is a nullable pointer to 4
/// `u16` RGBA lanes; null leaves the current cursor colour. Field order and
/// the trailing pointer match the Zig `extern struct` layout exactly
/// (`zig-structs.ts:218-232` `CursorStyleOptionsStruct`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorStyleOptionsAbi {
    pub style: u8,
    pub blinking: u8,
    /// Nullable 4×`u16` RGBA lanes; layout is pointer-aligned after the two
    /// leading bytes.
    pub color: *const u16,
    pub cursor: u8,
}

#[cfg(feature = "native")]
#[link(name = "opentui")]
#[expect(
    dead_code,
    reason = "ABI inventory is retained while live native calls use safe_renderer bindings"
)]
unsafe extern "C" {
    /// Safety: handle must be a live renderer; null feed_ptr selects buffered backend.
    fn createRenderer(
        width: c_uint,
        height: c_uint,
        bufferedDestinationKind: c_uchar,
        remoteModeValue: c_uchar,
        feedPtr: *const c_void,
    ) -> NativeHandle;
    /// Safety: handle must be a live owned renderer; must not be used afterwards.
    fn destroyRenderer(renderer_handle: NativeHandle, flush_input: bool);
    /// Safety: handle must be a live renderer.
    fn setupTerminal(renderer_handle: NativeHandle, useAlternateScreen: bool);
    /// Safety: handle must be a live renderer.
    fn suspendRenderer(renderer_handle: NativeHandle);
    /// Safety: handle must be a live renderer.
    fn resumeRenderer(renderer_handle: NativeHandle);
    /// Safety: handle must be a live renderer.
    fn restoreTerminalModes(renderer_handle: NativeHandle);
    /// Safety: handle must be a live renderer.
    fn resizeRenderer(renderer_handle: NativeHandle, width: c_uint, height: c_uint);
    /// Safety: handle must be a live renderer.
    fn setUseThread(renderer_handle: NativeHandle, useThread: bool);
    /// Safety: handle must be a live renderer.
    fn setCursorPosition(renderer_handle: NativeHandle, x: i32, y: i32, visible: bool);
    /// Safety: handle must be a live renderer; titlePtr null iff titleLen is 0.
    fn setTerminalTitle(renderer_handle: NativeHandle, titlePtr: *const c_uchar, titleLen: c_uint);
    /// Safety: handle must be a live renderer.
    fn getNextBuffer(renderer_handle: NativeHandle) -> NativeHandle;
    /// Safety: handle must be a live renderer.
    fn getCurrentBuffer(renderer_handle: NativeHandle) -> NativeHandle;
    /// Safety: handle must be a live buffer.
    fn getBufferWidth(buffer_handle: NativeHandle) -> c_uint;
    /// Safety: handle must be a live buffer.
    fn getBufferHeight(buffer_handle: NativeHandle) -> c_uint;
    /// Safety: handle must be a live renderer.
    fn render(renderer_handle: NativeHandle, force: bool) -> c_uchar;
    /// Safety: handle must be a live renderer; color must point to 4 valid u16s.
    fn setBackgroundColor(renderer_handle: NativeHandle, color: *const u16);
    /// Safety: handle must be a live renderer.
    fn enableMouse(renderer_handle: NativeHandle, enableMovement: bool);
    /// Safety: handle must be a live renderer.
    fn disableMouse(renderer_handle: NativeHandle);
    /// Safety: handle must be a live renderer.
    fn enableKittyKeyboard(renderer_handle: NativeHandle, flags: c_uchar);
    /// Safety: handle must be a live renderer.
    fn disableKittyKeyboard(renderer_handle: NativeHandle);
    /// Safety: handle must be a live renderer.
    fn queryPixelResolution(renderer_handle: NativeHandle);
    /// Safety: handle must be a live renderer; color must point to 4 valid u16s.
    fn setCursorColor(renderer_handle: NativeHandle, color: *const u16);
    /// Safety: handle must be a live renderer; `options` must point to one
    /// valid [`CursorStyleOptionsAbi`] whose optional `color` pointer, when
    /// non-null, points to 4 valid u16s.
    fn setCursorStyleOptions(renderer_handle: NativeHandle, options: *const CursorStyleOptionsAbi);
    /// Safety: handle must be a live renderer.
    fn clearTerminal(renderer_handle: NativeHandle);
    /// Safety: out_ptr must be valid for writes of one ExternalBuildOptions.
    fn getBuildOptions(out_ptr: *mut ExternalBuildOptions);
    /// Safety: handle must be a live renderer; ptrs null iff matching len is 0.
    fn setTerminalEnvVar(
        renderer_handle: NativeHandle,
        keyPtr: *const c_uchar,
        keyLen: c_uint,
        valuePtr: *const c_uchar,
        valueLen: c_uint,
    ) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn renderer_spec_size_stable() {
        assert_eq!(size_of::<RendererSpec>(), 8);
        assert_eq!(RendererSpec::new(80, 24).width, 80);
    }

    #[test]
    fn abi_consts_match_zig_defaults() {
        assert_eq!(DEST_STDOUT, 0);
        assert_eq!(DEST_MEMORY, 1);
        assert_eq!(REMOTE_AUTO, 0);
        assert_eq!(REMOTE_LOCAL, 1);
        assert_eq!(REMOTE_REMOTE, 2);
        assert_eq!(size_of::<crate::buffer::NativeHandle>(), 4);
    }

    #[test]
    fn build_options_layout_matches_extern_struct() {
        assert_eq!(size_of::<ExternalBuildOptions>(), 2);
        assert_eq!(size_of::<bool>(), 1);
    }

    /// THEME-14: the cursor-style ABI struct must match Zig's `extern struct`
    /// layout (`lib.zig:1630-1635`): two leading `u8`s, a pointer-aligned
    /// nullable colour pointer, then a trailing `u8`. On a 64-bit target this
    /// is 24 bytes with the pointer at offset 8 and `cursor` at offset 16.
    #[test]
    fn cursor_style_options_layout_matches_extern_struct() {
        assert_eq!(size_of::<CursorStyleOptionsAbi>(), 24);
        assert_eq!(std::mem::offset_of!(CursorStyleOptionsAbi, style), 0);
        assert_eq!(std::mem::offset_of!(CursorStyleOptionsAbi, blinking), 1);
        assert_eq!(std::mem::offset_of!(CursorStyleOptionsAbi, color), 8);
        assert_eq!(std::mem::offset_of!(CursorStyleOptionsAbi, cursor), 16);
        let options = CursorStyleOptionsAbi {
            style: 0,
            blinking: 1,
            color: std::ptr::null(),
            cursor: 255,
        };
        assert!(options.color.is_null());
    }
}
