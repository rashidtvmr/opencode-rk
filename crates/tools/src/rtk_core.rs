//! TOOL-019: native inbuilt RTK core filters.
//!
//! Pure Rust library replacing the external-wrapper hot path per PLAN.md
//! ADR-001 (no OS process per subagent). No `Command` spawn, no thread,
//! no I/O, no clock, no network. Time O(n), retained memory
//! O(`max_bytes + MARKER_MAX`). Deterministic: same input bytes + kind +
//! config yields byte-identical output.
//!
//! Order (enabled): strip ANSI CSI first, then kind-specific shrink, then
//! byte-budget truncate with marker. Disabled config aliases to [`raw`].
//! Exit codes pass through untouched; stderr is always preserved verbatim.
//! A previous truncation marker makes re-application passthrough, so chained
//! filters are idempotent.

/// Suffix of the byte-budget truncation marker. The full marker is
/// `"\n... [rtk:truncated {dropped} bytes, rerun with raw]"`.
pub const TRUNC_MARKER_SUFFIX: &str = ", rerun with raw]";

/// Marker prefix (bytes): `"\n... [rtk:truncated "`.
const TRUNC_MARKER_PREFIX: &[u8] = b"\n... [rtk:truncated ";
/// Middle of the marker between the dropped-byte count and the suffix.
const TRUNC_MARKER_MIDDLE: &[u8] = b" bytes";

/// Upper bound on any emitted truncation-marker length. The real marker is
/// `20 + digits + 6 + 17` bytes (<= 64 even for `usize::MAX`); 128 leaves
/// headroom so `stdout.len() <= max_bytes + MARKER_MAX` always holds.
pub const MARKER_MAX: usize = 128;

/// Per-line byte cap (I-10: single-line blowup). Lines longer than this are
/// cut at a UTF-8 boundary. Test fixtures use short lines, unaffected.
const MAX_LINE_BYTES: usize = 1000;

/// Which filter to apply. All variants share the pipeline; only the
/// head-line cap differs (grep mirrors the `rtk grep` max-200 precedent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilterKind {
    GitStatus,
    GitDiff,
    GitLog,
    Ls,
    Read,
    Grep,
    Find,
    Diff,
}

/// Filter configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtkConfig {
    /// Default-on. `false` makes [`filter`] an alias of [`raw`].
    pub enabled: bool,
    /// Byte budget for filtered stdout (marker excluded, see [`MARKER_MAX`]).
    pub max_bytes: usize,
}

impl Default for RtkConfig {
    fn default() -> Self {
        Self { enabled: true, max_bytes: 65536 }
    }
}

impl RtkConfig {
    /// Passthrough config: [`filter`] behaves exactly like [`raw`].
    pub fn disabled() -> Self {
        Self { enabled: false, max_bytes: usize::MAX }
    }
}

/// Filter result. `stderr` is always the verbatim input; `code` is never
/// remapped (shared exit-propagation choke point: per-tool code tables are
/// forbidden in this layer, fixing I-01/I-03/I-06 structurally).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filtered {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub code: i32,
    pub truncated: bool,
}

/// Escape hatch: byte-identical streams and code, `truncated: false`.
pub fn raw(stdout: &[u8], stderr: &[u8], code: i32) -> Filtered {
    Filtered {
        stdout: stdout.to_vec(),
        stderr: stderr.to_vec(),
        code,
        truncated: false,
    }
}

/// Strip ANSI CSI sequences (`ESC [ ... final-byte`), OSC sequences
/// (`ESC ] ... BEL`), and lone `ESC` bytes. Never emits `0x1B`.
fn strip_ansi(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if input[i] != 0x1B {
            out.push(input[i]);
            i += 1;
            continue;
        }
        if i + 1 < input.len() && input[i + 1] == b'[' {
            i += 2;
            while i < input.len() && !(0x40..=0x7E).contains(&input[i]) {
                i += 1;
            }
            i += 1; // consume final byte (or run past end on malformed input)
        } else if i + 1 < input.len() && input[i + 1] == b']' {
            i += 2;
            while i < input.len() {
                if input[i] == 0x07 {
                    i += 1;
                    break;
                }
                if input[i] == 0x1B && i + 1 < input.len() && input[i + 1] == b'\\' {
                    i += 2;
                    break;
                }
                i += 1;
            }
        } else {
            i += 1; // drop lone ESC
        }
    }
    out
}

/// Greatest `idx <= bytes.len()` that is a UTF-8 boundary, found by walking
/// back over continuation bytes (`0x80..=0xBF`). Operates on raw bytes so
/// non-UTF8 input is never corrupted.
fn floor_char_boundary(bytes: &[u8], idx: usize) -> usize {
    let mut i = idx.min(bytes.len());
    if i >= bytes.len() {
        return bytes.len();
    }
    while i > 0 && (bytes[i] & 0xC0) == 0x80 {
        i -= 1;
    }
    i
}

/// Head-line cap per kind. Generous: only pathological outputs hit it; the
/// byte budget below is the real bound.
fn max_lines(kind: FilterKind) -> usize {
    match kind {
        FilterKind::GitStatus => 1000,
        FilterKind::GitDiff => 1000,
        FilterKind::GitLog => 500,
        FilterKind::Ls => 2000,
        FilterKind::Read => 500,
        FilterKind::Grep => 200,
        FilterKind::Find => 2000,
        FilterKind::Diff => 1000,
    }
}

/// Kind-specific shrink over ANSI-free bytes: cut over-long lines, keep the
/// head `max_lines(kind)` lines. Returns the shrunk bytes plus the count of
/// dropped trailing lines (0 when nothing was dropped).
fn shrink(kind: FilterKind, input: &[u8]) -> (Vec<u8>, usize) {
    let cap = max_lines(kind);
    let mut out: Vec<u8> = Vec::with_capacity(input.len().min(65536));
    let mut kept = 0usize;
    let mut dropped = 0usize;
    // Split keeping the `\n` terminator so reassembly is exact.
    let mut start = 0usize;
    for (i, b) in input.iter().enumerate() {
        if *b != b'\n' {
            continue;
        }
        let line = &input[start..=i]; // includes `\n`
        let body_len = line.len() - 1;
        if kept < cap {
            let take = floor_char_boundary(line, body_len.min(MAX_LINE_BYTES));
            out.extend_from_slice(&line[..take]);
            out.push(b'\n');
            kept += 1;
        } else {
            dropped += 1;
        }
        start = i + 1;
    }
    if start < input.len() {
        // Trailing partial line without `\n`.
        if kept < cap {
            let rest = &input[start..];
            let take = floor_char_boundary(rest, rest.len().min(MAX_LINE_BYTES));
            out.extend_from_slice(&rest[..take]);
            kept += 1;
        } else {
            dropped += 1;
        }
    }
    (out, dropped)
}

/// True when `bytes` already carry our truncation marker (idempotent
/// re-application: chained filters become passthrough).
fn has_trunc_marker(bytes: &[u8]) -> bool {
    if !bytes.ends_with(TRUNC_MARKER_SUFFIX.as_bytes()) {
        return false;
    }
    bytes
        .windows(TRUNC_MARKER_PREFIX.len())
        .any(|w| w == TRUNC_MARKER_PREFIX)
}

/// Filter `stdout`, preserve `stderr` verbatim and `code` untouched.
pub fn filter(
    kind: FilterKind,
    stdout: &[u8],
    stderr: &[u8],
    code: i32,
    cfg: &RtkConfig,
) -> Filtered {
    if !cfg.enabled {
        return raw(stdout, stderr, code);
    }
    if stdout.is_empty() {
        return Filtered {
            stdout: Vec::new(),
            stderr: stderr.to_vec(),
            code,
            truncated: false,
        };
    }
    let stripped = strip_ansi(stdout);
    if has_trunc_marker(&stripped) {
        return Filtered {
            stdout: stripped,
            stderr: stderr.to_vec(),
            code,
            truncated: true,
        };
    }
    let (mut shrunk, dropped_lines) = shrink(kind, &stripped);
    if dropped_lines > 0 {
        // Under-budget notice for line-cap drops; over-budget outputs lose
        // this to the byte marker below, which is the authoritative signal.
        let note = format!("\n... [{dropped_lines} more lines, rerun with raw]\n");
        shrunk.extend_from_slice(note.as_bytes());
    }
    if shrunk.len() <= cfg.max_bytes {
        return Filtered {
            stdout: shrunk,
            stderr: stderr.to_vec(),
            code,
            truncated: false,
        };
    }
    let keep = floor_char_boundary(&shrunk, cfg.max_bytes);
    let dropped = shrunk.len() - keep;
    let mut out = Vec::with_capacity(keep + MARKER_MAX);
    out.extend_from_slice(&shrunk[..keep]);
    out.extend_from_slice(TRUNC_MARKER_PREFIX);
    out.extend_from_slice(dropped.to_string().as_bytes());
    out.extend_from_slice(TRUNC_MARKER_MIDDLE);
    out.extend_from_slice(TRUNC_MARKER_SUFFIX.as_bytes());
    debug_assert!(out.len() <= cfg.max_bytes.saturating_add(MARKER_MAX));
    Filtered {
        stdout: out,
        stderr: stderr.to_vec(),
        code,
        truncated: true,
    }
}
