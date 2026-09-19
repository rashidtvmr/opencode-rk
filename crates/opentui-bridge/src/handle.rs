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

/// Opaque bridge handle placeholder (no unsafe yet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgeHandle(pub u64);

const _: () = assert!(std::mem::size_of::<NativeHandle>() == 4);
const _: () = assert!(std::mem::size_of::<RenderStatus>() == 1);

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

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
}
