#![forbid(unsafe_code)]
//! Clipboard read/write routing (mirrors `packages/tui/src/clipboard.ts`,
//! `packages/tui/src/context/clipboard.tsx`).
//!
//! TS checkout at /home/rashid/projects/opencode commit a0d9b6c (NOT the
//! pinned 95daf90; paths/lines cited below are against a0d9b6c).
//! Evidence: clipboard.ts:29-51 (darwin osascript PNG path, tmp
//! `opencode-clipboard.png`, fall through to text), clipboard.ts:53-60
//! (win32/WSL powershell image read), clipboard.ts:62-69 (linux wl-paste
//! then xclip image reads), clipboard.ts:71-73 (clipboardy text fallback),
//! clipboard.ts:76-94 (`copyCommand` per-OS branches), clipboard.ts:120-124
//! (`write`: OSC52 emit then native method), clipboard.tsx:4-8
//! (`ClipboardContent`/`ClipboardService` shape).
//!
//! Reuse boundary: `terminal.rs` owns the OSC52 byte encoding/emit
//! (`osc52_wrap`), the win32 console-mode call, and its own
//! `ClipboardBackend`/`check_clipboard_len`. This file owns the read/write
//! routing policy (image-first chain, text fallback) and validates payloads
//! with its own [`CopyCmd`] enum; it does not re-emit OSC52.

use std::fmt;

/// 1 MiB text bound (same value as `terminal.rs::MAX_CLIPBOARD_BYTES`;
/// re-stated so this module stays self-contained until wired in `lib.rs`).
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;

/// Clipboard payload: text write path or image-file read path.
/// TS: clipboard.ts:45,59,64,68 (image reads yield base64 PNG);
/// clipboard.tsx:4 (`ClipboardContent` data+mime).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardContent {
    Text(String),
    ImagePath(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardError {
    Unsupported,
    TooLarge,
    Io(String),
}

impl fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => write!(f, "clipboard unsupported on this platform"),
            Self::TooLarge => write!(f, "clipboard payload over capacity"),
            Self::Io(detail) => write!(f, "clipboard io failed: {detail}"),
        }
    }
}

impl std::error::Error for ClipboardError {}

/// Native copy program per OS (mirrors `copyCommand` branches,
/// clipboard.ts:76-94). Linux static pick is `xclip`; `wl-copy`/`xsel`
/// resolution is a runtime PATH probe (documented ceiling).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyCmd {
    Osascript,
    WlCopy,
    Xclip,
    Xsel,
    PowerShell,
}

impl CopyCmd {
    #[must_use]
    pub const fn program(self) -> &'static str {
        match self {
            Self::Osascript => "osascript",
            Self::WlCopy => "wl-copy",
            Self::Xclip => "xclip",
            Self::Xsel => "xsel",
            Self::PowerShell => "powershell.exe",
        }
    }
}

/// Compile-time copy-program pick (linux defaults to `xclip`; Wayland
/// `wl-copy` preference is resolved at runtime by probing PATH).
#[must_use]
pub const fn select_copy_cmd() -> &'static str {
    #[cfg(target_os = "macos")]
    return CopyCmd::Osascript.program();
    #[cfg(windows)]
    return CopyCmd::PowerShell.program();
    #[cfg(target_os = "linux")]
    return CopyCmd::Xclip.program();
    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    return "";
}

/// Empty program pick means no native helper: caller falls back to OSC52
/// (`terminal.rs::osc52_wrap`) plus clipboardy-style text path.
#[must_use]
pub fn has_native_copy() -> bool {
    !select_copy_cmd().is_empty()
}

/// Read routing: image-first chain (osascript/powershell/wl-paste/xclip)
/// with text fallback is supported on darwin, win32, and linux.
/// TS: clipboard.ts:29-74.
#[must_use]
pub const fn reads_image_first() -> bool {
    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    return true;
    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    return false;
}

/// Bound text payloads at [`MAX_TEXT_BYTES`]; reject empty image paths.
/// Fail-closed on oversize/empty.
pub fn validate(content: &ClipboardContent) -> Result<(), ClipboardError> {
    match content {
        ClipboardContent::Text(text) => {
            if text.len() > MAX_TEXT_BYTES {
                return Err(ClipboardError::TooLarge);
            }
            Ok(())
        }
        ClipboardContent::ImagePath(path) => {
            if path.is_empty() {
                return Err(ClipboardError::Io("empty image path".to_string()));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_text_ok() {
        assert_eq!(validate(&ClipboardContent::Text("hi".to_string())), Ok(()));
    }

    #[test]
    fn validate_text_too_large() {
        let big = "x".repeat(MAX_TEXT_BYTES + 1);
        assert_eq!(validate(&ClipboardContent::Text(big)), Err(ClipboardError::TooLarge));
    }

    #[test]
    fn validate_image_path_rejects_empty() {
        assert!(validate(&ClipboardContent::ImagePath(String::new())).is_err());
        assert_eq!(
            validate(&ClipboardContent::ImagePath("/tmp/opencode-clipboard.png".to_string())),
            Ok(())
        );
    }

    #[test]
    fn copy_cmd_select_compiles_per_cfg() {
        let cmd = select_copy_cmd();
        assert_eq!(has_native_copy(), !cmd.is_empty());
        #[cfg(target_os = "macos")]
        assert_eq!(cmd, "osascript");
        #[cfg(target_os = "linux")]
        assert_eq!(cmd, "xclip");
        #[cfg(windows)]
        assert_eq!(cmd, "powershell.exe");
    }

    #[test]
    fn image_first_on_supported_platforms() {
        #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
        assert!(reads_image_first());
    }
}
