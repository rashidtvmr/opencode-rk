//! Safe ownership wrapper over the native renderer handle.
//!
//! Exactly one live [`Renderer`] at a time, enforced by `CLAIMED`.
//! FFI signatures mirror `renderer.rs` / `buffer.rs`, which mirror
//! `packages/native/src/lib.zig` (`NativeHandle = handles.Handle = u32`,
//! 0 = invalid). The `extern` block below re-declares only the symbols this
//! wrapper calls (duplicate declarations across modules resolve to the same
//! link symbol); `renderer.rs` keeps the audited full-surface declarations.
//! No `#![forbid(unsafe_code)]` anywhere in this crate (verified: neither
//! `lib.rs` nor the workspace root sets it), so the `unsafe {}` call sites
//! below compile.

use std::error::Error;
use std::fmt;
use std::marker::PhantomData;
#[cfg(feature = "native")]
use std::os::raw::c_void;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::buffer::NativeHandle;
use crate::color::Rgba;

/// Invalid/sentinel handle returned on failure.
const INVALID_HANDLE: NativeHandle = 0;

/// Global single-owner claim. Held while a [`Renderer`] is live.
static CLAIMED: AtomicBool = AtomicBool::new(false);

/// Byte cap for one [`Renderer::draw_text`] call.
pub const MAX_TEXT_BYTES: usize = 64 * 1024;
/// [`Renderer::snapshot_text`] buffer bounds.
#[cfg(feature = "native")]
const SNAP_MIN: usize = 64 * 1024;
#[cfg(feature = "native")]
const SNAP_MAX: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeError {
    AlreadyLive,
    CreateFailed,
    InvalidHandle,
    RenderFailed,
    ZeroSize,
    TextTooLarge,
    TitleNul,
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::AlreadyLive => "renderer already live",
            Self::CreateFailed => "renderer creation failed",
            Self::InvalidHandle => "invalid renderer handle",
            Self::RenderFailed => "render failed",
            Self::ZeroSize => "zero-size renderer",
            Self::TextTooLarge => "text exceeds size limit",
            Self::TitleNul => "title contains NUL byte",
        };
        f.write_str(s)
    }
}

impl Error for BridgeError {}

impl BridgeError {
    /// Classify a native status code: 0 ok, 1 invalid handle, else render failed.
    pub fn classify(code: i32) -> Result<(), Self> {
        match code {
            0 => Ok(()),
            1 => Err(Self::InvalidHandle),
            _ => Err(Self::RenderFailed),
        }
    }
}

/// Owned native renderer. `!Send + !Sync` via the raw-pointer marker.
#[derive(Debug)]
pub struct Renderer {
    handle: NativeHandle,
    cols: u32,
    rows: u32,
    _no_send: PhantomData<*const ()>,
}

#[cfg(feature = "native")]
#[link(name = "opentui")]
extern "C" {
    fn createRenderer(
        width: u32,
        height: u32,
        bufferedDestinationKind: u8,
        remoteModeValue: u8,
        feedPtr: *const c_void,
    ) -> NativeHandle;
    fn destroyRenderer(renderer_handle: NativeHandle, flush_input: bool);
    fn setupTerminal(renderer_handle: NativeHandle, useAlternateScreen: bool);
    fn resizeRenderer(renderer_handle: NativeHandle, width: u32, height: u32);
    fn setCursorPosition(renderer_handle: NativeHandle, x: i32, y: i32, visible: bool);
    fn setTerminalTitle(renderer_handle: NativeHandle, titlePtr: *const u8, titleLen: u32);
    fn getCurrentBuffer(renderer_handle: NativeHandle) -> NativeHandle;
    fn render(renderer_handle: NativeHandle, force: bool) -> u8;
    fn bufferDrawText(
        buffer_handle: NativeHandle,
        text: *const u8,
        textLen: u32,
        x: u32,
        y: u32,
        fg: *const u16,
        bg: *const u16,
        attributes: u32,
    );
    fn bufferFillRect(
        buffer_handle: NativeHandle,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        bg: *const u16,
    );
    fn bufferWriteResolvedChars(
        buffer_handle: NativeHandle,
        outputPtr: *mut u8,
        outputLen: u32,
        addLineBreaks: bool,
    ) -> u32;
}

impl Renderer {
    fn create_inner(cols: u32, rows: u32, dest: u8) -> Result<Self, BridgeError> {
        if cols == 0 || rows == 0 {
            return Err(BridgeError::ZeroSize);
        }
        if CLAIMED.swap(true, Ordering::AcqRel) {
            return Err(BridgeError::AlreadyLive);
        }
        #[cfg(feature = "native")]
        {
            // SAFETY: plain integers + null feed ptr (buffered backend);
            // handle checked below, destroyed in `release`.
            let handle =
                unsafe { createRenderer(cols, rows, dest, 0, std::ptr::null()) };
            if handle == INVALID_HANDLE {
                CLAIMED.store(false, Ordering::Release);
                return Err(BridgeError::CreateFailed);
            }
            Ok(Self {
                handle,
                cols,
                rows,
                _no_send: PhantomData,
            })
        }
        #[cfg(not(feature = "native"))]
        {
            let _ = dest;
            CLAIMED.store(false, Ordering::Release);
            Err(BridgeError::CreateFailed)
        }
    }

    /// Create a renderer targeting stdout (`DEST_STDOUT`).
    pub fn create(cols: u32, rows: u32) -> Result<Self, BridgeError> {
        Self::create_inner(cols, rows, 0)
    }

    /// Create a memory-backed renderer (headless snapshots, `DEST_MEMORY`).
    pub fn create_memory(cols: u32, rows: u32) -> Result<Self, BridgeError> {
        Self::create_inner(cols, rows, 1)
    }

    fn live(&self) -> Result<NativeHandle, BridgeError> {
        if self.handle == INVALID_HANDLE {
            return Err(BridgeError::InvalidHandle);
        }
        Ok(self.handle)
    }

    fn release(&mut self) {
        if self.handle == INVALID_HANDLE {
            return;
        }
        #[cfg(feature = "native")]
        // SAFETY: live handle owned by self; zeroed below so destroy runs once.
        unsafe {
            destroyRenderer(self.handle, true);
        }
        self.handle = INVALID_HANDLE;
        CLAIMED.store(false, Ordering::Release);
    }

    /// Destroy early and release the claim; further calls fail `InvalidHandle`.
    pub fn close(&mut self) {
        self.release();
    }

    pub fn cols(&self) -> u32 {
        self.cols
    }

    pub fn rows(&self) -> u32 {
        self.rows
    }

    pub fn setup_terminal(&self) -> Result<(), BridgeError> {
        let handle = self.live()?;
        #[cfg(not(feature = "native"))]
        {
            let _ = handle;
            return Err(BridgeError::InvalidHandle);
        }
        #[cfg(feature = "native")]
        {
            // SAFETY: live handle; plain integers only.
            unsafe { setupTerminal(handle, true) };
            Ok(())
        }
    }

    pub fn resize(&mut self, cols: u32, rows: u32) -> Result<(), BridgeError> {
        if cols == 0 || rows == 0 {
            return Err(BridgeError::ZeroSize);
        }
        let handle = self.live()?;
        #[cfg(not(feature = "native"))]
        {
            let _ = handle;
            return Err(BridgeError::InvalidHandle);
        }
        #[cfg(feature = "native")]
        {
            // SAFETY: live handle; plain integers only.
            unsafe { resizeRenderer(handle, cols, rows) };
            self.cols = cols;
            self.rows = rows;
            Ok(())
        }
    }

    /// Run `paint` on the current buffer, then present it.
    pub fn frame(&mut self, paint: impl FnOnce(NativeHandle)) -> Result<(), BridgeError> {
        let handle = self.live()?;
        #[cfg(not(feature = "native"))]
        {
            let _ = handle;
            let _ = paint;
            return Err(BridgeError::InvalidHandle);
        }
        #[cfg(feature = "native")]
        {
            // SAFETY: live handle; returned buffer valid for the frame.
            let buf = unsafe { getCurrentBuffer(handle) };
            if buf == INVALID_HANDLE {
                return Err(BridgeError::RenderFailed);
            }
            paint(buf);
            // SAFETY: live handle; `render` takes no pointers.
            let status = unsafe { render(handle, true) };
            BridgeError::classify(i32::from(status))
        }
    }

    pub fn draw_text(&self, x: u32, y: u32, text: &str) -> Result<(), BridgeError> {
        if text.len() > MAX_TEXT_BYTES {
            return Err(BridgeError::TextTooLarge);
        }
        if x >= self.cols || y >= self.rows {
            return Err(BridgeError::RenderFailed);
        }
        let handle = self.live()?;
        #[cfg(not(feature = "native"))]
        {
            let _ = handle;
            return Err(BridgeError::InvalidHandle);
        }
        #[cfg(feature = "native")]
        {
            // SAFETY: live handle; `text` outlives the call; null fg/bg
            // selects native defaults (nullable `?[*]u16`).
            unsafe {
                let buf = getCurrentBuffer(handle);
                if buf == INVALID_HANDLE {
                    return Err(BridgeError::RenderFailed);
                }
                bufferDrawText(
                    buf,
                    text.as_ptr(),
                    text.len().min(u32::MAX as usize) as u32,
                    x,
                    y,
                    std::ptr::null(),
                    std::ptr::null(),
                    0,
                );
            }
            Ok(())
        }
    }

    pub fn fill_rect(
        &self,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        color: Rgba,
    ) -> Result<(), BridgeError> {
        if x.saturating_add(w) > self.cols || y.saturating_add(h) > self.rows {
            return Err(BridgeError::RenderFailed);
        }
        let handle = self.live()?;
        #[cfg(not(feature = "native"))]
        {
            let _ = (handle, color);
            return Err(BridgeError::InvalidHandle);
        }
        #[cfg(feature = "native")]
        {
            let bg: [u16; 4] = [
                u16::from(color.r),
                u16::from(color.g),
                u16::from(color.b),
                u16::from(color.a),
            ];
            // SAFETY: live handle; `bg` outlives the call.
            unsafe {
                let buf = getCurrentBuffer(handle);
                if buf == INVALID_HANDLE {
                    return Err(BridgeError::RenderFailed);
                }
                bufferFillRect(buf, x, y, w, h, bg.as_ptr());
            }
            Ok(())
        }
    }

    pub fn set_cursor(&self, x: u32, y: u32, visible: bool) -> Result<(), BridgeError> {
        if x >= self.cols || y >= self.rows {
            return Err(BridgeError::RenderFailed);
        }
        let handle = self.live()?;
        #[cfg(not(feature = "native"))]
        {
            let _ = (handle, visible);
            return Err(BridgeError::InvalidHandle);
        }
        #[cfg(feature = "native")]
        {
            // SAFETY: live handle; plain integers only.
            unsafe { setCursorPosition(handle, x as i32, y as i32, visible) };
            Ok(())
        }
    }

    pub fn set_title(&self, title: &str) -> Result<(), BridgeError> {
        if title.as_bytes().contains(&0) {
            return Err(BridgeError::TitleNul);
        }
        let handle = self.live()?;
        #[cfg(not(feature = "native"))]
        {
            let _ = handle;
            return Err(BridgeError::InvalidHandle);
        }
        #[cfg(feature = "native")]
        {
            // SAFETY: live handle; `title` outlives the call.
            unsafe { setTerminalTitle(handle, title.as_ptr(), title.len() as u32) };
            Ok(())
        }
    }

    /// Read back resolved chars, growing the buffer 64K → 1M on truncation.
    pub fn snapshot_text(&self) -> Result<String, BridgeError> {
        let handle = self.live()?;
        #[cfg(not(feature = "native"))]
        {
            let _ = handle;
            return Err(BridgeError::InvalidHandle);
        }
        #[cfg(feature = "native")]
        {
            // SAFETY: live handle; returned buffer handle valid for reads.
            let buf = unsafe { getCurrentBuffer(handle) };
            if buf == INVALID_HANDLE {
                return Err(BridgeError::RenderFailed);
            }
            let mut cap = SNAP_MIN;
            loop {
                let mut out = vec![0u8; cap];
                // SAFETY: `out` sized `cap`; native writes ≤ `outputLen` bytes.
                let n = unsafe {
                    bufferWriteResolvedChars(buf, out.as_mut_ptr(), cap as u32, true)
                } as usize;
                let n = n.min(cap);
                if n < cap || cap >= SNAP_MAX {
                    out.truncate(n);
                    return String::from_utf8(out).map_err(|_| BridgeError::RenderFailed);
                }
                cap = (cap * 2).min(SNAP_MAX);
            }
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Test-only live renderer with a fake nonzero handle; holds `CLAIMED`.
    /// (Drop on non-`native` builds only clears the claim; no FFI runs.)
    fn fake(cols: u32, rows: u32) -> Renderer {
        assert!(!CLAIMED.swap(true, Ordering::AcqRel));
        Renderer {
            handle: 0xC10C,
            cols,
            rows,
            _no_send: PhantomData,
        }
    }

    #[test]
    fn zero_size_rejected() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(Renderer::create(0, 24).unwrap_err(), BridgeError::ZeroSize);
        assert_eq!(Renderer::create(80, 0).unwrap_err(), BridgeError::ZeroSize);
        assert_eq!(
            Renderer::create_memory(0, 0).unwrap_err(),
            BridgeError::ZeroSize
        );
    }

    #[test]
    fn double_claim_rejected() {
        let _guard = TEST_LOCK.lock().unwrap();
        let _held = fake(80, 24);
        assert_eq!(
            Renderer::create(80, 24).unwrap_err(),
            BridgeError::AlreadyLive
        );
    }

    #[test]
    fn closed_handle_rejected() {
        let _guard = TEST_LOCK.lock().unwrap();
        let mut r = fake(80, 24);
        r.close();
        assert_eq!(
            r.draw_text(0, 0, "hi").unwrap_err(),
            BridgeError::InvalidHandle
        );
        assert_eq!(
            r.set_cursor(0, 0, true).unwrap_err(),
            BridgeError::InvalidHandle
        );
        assert_eq!(r.snapshot_text().unwrap_err(), BridgeError::InvalidHandle);
    }

    #[test]
    fn title_nul_rejected_before_handle_check() {
        let _guard = TEST_LOCK.lock().unwrap();
        let mut closed = fake(10, 10);
        closed.close();
        assert_eq!(closed.set_title("a\0b").unwrap_err(), BridgeError::TitleNul);
    }

    #[test]
    fn oversized_text_rejected() {
        let _guard = TEST_LOCK.lock().unwrap();
        let r = fake(80, 24);
        let big = "x".repeat(MAX_TEXT_BYTES + 1);
        assert_eq!(
            r.draw_text(0, 0, &big).unwrap_err(),
            BridgeError::TextTooLarge
        );
    }

    #[test]
    fn status_classify() {
        assert_eq!(BridgeError::classify(0), Ok(()));
        assert_eq!(BridgeError::classify(1), Err(BridgeError::InvalidHandle));
        assert_eq!(BridgeError::classify(7), Err(BridgeError::RenderFailed));
        for e in [
            BridgeError::AlreadyLive,
            BridgeError::CreateFailed,
            BridgeError::InvalidHandle,
            BridgeError::RenderFailed,
            BridgeError::ZeroSize,
            BridgeError::TextTooLarge,
            BridgeError::TitleNul,
        ] {
            assert!(!e.to_string().is_empty());
            let _: &dyn Error = &e;
        }
    }
}
