//! Text/edit buffer FFI surface.
//!
//! Signatures mirror `packages/native/src/lib.zig` (branch `rust-bridge`),
//! as captured in the ABI extract. Only exports present in that extract are
//! declared here — nothing is invented:
//! `createEditBuffer`, `destroyEditBuffer`, `editBufferInsertText`,
//! `editBufferGetText`, and any `syntaxStyleRegister` do NOT appear in the
//! extract, so they are intentionally absent.

/// Opaque native handle (`NativeHandle = handles.Handle`; 0 = invalid).
pub type NativeHandle = u32;

/// Invalid/sentinel handle returned on failure.
pub const INVALID_HANDLE: NativeHandle = 0;

/// Opaque styled-text chunk (`text_buffer.StyledChunk`).
/// Layout is native-owned; only passed by pointer, never constructed here.
#[repr(C)]
pub struct StyledChunk {
    _opaque: [u8; 0],
}

#[cfg(feature = "native")]
unsafe extern "C" {
    pub fn createTextBuffer(width_method: u8) -> NativeHandle;
    pub fn destroyTextBuffer(tb_handle: NativeHandle);
    pub fn textBufferGetLength(tb_handle: NativeHandle) -> u32;
    pub fn textBufferGetByteSize(tb_handle: NativeHandle) -> u32;
    pub fn textBufferReset(tb_handle: NativeHandle);
    pub fn textBufferClear(tb_handle: NativeHandle);
    pub fn textBufferSetDefaultFg(tb_handle: NativeHandle, fg: *const u16);
    pub fn textBufferSetDefaultBg(tb_handle: NativeHandle, bg: *const u16);
    pub fn textBufferSetTextFromMem(tb_handle: NativeHandle, id: u8);
    pub fn textBufferAppend(
        tb_handle: NativeHandle,
        data_ptr: *const u8,
        data_len: u32,
    );
    pub fn textBufferAppendFromMemId(tb_handle: NativeHandle, id: u8);
    pub fn textBufferLoadFile(
        tb_handle: NativeHandle,
        path_ptr: *const u8,
        path_len: u32,
    ) -> bool;
    pub fn textBufferSetStyledText(
        tb_handle: NativeHandle,
        chunks_ptr: *const StyledChunk,
        chunk_count: u32,
    );
    pub fn textBufferGetLineCount(tb_handle: NativeHandle) -> u32;
    pub fn textBufferGetPlainText(
        tb_handle: NativeHandle,
        out_ptr: *mut u8,
        max_len: u32,
    ) -> u32;
    pub fn editBufferSetText(
        edit_handle: NativeHandle,
        text_ptr: *const u8,
        text_len: u32,
    );
    pub fn editBufferSetTextFromMem(edit_handle: NativeHandle, mem_id: u8);
    pub fn editBufferReplaceText(
        edit_handle: NativeHandle,
        text_ptr: *const u8,
        text_len: u32,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn native_handle_layout_stable() {
        assert_eq!(size_of::<NativeHandle>(), 4);
        assert_eq!(INVALID_HANDLE, 0);
    }
}
