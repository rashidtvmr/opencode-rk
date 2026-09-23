#![forbid(unsafe_code)]

//! Clipboard bridge: trait + backends, null default, bounded text, OSC52.
//!
//! Mirrors `packages/core/src/lib/clipboard.ts` (`ClipboardService`,
//! `HostClipboardBackend`, `TerminalClipboardAdapter`, destination policies),
//! `host-clipboard.internal.ts` (`validateClipboardText`, 8 MiB default cap),
//! and `packages/native/src/terminal.zig` (`writeClipboardSequence`:
//! `ESC ] 52 ; <c> ; <base64> ST`, clear = empty payload, standard base64).
//! Std-only, no process spawn.

/// Byte cap for clipboard text. Mirrors `DEFAULT_CLIPBOARD_MAX_BYTES` (8 MiB).
pub const MAX_CLIP_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardError {
    Empty,
    ContainsNul,
    TooLarge,
    Unsupported,
    Failed(&'static str),
}

impl std::fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "clipboard text must be non-empty"),
            Self::ContainsNul => write!(f, "clipboard text contains NUL"),
            Self::TooLarge => write!(f, "clipboard text exceeds size limit"),
            Self::Unsupported => write!(f, "clipboard unsupported"),
            Self::Failed(m) => write!(f, "clipboard failed: {m}"),
        }
    }
}

impl std::error::Error for ClipboardError {}

/// Validate clipboard text: non-empty, no NUL, within `max_bytes` UTF-8
/// bytes. Mirrors `validateClipboardText` in `host-clipboard.internal.ts`.
pub fn validate_clipboard_text(text: &str, max_bytes: usize) -> Result<(), ClipboardError> {
    if text.is_empty() {
        return Err(ClipboardError::Empty);
    }
    if text.as_bytes().contains(&0) {
        return Err(ClipboardError::ContainsNul);
    }
    if text.len() > max_bytes {
        return Err(ClipboardError::TooLarge);
    }
    Ok(())
}

const B64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard-base64 encode (no line breaks), mirroring
/// `std.base64.standard.Encoder` used by `writeClipboardBase64`.
fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64_ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(B64_ALPHABET[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64_ALPHABET[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64_ALPHABET[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn base64_decode_value(c: u8) -> Option<u32> {
    match c {
        b'A'..=b'Z' => Some((c - b'A') as u32),
        b'a'..=b'z' => Some((c - b'a' + 26) as u32),
        b'0'..=b'9' => Some((c - b'0' + 52) as u32),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    if (b.len() % 4 != 0) {
        return None;
    }
    let mut out = Vec::with_capacity(b.len() / 4 * 3);
    for chunk in b.chunks(4) {
        let mut n: u32 = 0;
        let mut pad = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            if c == b'=' {
                if i < 2 {
                    return None;
                }
                pad += 1;
                n <<= 6;
            } else {
                if pad > 0 {
                    return None;
                }
                n = (n << 6) | base64_decode_value(c)?;
            }
        }
        if pad > 2 {
            return None;
        }
        out.push(((n >> 16) & 0xff) as u8);
        if pad < 2 {
            out.push(((n >> 8) & 0xff) as u8);
        }
        if pad < 1 {
            out.push((n & 0xff) as u8);
        }
    }
    Some(out)
}

/// Encode raw bytes as an OSC 52 sequence `ESC ] 52 ; <c> ; <base64> ST`,
/// mirroring `writeClipboardSequence` (`terminal.zig`). Empty payload is the
/// clear sequence (`renderer.zig` `clearClipboardOSC52`).
pub fn encode_osc52(text: &[u8], target: ClipboardTarget) -> String {
    let mut seq = String::from("\x1b]52;");
    seq.push(target.to_char());
    seq.push(';');
    seq.push_str(&base64_encode(text));
    seq.push_str("\x1b\\");
    seq
}

/// Decode an OSC 52 sequence produced by [`encode_osc52`]; returns the
/// target char and raw payload bytes.
pub fn decode_osc52_payload(sequence: &str) -> Option<(char, Vec<u8>)> {
    let rest = sequence.strip_prefix("\x1b]52;")?;
    let rest = rest.strip_suffix("\x1b\\")?;
    let mut parts = rest.splitn(2, ';');
    let target = parts.next()?;
    let payload = parts.next()?;
    let t: char = target.chars().next().filter(|_| target.len() == 1)?;
    if !matches!(t, 'c' | 'p' | 's' | 'q') {
        return None;
    }
    Some((t, base64_decode(payload)?))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardTarget {
    Clipboard,
    Primary,
    Select,
    Secondary,
}

impl ClipboardTarget {
    pub fn to_char(self) -> char {
        match self {
            Self::Clipboard => 'c',
            Self::Primary => 'p',
            Self::Select => 's',
            Self::Secondary => 'q',
        }
    }
}

/// Clipboard backend trait. Mirrors `HostClipboardBackend`
/// (`clipboard.ts` lines 62-70): read / writeText / clear / dispose
/// (dispose = drop here; no async runtime in std-only bridge).
pub trait Clipboard {
    /// Read returns `None` when empty/unsupported; `Err` only on hard failure.
    fn read(&self) -> Result<Option<String>, ClipboardError>;
    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError>;
    fn clear(&mut self) -> Result<(), ClipboardError>;
}

/// No-op backend: reads report unsupported (clean error, never panic),
/// writes/clears are accepted and counted. Default when no host clipboard.
#[derive(Debug, Default)]
pub struct NullClipboard {
    writes: u32,
}

impl NullClipboard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn writes(&self) -> u32 {
        self.writes
    }
}

impl Clipboard for NullClipboard {
    fn read(&self) -> Result<Option<String>, ClipboardError> {
        Err(ClipboardError::Unsupported)
    }

    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        validate_clipboard_text(text, MAX_CLIP_BYTES)?;
        self.writes = self.writes.saturating_add(1);
        Ok(())
    }

    fn clear(&mut self) -> Result<(), ClipboardError> {
        self.writes = self.writes.saturating_add(1);
        Ok(())
    }
}

/// Write destination policy. Mirrors `ClipboardWriteDestination`
/// (`clipboard.ts` lines 72-73).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    TerminalOnly,
    HostOnly,
    BestAvailable,
    AllAvailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Host,
    Terminal,
}

/// Ordered dispatch places for a write/clear, mirroring `composeMutation`
/// (`clipboard.ts` lines 197-226): terminal-only skips host; host-only skips
/// terminal; best-available prefers host locally (terminal on remote, or as
/// fallback when host failed); all-available uses both.
pub fn fallback_order(dest: Destination, host_ok: bool, remote: bool) -> Vec<Place> {
    match dest {
        Destination::TerminalOnly => vec![Place::Terminal],
        Destination::HostOnly => vec![Place::Host],
        Destination::BestAvailable => {
            if remote {
                vec![Place::Terminal]
            } else if host_ok {
                vec![Place::Host]
            } else {
                vec![Place::Host, Place::Terminal]
            }
        }
        Destination::AllAvailable => vec![Place::Host, Place::Terminal],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_read_errors_cleanly() {
        let c = NullClipboard::new();
        assert_eq!(c.read().unwrap_err(), ClipboardError::Unsupported);
    }

    #[test]
    fn osc52_roundtrip() {
        let text = "hello 世界";
        let seq = encode_osc52(text.as_bytes(), ClipboardTarget::Clipboard);
        assert!(seq.starts_with("\x1b]52;c;"), "missing OSC52 header: {seq:?}");
        assert!(seq.ends_with("\x1b\\"), "missing ST terminator: {seq:?}");
        let (target, bytes) = decode_osc52_payload(&seq).expect("decode");
        assert_eq!(target, 'c');
        assert_eq!(bytes, text.as_bytes());
    }

    #[test]
    fn osc52_clear_is_empty_payload() {
        let seq = encode_osc52(b"", ClipboardTarget::Primary);
        assert_eq!(seq, "\x1b]52;p;\x1b\\");
    }

    #[test]
    fn oversize_rejected() {
        let big = "x".repeat(MAX_CLIP_BYTES + 1);
        assert_eq!(
            validate_clipboard_text(&big, MAX_CLIP_BYTES).unwrap_err(),
            ClipboardError::TooLarge
        );
        assert!(validate_clipboard_text(&"x".repeat(MAX_CLIP_BYTES), MAX_CLIP_BYTES).is_ok());
    }

    #[test]
    fn nul_and_empty_rejected() {
        assert_eq!(
            validate_clipboard_text("", MAX_CLIP_BYTES).unwrap_err(),
            ClipboardError::Empty
        );
        assert_eq!(
            validate_clipboard_text("a\0b", MAX_CLIP_BYTES).unwrap_err(),
            ClipboardError::ContainsNul
        );
    }

    #[test]
    fn fallback_order_best_available() {
        assert_eq!(
            fallback_order(Destination::BestAvailable, true, false),
            vec![Place::Host]
        );
        assert_eq!(
            fallback_order(Destination::BestAvailable, false, false),
            vec![Place::Host, Place::Terminal]
        );
        assert_eq!(
            fallback_order(Destination::BestAvailable, true, true),
            vec![Place::Terminal]
        );
    }

    #[test]
    fn fallback_order_terminal_host_all() {
        assert_eq!(
            fallback_order(Destination::TerminalOnly, true, false),
            vec![Place::Terminal]
        );
        assert_eq!(
            fallback_order(Destination::HostOnly, true, false),
            vec![Place::Host]
        );
        assert_eq!(
            fallback_order(Destination::AllAvailable, true, false),
            vec![Place::Host, Place::Terminal]
        );
    }
}
