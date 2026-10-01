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
#[cfg(all(feature = "native", unix))]
use std::os::fd::{AsFd, OwnedFd};
#[cfg(feature = "native")]
use std::os::raw::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "native")]
use std::sync::atomic::{AtomicU32, AtomicU8};
#[cfg(all(feature = "native", unix))]
use std::sync::Mutex;
#[cfg(all(feature = "native", unix))]
use std::time::Duration;

#[cfg(all(feature = "native", unix))]
use rustix::event::{poll, PollFd, PollFlags, Timespec};
#[cfg(all(feature = "native", unix))]
use rustix::termios::{tcgetattr, tcgetwinsize, tcsetattr, OptionalActions, Termios};

use crate::buffer::NativeHandle;
use crate::color::Rgba;

/// Invalid/sentinel handle returned on failure.
const INVALID_HANDLE: NativeHandle = 0;

/// Global single-owner claim. Held while a [`Renderer`] is live.
static CLAIMED: AtomicBool = AtomicBool::new(false);

/// Native terminal/input state for the one renderer owned by this process.
/// `CLAIMED` permits only one live renderer, so one bounded atomic slot avoids
/// changing the frozen test-only struct literal or retaining a handle map.
#[cfg(feature = "native")]
static LIFECYCLE_HANDLE: AtomicU32 = AtomicU32::new(INVALID_HANDLE);
#[cfg(feature = "native")]
static LIFECYCLE_FLAGS: AtomicU8 = AtomicU8::new(0);
#[cfg(feature = "native")]
const TERMINAL_ACTIVE: u8 = 1;
#[cfg(feature = "native")]
const MOUSE_ENABLED: u8 = 2;
#[cfg(feature = "native")]
const KITTY_KEYBOARD_ENABLED: u8 = 4;

/// OpenTUI's native library owns ANSI modes, while its TypeScript wrapper
/// owns stdin raw mode. Retain the same input descriptor and exact attributes
/// here, including across suspend/resume. The single-renderer claim bounds
/// this slot to one terminal and prevents concurrent input-mode owners.
#[cfg(all(feature = "native", unix))]
static TERMINAL_INPUT: Mutex<Option<TerminalInput>> = Mutex::new(None);

#[cfg(all(feature = "native", unix))]
struct TerminalInput {
    handle: NativeHandle,
    fd: OwnedFd,
    original: Termios,
    raw: Termios,
    active: bool,
}

#[cfg(all(feature = "native", unix))]
fn activate_terminal_input(handle: NativeHandle) -> Result<(), BridgeError> {
    let mut slot = TERMINAL_INPUT
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if let Some(input) = slot.as_mut() {
        if input.handle != handle {
            return Err(BridgeError::TerminalFailed);
        }
        if !input.active {
            tcsetattr(&input.fd, OptionalActions::Now, &input.raw)
                .map_err(|_| BridgeError::TerminalFailed)?;
            input.active = true;
        }
        return Ok(());
    }

    // An owned CLOEXEC duplicate keeps restoration bound to the captured TTY
    // even if the caller subsequently replaces stdin. A non-TTY input has no
    // terminal attributes to own (e.g. memory/headless rendering).
    let fd = std::io::stdin()
        .as_fd()
        .try_clone_to_owned()
        .map_err(|_| BridgeError::TerminalFailed)?;
    let original = match tcgetattr(&fd) {
        Ok(attributes) => attributes,
        Err(rustix::io::Errno::NOTTY) => return Ok(()),
        Err(_) => return Err(BridgeError::TerminalFailed),
    };
    let mut raw = original.clone();
    raw.make_raw();
    tcsetattr(&fd, OptionalActions::Now, &raw).map_err(|_| BridgeError::TerminalFailed)?;
    *slot = Some(TerminalInput {
        handle,
        fd,
        original,
        raw,
        active: true,
    });
    Ok(())
}

#[cfg(all(feature = "native", unix))]
fn restore_terminal_input(handle: NativeHandle, release: bool) -> Result<(), BridgeError> {
    let mut slot = TERMINAL_INPUT
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if let Some(input) = slot.as_mut() {
        if input.handle != handle {
            return Err(BridgeError::TerminalFailed);
        }
        if input.active {
            tcsetattr(&input.fd, OptionalActions::Now, &input.original)
                .map_err(|_| BridgeError::TerminalFailed)?;
            #[cfg(target_os = "macos")]
            {
                use rustix::termios::{tcflush, LocalModes, QueueSelector};

                // Darwin marks input for reprocessing when switching back to
                // canonical mode. A nonblocking input flush clears this derived
                // PENDIN bit, matching native cleanup's flush_input contract.
                // Preserve a PENDIN bit that was already present on entry.
                if !input.original.local_modes.contains(LocalModes::PENDIN)
                    && tcgetattr(&input.fd)
                        .map_err(|_| BridgeError::TerminalFailed)?
                        .local_modes
                        .contains(LocalModes::PENDIN)
                {
                    tcflush(&input.fd, QueueSelector::IFlush)
                        .map_err(|_| BridgeError::TerminalFailed)?;
                }
            }
            input.active = false;
        }
        if release {
            *slot = None;
        }
    }
    Ok(())
}

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
    TerminalFailed,
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
            Self::TerminalFailed => "terminal input mode operation failed",
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
    fn restoreTerminalModes(renderer_handle: NativeHandle);
    fn suspendRenderer(renderer_handle: NativeHandle);
    fn resumeRenderer(renderer_handle: NativeHandle);
    fn enableMouse(renderer_handle: NativeHandle, enableMovement: bool);
    fn disableMouse(renderer_handle: NativeHandle);
    fn enableKittyKeyboard(renderer_handle: NativeHandle, flags: u8);
    fn disableKittyKeyboard(renderer_handle: NativeHandle);
    fn clearTerminal(renderer_handle: NativeHandle);
    fn resizeRenderer(renderer_handle: NativeHandle, width: u32, height: u32);
    fn setCursorPosition(renderer_handle: NativeHandle, x: i32, y: i32, visible: bool);
    fn setTerminalTitle(renderer_handle: NativeHandle, titlePtr: *const u8, titleLen: u32);
    fn getCurrentBuffer(renderer_handle: NativeHandle) -> NativeHandle;
    fn getNextBuffer(renderer_handle: NativeHandle) -> NativeHandle;
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
    /// Wait for native terminal input without taking the lifecycle mutex.
    #[cfg(all(feature = "native", unix))]
    pub fn input_ready(&self, timeout: Option<Duration>) -> Result<bool, BridgeError> {
        let handle = self.live()?;
        let slot = TERMINAL_INPUT
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(input) = slot.as_ref().filter(|input| input.handle == handle) else {
            return Ok(None);
        };
        let fd = input
            .fd
            .try_clone()
            .map_err(|_| BridgeError::TerminalFailed)?;
        let mut descriptors = [PollFd::new(
            &fd,
            PollFlags::IN | PollFlags::HUP | PollFlags::ERR,
        )];
        drop(slot);
        let end = timeout.map(|duration| std::time::Instant::now() + duration);
        loop {
            let remaining = end.map(|end| end.saturating_duration_since(std::time::Instant::now()));
            let wait = remaining.map(|duration| Timespec {
                tv_sec: duration.as_secs() as i64,
                tv_nsec: duration.subsec_nanos() as _,
            });
            match poll(&mut descriptors, wait.as_ref()) {
                Ok(0) => return Ok(false),
                Ok(_) => {
                    let events = descriptors[0].revents();
                    if events.contains(PollFlags::NVAL) {
                        return Err(BridgeError::TerminalFailed);
                    }
                    return Ok(events.intersects(PollFlags::IN | PollFlags::HUP | PollFlags::ERR));
                }
                Err(error) if error == rustix::io::Errno::INTR => continue,
                Err(_) => return Err(BridgeError::TerminalFailed),
            }
        }
    }

    /// Read terminal bytes from the same descriptor used by `input_ready`.
    /// This bypasses buffered stdin and performs one bounded kernel read.
    #[cfg(all(feature = "native", unix))]
    pub fn read_input(&self, buffer: &mut [u8]) -> Result<usize, BridgeError> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let handle = self.live()?;
        let slot = TERMINAL_INPUT
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let input = slot
            .as_ref()
            .filter(|input| input.handle == handle && input.active)
            .ok_or(BridgeError::TerminalFailed)?;
        let fd = input
            .fd
            .try_clone()
            .map_err(|_| BridgeError::TerminalFailed)?;
        drop(slot);
        loop {
            match rustix::io::read(&fd, &mut *buffer) {
                Ok(read) => return Ok(read),
                Err(error) if error == rustix::io::Errno::INTR => continue,
                Err(_) => return Err(BridgeError::TerminalFailed),
            }
        }
    }

    /// Read the live geometry from the same owned descriptor used for input.
    ///
    /// A renderer backed by a pipe or memory has no terminal geometry; that is
    /// a normal condition rather than a renderer failure.  The descriptor is
    /// cloned while holding the slot lock so the ioctl cannot race terminal
    /// teardown, then queried without borrowing a raw file descriptor.
    #[cfg(all(feature = "native", unix))]
    pub fn terminal_size(&self) -> Result<Option<(u32, u32)>, BridgeError> {
        let handle = self.live()?;
        let slot = TERMINAL_INPUT
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let input = slot
            .as_ref()
            .filter(|input| input.handle == handle)
            .ok_or(BridgeError::TerminalFailed)?;
        let fd = input
            .fd
            .try_clone()
            .map_err(|_| BridgeError::TerminalFailed)?;
        drop(slot);
        match tcgetwinsize(&fd) {
            Ok(size) if size.ws_col != 0 && size.ws_row != 0 => {
                Ok(Some((u32::from(size.ws_col), u32::from(size.ws_row))))
            }
            Ok(_) | Err(_) => Ok(None),
        }
    }

    /// Native platforms without a Unix terminal descriptor retain the
    /// caller's configured geometry. Closed renderers still fail validation.
    #[cfg(all(feature = "native", not(unix)))]
    pub fn terminal_size(&self) -> Result<Option<(u32, u32)>, BridgeError> {
        self.live()?;
        Ok(None)
    }

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
            let handle = unsafe { createRenderer(cols, rows, dest, 0, std::ptr::null()) };
            if handle == INVALID_HANDLE {
                LIFECYCLE_HANDLE.store(INVALID_HANDLE, Ordering::Release);
                LIFECYCLE_FLAGS.store(0, Ordering::Release);
                CLAIMED.store(false, Ordering::Release);
                return Err(BridgeError::CreateFailed);
            }
            LIFECYCLE_FLAGS.store(0, Ordering::Release);
            LIFECYCLE_HANDLE.store(handle, Ordering::Release);
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
        // SAFETY: live handle owned by self; owned terminal/input modes are
        // restored before the exactly-once destroy.
        unsafe {
            let owned = LIFECYCLE_HANDLE.load(Ordering::Acquire) == self.handle;
            let flags = if owned {
                LIFECYCLE_FLAGS.swap(0, Ordering::AcqRel)
            } else {
                0
            };
            if owned && flags & MOUSE_ENABLED != 0 {
                disableMouse(self.handle);
            }
            if owned && flags & KITTY_KEYBOARD_ENABLED != 0 {
                disableKittyKeyboard(self.handle);
            }
            if owned && flags & TERMINAL_ACTIVE != 0 {
                restoreTerminalModes(self.handle);
            }
            if owned {
                LIFECYCLE_HANDLE.store(INVALID_HANDLE, Ordering::Release);
            }
            destroyRenderer(self.handle, true);
        }
        #[cfg(all(feature = "native", unix))]
        if let Err(error) = restore_terminal_input(self.handle, true) {
            // Drop must also work during unwinding; report a genuine kernel
            // restoration failure without panicking or discarding its result.
            eprintln!("native renderer cleanup: {error}");
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
            #[cfg(unix)]
            activate_terminal_input(handle)?;
            // SAFETY: live handle; plain integers only.
            unsafe { setupTerminal(handle, true) };
            if LIFECYCLE_HANDLE.load(Ordering::Acquire) == handle {
                LIFECYCLE_FLAGS.fetch_or(TERMINAL_ACTIVE, Ordering::AcqRel);
            }
            Ok(())
        }
    }

    pub fn restore_terminal_modes(&self) -> Result<(), BridgeError> {
        let handle = self.live()?;
        #[cfg(not(feature = "native"))]
        {
            let _ = handle;
            return Err(BridgeError::InvalidHandle);
        }
        #[cfg(feature = "native")]
        {
            #[cfg(unix)]
            restore_terminal_input(handle, true)?;
            if LIFECYCLE_HANDLE.load(Ordering::Acquire) == handle
                && LIFECYCLE_FLAGS.fetch_and(!TERMINAL_ACTIVE, Ordering::AcqRel) & TERMINAL_ACTIVE
                    != 0
            {
                unsafe { restoreTerminalModes(handle) };
            }
            Ok(())
        }
    }

    pub fn suspend(&self) -> Result<(), BridgeError> {
        let handle = self.live()?;
        #[cfg(not(feature = "native"))]
        {
            let _ = handle;
            return Err(BridgeError::InvalidHandle);
        }
        #[cfg(feature = "native")]
        {
            // SAFETY: live renderer handle owned by self.
            unsafe { suspendRenderer(handle) };
            #[cfg(unix)]
            restore_terminal_input(handle, false)?;
            Ok(())
        }
    }

    pub fn resume(&self) -> Result<(), BridgeError> {
        let handle = self.live()?;
        #[cfg(not(feature = "native"))]
        {
            let _ = handle;
            return Err(BridgeError::InvalidHandle);
        }
        #[cfg(feature = "native")]
        {
            #[cfg(unix)]
            if LIFECYCLE_HANDLE.load(Ordering::Acquire) == handle
                && LIFECYCLE_FLAGS.load(Ordering::Acquire) & TERMINAL_ACTIVE != 0
            {
                activate_terminal_input(handle)?;
            }
            // SAFETY: live renderer handle owned by self.
            unsafe { resumeRenderer(handle) };
            Ok(())
        }
    }

    pub fn enable_mouse(&self, movement: bool) -> Result<(), BridgeError> {
        let handle = self.live()?;
        #[cfg(not(feature = "native"))]
        {
            let _ = (handle, movement);
            return Err(BridgeError::InvalidHandle);
        }
        #[cfg(feature = "native")]
        {
            unsafe { enableMouse(handle, movement) };
            if LIFECYCLE_HANDLE.load(Ordering::Acquire) == handle {
                LIFECYCLE_FLAGS.fetch_or(MOUSE_ENABLED, Ordering::AcqRel);
            }
            Ok(())
        }
    }

    pub fn disable_mouse(&self) -> Result<(), BridgeError> {
        let handle = self.live()?;
        #[cfg(not(feature = "native"))]
        {
            let _ = handle;
            return Err(BridgeError::InvalidHandle);
        }
        #[cfg(feature = "native")]
        {
            if LIFECYCLE_HANDLE.load(Ordering::Acquire) == handle
                && LIFECYCLE_FLAGS.fetch_and(!MOUSE_ENABLED, Ordering::AcqRel) & MOUSE_ENABLED != 0
            {
                unsafe { disableMouse(handle) };
            }
            Ok(())
        }
    }

    pub fn enable_kitty_keyboard(&self, flags: u8) -> Result<(), BridgeError> {
        let handle = self.live()?;
        #[cfg(not(feature = "native"))]
        {
            let _ = (handle, flags);
            return Err(BridgeError::InvalidHandle);
        }
        #[cfg(feature = "native")]
        {
            unsafe { enableKittyKeyboard(handle, flags) };
            if LIFECYCLE_HANDLE.load(Ordering::Acquire) == handle {
                LIFECYCLE_FLAGS.fetch_or(KITTY_KEYBOARD_ENABLED, Ordering::AcqRel);
            }
            Ok(())
        }
    }

    pub fn disable_kitty_keyboard(&self) -> Result<(), BridgeError> {
        let handle = self.live()?;
        #[cfg(not(feature = "native"))]
        {
            let _ = handle;
            return Err(BridgeError::InvalidHandle);
        }
        #[cfg(feature = "native")]
        {
            if LIFECYCLE_HANDLE.load(Ordering::Acquire) == handle
                && LIFECYCLE_FLAGS.fetch_and(!KITTY_KEYBOARD_ENABLED, Ordering::AcqRel)
                    & KITTY_KEYBOARD_ENABLED
                    != 0
            {
                unsafe { disableKittyKeyboard(handle) };
            }
            Ok(())
        }
    }

    pub fn clear_terminal(&self) -> Result<(), BridgeError> {
        let handle = self.live()?;
        #[cfg(not(feature = "native"))]
        {
            let _ = handle;
            return Err(BridgeError::InvalidHandle);
        }
        #[cfg(feature = "native")]
        {
            unsafe { clearTerminal(handle) };
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

    /// Run `paint` on the writable buffer, then present its changes.
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
            // OpenTUI treats `nextRenderBuffer` as the writable scene and
            // commits it into `currentRenderBuffer` during render(). Drawing
            // into current is only useful for snapshots and is discarded by
            // the persistent diff renderer.
            let buf = unsafe { getNextBuffer(handle) };
            if buf == INVALID_HANDLE {
                return Err(BridgeError::RenderFailed);
            }
            paint(buf);
            // SAFETY: live handle; `render` takes no pointers.
            // The pinned renderer tracks dirty cells and forces its own initial,
            // resize and terminal-restoration repaints. Forcing every frame
            // emits a full terminal even for one composer character.
            let status = unsafe { render(handle, false) };
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
            // Pinned core default foreground (`RGBA.ts` `defaultForeground`):
            // `DEFAULT_FOREGROUND_RGB=[255,255,255]`, alpha 255, packed with
            // `packMeta(INTENT_DEFAULT)`. Zig `bufferDrawText` takes fg as a
            // non-nullable `[*]const u16` (`lib.zig:1800-1810`; `ptrToRGBA`
            // at `:107-109` dereferences unconditionally), so null fg is
            // invalid. Bg is genuinely optional (`optionalPtrToRGBA` at
            // `:111-117`), so bg stays null.
            let fg = crate::color::pack_rgba8(
                255,
                255,
                255,
                255,
                crate::color::pack_meta(crate::color::INTENT_DEFAULT, 0),
            );
            // SAFETY: live handle; `text` and `fg` outlive the call; null bg
            // selects the native default (nullable `?[*]u16`).
            unsafe {
                let buf = getNextBuffer(handle);
                if buf == INVALID_HANDLE {
                    return Err(BridgeError::RenderFailed);
                }
                bufferDrawText(
                    buf,
                    text.as_ptr(),
                    text.len().min(u32::MAX as usize) as u32,
                    x,
                    y,
                    fg.as_ptr(),
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
                let buf = getNextBuffer(handle);
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
                let n = unsafe { bufferWriteResolvedChars(buf, out.as_mut_ptr(), cap as u32, true) }
                    as usize;
                let n = n.min(cap);
                if n < cap || cap >= SNAP_MAX {
                    out.truncate(n);
                    return String::from_utf8(out).map_err(|_| BridgeError::RenderFailed);
                }
                cap = (cap * 2).min(SNAP_MAX);
            }
        }
    }

    /// One-shot headless render: create a memory renderer, draw each line
    /// clipped to `(cols, rows)`, snapshot, then drop (releasing `CLAIMED`).
    ///
    /// Rows beyond `rows` are skipped; lines longer than `cols` are
    /// truncated by `char` count. Lines over [`MAX_TEXT_BYTES`] fail with
    /// [`BridgeError::TextTooLarge`] before any draw (validation precedes
    /// handle errors, matching `draw_text`/`set_title`).
    /// ponytail: char-count clip, not display width; upgrade: unicode-width.
    pub fn render_once(cols: u32, rows: u32, lines: &[String]) -> Result<String, BridgeError> {
        if cols == 0 || rows == 0 {
            return Err(BridgeError::ZeroSize);
        }
        if lines.iter().any(|l| l.len() > MAX_TEXT_BYTES) {
            return Err(BridgeError::TextTooLarge);
        }
        let mut renderer = Self::create_memory(cols, rows)?;
        let max_cols = cols as usize;
        for (y, line) in lines.iter().enumerate().take(rows as usize) {
            let clipped: String = line.chars().take(max_cols).collect();
            if clipped.is_empty() {
                continue;
            }
            renderer.draw_text(0, y as u32, &clipped)?;
        }
        renderer.frame(|_| {})?;
        renderer.snapshot_text()
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
    fn render_once_zero_size_rejected() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(
            Renderer::render_once(0, 24, &["hi".to_owned()]).unwrap_err(),
            BridgeError::ZeroSize
        );
        assert_eq!(
            Renderer::render_once(80, 0, &["hi".to_owned()]).unwrap_err(),
            BridgeError::ZeroSize
        );
    }

    #[test]
    fn render_once_oversize_rejected() {
        let _guard = TEST_LOCK.lock().unwrap();
        let big = "x".repeat(MAX_TEXT_BYTES + 1);
        assert_eq!(
            Renderer::render_once(80, 24, &[big]).unwrap_err(),
            BridgeError::TextTooLarge
        );
    }

    #[cfg(feature = "native")]
    #[test]
    fn render_once_content_present() {
        let _guard = TEST_LOCK.lock().unwrap();
        let out = Renderer::render_once(80, 24, &["hello".to_owned(), "world".to_owned()])
            .expect("memory render");
        assert!(out.contains("hello"), "snapshot missing row 0");
        assert!(out.contains("world"), "snapshot missing row 1");
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
