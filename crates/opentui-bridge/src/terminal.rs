#![forbid(unsafe_code)]
//! Win32 console mode + OSC52 clipboard (mirrors
//! `packages/tui/src/terminal-win32.ts`, `packages/tui/src/clipboard.ts`,
//! `packages/tui/src/app.tsx` win32 call sites).
//!
//! TS checkout at /home/rashid/projects/opencode commit a0d9b6c (NOT the
//! pinned 95daf90; paths/lines cited below are against a0d9b6c).
//! Evidence: terminal-win32.ts:30 (`win32DisableProcessedInput` clears
//! `ENABLE_PROCESSED_INPUT` via kernel32), clipboard.ts:23-27 (`writeOsc52`
//! base64 + `TMUX`/`STY` passthrough wrap), app.tsx:86,214,358 (call sites).

use std::env;
use std::fmt;

/// 1 MiB clipboard bound (DoS cap on OSC52 / child-stdin payloads).
pub const MAX_CLIPBOARD_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalError {
    Unsupported,
    OverCapacity,
    SpawnFailed,
}

impl fmt::Display for TerminalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => write!(f, "unsupported platform"),
            Self::OverCapacity => write!(f, "clipboard payload over capacity"),
            Self::SpawnFailed => write!(f, "clipboard helper spawn failed"),
        }
    }
}

impl std::error::Error for TerminalError {}

/// Byte-length gate against [`MAX_CLIPBOARD_BYTES`].
pub fn check_clipboard_len(len: usize) -> Result<(), TerminalError> {
    if len > MAX_CLIPBOARD_BYTES {
        return Err(TerminalError::OverCapacity);
    }
    Ok(())
}

/// Clear `ENABLE_PROCESSED_INPUT` so Ctrl+C arrives as stdin, not an event.
/// TS: terminal-win32.ts:30. Non-Windows: `Err(Unsupported)`.
#[cfg(windows)]
pub fn win32_disable_processed_input() -> Result<(), TerminalError> {
    // ponytail: child powershell P/Invoke edits console-global mode; direct
    // kernel32 needs unsafe/ffi. Upgrade when windows-sys dep accepted.
    let script = "Add-Type -MemberDefinition '[DllImport(\"kernel32.dll\")]public static extern IntPtr GetStdHandle(int n);[DllImport(\"kernel32.dll\")]public static extern bool GetConsoleMode(IntPtr h,out uint m);[DllImport(\"kernel32.dll\")]public static extern bool SetConsoleMode(IntPtr h,uint m);' -Name C -Namespace W; $h=[W.C]::GetStdHandle(-10); $m=0; [W.C]::GetConsoleMode($h,[ref]$m); [W.C]::SetConsoleMode($h,($m -band (-bnot 1)))";
    let status = std::process::Command::new("powershell.exe")
        .args(["-NonInteractive", "-NoProfile", "-Command", script])
        .status()
        .map_err(|_| TerminalError::SpawnFailed)?;
    if status.success() {
        Ok(())
    } else {
        Err(TerminalError::SpawnFailed)
    }
}

#[cfg(not(windows))]
pub fn win32_disable_processed_input() -> Result<(), TerminalError> {
    Err(TerminalError::Unsupported)
}

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | (*chunk.get(2).unwrap_or(&0) as u32);
        out.push(ALPHA[((n >> 18) & 63) as usize] as char);
        out.push(ALPHA[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { ALPHA[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHA[(n & 63) as usize] as char } else { '=' });
    }
    out
}

fn osc52_inner(text: &str, multiplexed: bool) -> String {
    let seq = format!("\x1b]52;c;{}\x07", base64_encode(text.as_bytes()));
    if multiplexed {
        format!("\x1bPtmux;\x1b{seq}\x1b\\")
    } else {
        seq
    }
}

/// OSC52 sequence; wraps in tmux passthrough when `TMUX`/`STY` set.
/// TS: clipboard.ts:23-27.
#[must_use]
pub fn osc52_wrap(text: &str) -> String {
    let multiplexed = env::var_os("TMUX").is_some() || env::var_os("STY").is_some();
    osc52_inner(text, multiplexed)
}

/// Native copy helper per OS (mirrors clipboard.ts `copyCommand` branches).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardBackend {
    Osascript,
    WlCopy,
    Xclip,
    Xsel,
    PowerShell,
    Osc52,
}

/// Compile-time backend pick; linux defaults to `WlCopy` under Wayland
/// env presence is runtime, so static pick is `Xclip` (documented ceiling).
#[must_use]
pub fn select_backend() -> ClipboardBackend {
    #[cfg(target_os = "macos")]
    return ClipboardBackend::Osascript;
    #[cfg(windows)]
    return ClipboardBackend::PowerShell;
    #[cfg(target_os = "linux")]
    return ClipboardBackend::Xclip;
    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    return ClipboardBackend::Osc52;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disable_errs_unsupported_off_windows() {
        #[cfg(not(windows))]
        assert_eq!(win32_disable_processed_input(), Err(TerminalError::Unsupported));
        #[cfg(windows)]
        let _ = win32_disable_processed_input();
    }

    #[test]
    fn osc52_contains_base64_payload() {
        assert!(osc52_wrap("hi").contains("aGk="));
    }

    #[test]
    fn osc52_tmux_wrap_variant() {
        let plain = osc52_inner("hi", false);
        let wrapped = osc52_inner("hi", true);
        assert!(plain.starts_with("\x1b]52;c;"));
        assert!(wrapped.starts_with("\x1bPtmux;\x1b\x1b]52;c;"));
        assert!(wrapped.ends_with("\x1b\\"));
    }

    #[test]
    fn backend_select_compiles_per_cfg() {
        let _ = select_backend();
    }

    #[test]
    fn over_cap_errs() {
        assert_eq!(check_clipboard_len(MAX_CLIPBOARD_BYTES + 1), Err(TerminalError::OverCapacity));
        assert_eq!(check_clipboard_len(MAX_CLIPBOARD_BYTES), Ok(()));
    }
}
