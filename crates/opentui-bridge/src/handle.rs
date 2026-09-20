#![forbid(unsafe_code)]
//! Native ABI handle + render status (mirrors `lib.zig`).

/// Zig `NativeHandle` (= `handles.Handle`): 32-bit slot. 0 = invalid.
pub type NativeHandle = u32;
/// Zig `INVALID_HANDLE`.
pub const INVALID_HANDLE: u32 = 0;

/// Zig `renderer.RenderStatus` discriminant, returned as `u8` by `render`.
pub type RenderStatus = u8;
/// Render produced output.
pub const RENDER_STATUS_RENDERED: RenderStatus = 0;
/// Render skipped (nothing changed).
pub const RENDER_STATUS_SKIPPED: RenderStatus = 1;
/// Render failed (e.g. invalid handle).
pub const RENDER_STATUS_FAILED: RenderStatus = 2;

/// Fail-closed discriminant check.
#[must_use]
pub const fn render_status_is_known(status: u8) -> bool {
    matches!(status, 0 | 1 | 2)
}

/// Opaque single-owner bridge handle (mirrors `Renderer` claim pattern).
///
/// Exactly one live [`BridgeHandle`] at a time, enforced by `CLAIMED`.
/// `Copy`/`Clone` are deliberately absent: ownership moves, never duplicates.
/// Use [`BridgeHandle::claim`] to take the slot, [`BridgeHandle::close`] to
/// release early; [`Drop`] releases otherwise. After close/drop the slot is
/// free and any `live()` check fails [`BridgeError::InvalidHandle`].
#[derive(Debug)]
pub struct BridgeHandle {
    raw: NativeHandle,
    _no_send: std::marker::PhantomData<*const ()>,
}

use std::sync::atomic::{AtomicBool, Ordering};

use crate::safe_renderer::BridgeError;

/// Global single-owner claim. Held while a [`BridgeHandle`] is live.
static CLAIMED: AtomicBool = AtomicBool::new(false);

impl BridgeHandle {
    /// Take the single-owner slot for `raw`. Fails `InvalidHandle` on
    /// [`INVALID_HANDLE`] (claim untouched) or `AlreadyLive` when held.
    pub fn claim(raw: NativeHandle) -> Result<Self, BridgeError> {
        if raw == INVALID_HANDLE {
            return Err(BridgeError::InvalidHandle);
        }
        if CLAIMED.swap(true, Ordering::AcqRel) {
            return Err(BridgeError::AlreadyLive);
        }
        Ok(Self {
            raw,
            _no_send: std::marker::PhantomData,
        })
    }

    /// Borrow the live raw slot, or `InvalidHandle` once closed.
    pub fn live(&self) -> Result<NativeHandle, BridgeError> {
        if self.raw == INVALID_HANDLE {
            return Err(BridgeError::InvalidHandle);
        }
        Ok(self.raw)
    }

    /// Slot still owned (not yet closed).
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.raw != INVALID_HANDLE
    }

    fn release(&mut self) {
        if self.raw == INVALID_HANDLE {
            return;
        }
        self.raw = INVALID_HANDLE;
        CLAIMED.store(false, Ordering::Release);
    }

    /// Release early and free the claim; further `live()` fails
    /// `InvalidHandle`. Idempotent.
    pub fn close(&mut self) {
        self.release();
    }
}

impl Drop for BridgeHandle {
    fn drop(&mut self) {
        self.release();
    }
}

const _: () = assert!(std::mem::size_of::<NativeHandle>() == 4);
const _: () = assert!(std::mem::size_of::<RenderStatus>() == 1);

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn handle_sizes_stable() {
        assert_eq!(size_of::<NativeHandle>(), 4);
        assert_eq!(size_of::<RenderStatus>(), 1);
    }

    #[test]
    fn invalid_handle_is_zero() {
        assert_eq!(INVALID_HANDLE, 0);
        assert_ne!(1 as NativeHandle, INVALID_HANDLE);
    }

    #[test]
    fn render_status_known_only_for_defined() {
        assert_eq!(
            (
                RENDER_STATUS_RENDERED,
                RENDER_STATUS_SKIPPED,
                RENDER_STATUS_FAILED
            ),
            (0, 1, 2)
        );
        assert!(render_status_is_known(RENDER_STATUS_RENDERED));
        assert!(render_status_is_known(RENDER_STATUS_SKIPPED));
        assert!(render_status_is_known(RENDER_STATUS_FAILED));
        assert!(!render_status_is_known(3));
        assert!(!render_status_is_known(u8::MAX));
    }

    #[test]
    fn claim_release_cycle() {
        let _guard = TEST_LOCK.lock().unwrap();
        let mut h = BridgeHandle::claim(7).expect("claim");
        assert!(h.is_live());
        assert_eq!(h.live(), Ok(7));
        h.close();
        assert!(!h.is_live());
        assert_eq!(h.live(), Err(BridgeError::InvalidHandle));
        // Slot freed: reclaim works.
        let _again = BridgeHandle::claim(9).expect("reclaim after close");
    }

    #[test]
    fn double_claim_rejected() {
        let _guard = TEST_LOCK.lock().unwrap();
        let _held = BridgeHandle::claim(7).expect("first claim");
        assert_eq!(BridgeHandle::claim(8).unwrap_err(), BridgeError::AlreadyLive);
    }

    #[test]
    fn zero_raw_rejected_without_touching_claim() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(
            BridgeHandle::claim(INVALID_HANDLE).unwrap_err(),
            BridgeError::InvalidHandle
        );
        // Claim untouched, still free.
        let _held = BridgeHandle::claim(7).expect("claim after zero rejected");
    }

    #[test]
    fn close_idempotent() {
        let _guard = TEST_LOCK.lock().unwrap();
        let mut h = BridgeHandle::claim(7).expect("claim");
        h.close();
        h.close();
        assert_eq!(h.live(), Err(BridgeError::InvalidHandle));
    }

    #[test]
    fn drop_releases_claim() {
        let _guard = TEST_LOCK.lock().unwrap();
        {
            let _held = BridgeHandle::claim(7).expect("claim");
        }
        let _again = BridgeHandle::claim(8).expect("claim after drop");
    }

    #[test]
    fn closed_handle_rejected() {
        let _guard = TEST_LOCK.lock().unwrap();
        let mut h = BridgeHandle::claim(7).expect("claim");
        h.close();
        assert_eq!(h.live().unwrap_err(), BridgeError::InvalidHandle);
    }
}
