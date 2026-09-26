#![forbid(unsafe_code)]
//! Full FFI surface declaration model (opaque handles only).
//!
//! Upstream cited by `text.rs`/`renderer.rs`/`buffer.rs`:
//! `packages/native/src/lib.zig` (branch `rust-bridge`). No `packages/`
//! checkout exists on disk here, so the Zig FFI surface is undiffable
//! from this tree; this module declares the handle model only and
//! adds no `extern` blocks and no `unsafe`.

/// Cap on live opaque handles tracked by the model.
pub const MAX_HANDLES: usize = 1024;

/// Opaque native handle (declaration only; 0 is never valid).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FfiHandle(u64);

impl FfiHandle {
    /// Wrap `id` when [`alloc_ok`] holds, else `None`.
    #[must_use]
    pub const fn new(id: u64) -> Option<Self> {
        if alloc_ok(id) {
            Some(Self(id))
        } else {
            None
        }
    }

    /// Raw handle value.
    #[must_use]
    pub const fn id(self) -> u64 {
        self.0
    }
}

/// True when `id` names an allocatable handle slot (1..=MAX_HANDLES).
#[must_use]
pub const fn alloc_ok(id: u64) -> bool {
    id != 0 && id <= MAX_HANDLES as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_handles_pinned() {
        assert_eq!(MAX_HANDLES, 1024);
    }

    #[test]
    fn alloc_ok_bounds() {
        assert!(!alloc_ok(0));
        assert!(alloc_ok(1));
        assert!(alloc_ok(MAX_HANDLES as u64));
        assert!(!alloc_ok(MAX_HANDLES as u64 + 1));
        assert!(!alloc_ok(u64::MAX));
    }

    #[test]
    fn handle_roundtrip() {
        assert_eq!(FfiHandle::new(7).unwrap().id(), 7);
        assert!(FfiHandle::new(0).is_none());
        assert!(FfiHandle::new(MAX_HANDLES as u64 + 1).is_none());
    }
}
