//! TOOL-021 (RTK-SLICE-03 pass): pass/chain/proxy/opt-out filters.
//!
//! RED stub: API surface compiles, behavior is raw-only. Fails T02..T05.

use std::ffi::OsStr;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Suffix of the byte-budget truncation marker.
pub const TRUNC_MARKER_SUFFIX: &str = ", rerun with raw]";
/// Marker prefix (bytes): `"\n... [rtk:truncated "`.
const TRUNC_MARKER_PREFIX: &[u8] = b"\n... [rtk:truncated ";
/// Middle of the marker between the dropped-byte count and the suffix.
const TRUNC_MARKER_MIDDLE: &[u8] = b" bytes";

/// Upper bound on any emitted truncation-marker length.
pub const MARKER_MAX: usize = 128;

/// Env var forcing raw output for every kind.
pub const ENV_NO_FILTER: &str = "RTK_NO_FILTER";

/// Which filter to apply. `Unknown` is always byte-identical passthrough.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassKind {
    Err,
    Log,
    Json,
    Summary,
    Deps,
    Env,
    Gh,
    Docker,
    Kubectl,
    Pip,
    Pnpm,
    Npm,
    Unknown,
}

impl PassKind {
    /// Every known (filterable) kind.
    pub const ALL_KNOWN: [PassKind; 12] = [
        PassKind::Err,
        PassKind::Log,
        PassKind::Json,
        PassKind::Summary,
        PassKind::Deps,
        PassKind::Env,
        PassKind::Gh,
        PassKind::Docker,
        PassKind::Kubectl,
        PassKind::Pip,
        PassKind::Pnpm,
        PassKind::Npm,
    ];
}

/// Filter configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtkConfig {
    /// Default-on. `false` makes [`filter_pass`] an alias of [`raw`].
    pub enabled: bool,
    /// Byte budget for filtered stdout (marker excluded).
    pub max_bytes: usize,
    /// Per-command opt-out. Composes with global flag as OR.
    pub no_filter: bool,
}

impl Default for RtkConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_bytes: 65536,
            no_filter: false,
        }
    }
}

impl RtkConfig {
    /// Passthrough config: [`filter_pass`] behaves exactly like [`raw`].
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            max_bytes: usize::MAX,
            no_filter: false,
        }
    }
}

/// Filter result. `stderr` verbatim, `code` never remapped.
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

static SHRINK_CALLS: AtomicUsize = AtomicUsize::new(0);

/// Shrink-path call count (micro-assert for opt-out zero overhead).
pub fn shrink_calls() -> usize {
    SHRINK_CALLS.load(Ordering::Relaxed)
}

/// Reset [`shrink_calls`].
pub fn reset_shrink_calls() {
    SHRINK_CALLS.store(0, Ordering::Relaxed);
}

/// Pure mapping of an env-var value to opt-out. Accepts `1`/`true`/`yes`.
pub fn env_flag_value(v: Option<&OsStr>) -> bool {
    match v.and_then(|s| s.to_str()) {
        Some(s) => {
            s.eq_ignore_ascii_case("1")
                || s.eq_ignore_ascii_case("true")
                || s.eq_ignore_ascii_case("yes")
        }
        None => false,
    }
}

/// Live `RTK_NO_FILTER` check.
pub fn env_no_filter() -> bool {
    env_flag_value(std::env::var_os(ENV_NO_FILTER).as_deref())
}

/// Map a command line to its filter kind by first token (`rtk`/`proxy` skipped).
pub fn kind_of(cmd: &str) -> PassKind {
    let mut toks = cmd.split_whitespace();
    let mut head = toks.next().unwrap_or("");
    if head == "rtk" {
        head = toks.next().unwrap_or("");
    }
    if head == "proxy" {
        head = toks.next().unwrap_or("");
    }
    let mut buf = [0u8; 16];
    let b = head.as_bytes();
    if b.len() > buf.len() {
        return PassKind::Unknown;
    }
    buf[..b.len()].copy_from_slice(b);
    buf[..b.len()].make_ascii_lowercase();
    match &buf[..b.len()] {
        b"err" => PassKind::Err,
        b"log" => PassKind::Log,
        b"json" => PassKind::Json,
        b"summary" => PassKind::Summary,
        b"deps" => PassKind::Deps,
        b"env" => PassKind::Env,
        b"gh" => PassKind::Gh,
        b"docker" => PassKind::Docker,
        b"kubectl" => PassKind::Kubectl,
        b"pip" => PassKind::Pip,
        b"pnpm" => PassKind::Pnpm,
        b"npm" => PassKind::Npm,
        _ => PassKind::Unknown,
    }
}

/// Split a `&&` chain, respecting single/double quotes and backslash escapes.
pub fn split_chain(cmd: &str) -> Vec<String> {
    let mut segs = Vec::new();
    let mut cur = String::new();
    let mut single = false;
    let mut double = false;
    let mut chars = cmd.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(n) = chars.next() {
                cur.push(c);
                cur.push(n);
            } else {
                cur.push(c);
            }
            continue;
        }
        if c == '\'' && !double {
            single = !single;
            cur.push(c);
            continue;
        }
        if c == '"' && !single {
            double = !double;
            cur.push(c);
            continue;
        }
        if c == '&' && !single && !double && chars.peek() == Some(&'&') {
            chars.next();
            let t = cur.trim().to_string();
            if !t.is_empty() {
                segs.push(t);
            }
            cur.clear();
            continue;
        }
        cur.push(c);
    }
    let t = cur.trim().to_string();
    if !t.is_empty() {
        segs.push(t);
    }
    segs
}

/// One executed chain segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub kind: PassKind,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub code: i32,
}

/// Strip ANSI CSI sequences (`ESC [ ... final-byte`), OSC sequences
/// (`ESC ] ... BEL`), and lone `ESC` bytes. Never emits `0x1B`. Pure O(n).
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
            i += 1;
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
            i += 1;
        }
    }
    out
}

/// Greatest `idx <= bytes.len()` that is a UTF-8 boundary, found by walking
/// back over continuation bytes (`0x80..=0xBF`). Byte ops only.
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

/// A line is noise when it starts with a noise prefix and carries no verdict
/// or error signal. Byte-level compare, lowercase on the fly, no regex.
fn is_noise(line: &[u8]) -> bool {
    const NOISE: [&[u8]; 6] = [b"info:", b"debug:", b"trace:", b"ok ", b"pass ", b"nominal"];
    const SIGNAL: [&[u8]; 7] = [
        b"verdict", b"error", b"fail", b"panic", b"e0", b"warn", b"deny",
    ];
    let has = |needle: &[u8]| {
        line.len() >= needle.len()
            && (0..=line.len() - needle.len()).any(|s| {
                line[s..s + needle.len()]
                    .iter()
                    .zip(needle.iter())
                    .all(|(a, b)| a.to_ascii_lowercase() == *b)
            })
    };
    if SIGNAL.iter().any(|n| has(n)) {
        return false;
    }
    NOISE.iter().any(|n| {
        line.len() >= n.len()
            && line[..n.len()]
                .iter()
                .zip(n.iter())
                .all(|(a, b)| a.to_ascii_lowercase() == *b)
    })
}

/// Failures-only shrink over ANSI-free bytes, single pass: keep error/signal
/// lines plus up to 2 trailing context lines after each kept line, drop pure
/// noise. All-noise input shrinks to its first 8 lines (never empty from
/// nonempty input). Pure O(n), no regex, no spawn.
fn shrink(input: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(input.len().min(65536));
    let mut start = 0usize;
    let mut ctx_left = 0usize; // trailing context lines still allowed
    let mut kept_any = false;
    let mut i = 0usize;
    while i < input.len() {
        if input[i] != b'\n' {
            i += 1;
            continue;
        }
        let line = &input[start..=i];
        if is_noise(line) {
            if kept_any && ctx_left > 0 {
                out.extend_from_slice(line);
                ctx_left -= 1;
            }
            // Else: pure noise, dropped.
        } else {
            out.extend_from_slice(line);
            kept_any = true;
            ctx_left = 2;
        }
        start = i + 1;
        i += 1;
    }
    if start < input.len() {
        let rest = &input[start..];
        if !is_noise(rest) {
            out.extend_from_slice(rest);
            kept_any = true;
        } else if kept_any && ctx_left > 0 {
            out.extend_from_slice(rest);
        } else if out.is_empty() {
            // Nothing kept at all: keep the partial tail rather than emitting
            // empty output from nonempty input.
            out.extend_from_slice(rest);
            kept_any = true;
        }
    }
    if !kept_any {
        // All-noise input: shrink to at most the first 8 lines.
        let mut seen = 0usize;
        let mut cut = out.len();
        // `out` is empty here (all noise dropped), so cap the raw input.
        for (k, b) in input.iter().enumerate() {
            if *b == b'\n' {
                seen += 1;
                if seen == 8 {
                    cut = k + 1;
                    break;
                }
            }
        }
        out.extend_from_slice(&input[..cut.min(input.len())]);
    }
    out
}

/// True when `bytes` already carry our truncation marker.
fn has_trunc_marker(bytes: &[u8]) -> bool {
    if !bytes.ends_with(TRUNC_MARKER_SUFFIX.as_bytes()) {
        return false;
    }
    bytes
        .windows(TRUNC_MARKER_PREFIX.len())
        .any(|w| w == TRUNC_MARKER_PREFIX)
}

/// Filter `stdout` for `kind`, preserve `stderr` verbatim and `code`
/// untouched. Order: opt-out alias to [`raw`] (zero shrink work) -> strip
/// ANSI -> failures-only shrink -> byte-budget truncate with marker.
/// Unknown/empty: byte-identical passthrough. No spawn/thread/I-O.
pub fn filter_pass(
    kind: PassKind,
    stdout: &[u8],
    stderr: &[u8],
    code: i32,
    cfg: &RtkConfig,
) -> Filtered {
    if !cfg.enabled || cfg.no_filter || env_no_filter() {
        return raw(stdout, stderr, code);
    }
    if matches!(kind, PassKind::Unknown) || stdout.is_empty() {
        return Filtered {
            stdout: stdout.to_vec(),
            stderr: stderr.to_vec(),
            code,
            truncated: false,
        };
    }
    SHRINK_CALLS.fetch_add(1, Ordering::Relaxed);
    let stripped = strip_ansi(stdout);
    if has_trunc_marker(&stripped) {
        return Filtered {
            stdout: stripped,
            stderr: stderr.to_vec(),
            code,
            truncated: true,
        };
    }
    let shrunk = shrink(&stripped);
    if shrunk.len() <= cfg.max_bytes {
        return Filtered {
            stdout: shrunk,
            stderr: stderr.to_vec(),
            code,
            truncated: false,
        };
    }
    // Over budget: truncate the shrunk middle but keep the tail verdict line.
    // Head keeps the first `head_budget` bytes; the verdict line (when found)
    // is appended after, unless already covered by the head slice.
    let verdict_start = find_verdict(&shrunk);
    let tail_off = verdict_start.unwrap_or(shrunk.len());
    // Head budget leaves room for the verdict tail + marker above the byte
    // budget only by MARKER_MAX (the bound the test asserts).
    let tail_len = shrunk.len().saturating_sub(tail_off);
    let tail_room = tail_len.min(cfg.max_bytes / 4).min(4096);
    let head_budget = cfg.max_bytes.saturating_sub(tail_room);
    let mut keep = floor_char_boundary(&shrunk, head_budget.min(shrunk.len()));
    if keep > tail_off {
        keep = floor_char_boundary(&shrunk, tail_off);
    }
    let tail_full: &[u8] = verdict_start.map(|s| &shrunk[s..]).unwrap_or(b"");
    let tail_emit: &[u8] = if tail_off >= keep {
        // Cap the verdict tail so head + tail <= max_bytes; the verdict token
        // sits at the line start, so keep the head of the tail.
        let cap = tail_room.min(tail_full.len());
        let cap = floor_char_boundary(tail_full, cap);
        &tail_full[..cap]
    } else {
        b""
    };
    // Dropped = bytes elided between head end and tail start.
    let dropped = tail_off.saturating_sub(keep);
    let mut out = Vec::with_capacity(keep + tail_emit.len() + MARKER_MAX);
    out.extend_from_slice(&shrunk[..keep]);
    out.extend_from_slice(tail_emit);
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

/// Byte index of the last line containing `verdict` (case-insensitive).
fn find_verdict(bytes: &[u8]) -> Option<usize> {
    const NEEDLE: &[u8] = b"verdict";
    let mut best = None;
    let mut line_start = 0usize;
    let mut i = 0usize;
    while i <= bytes.len() {
        let end = i == bytes.len() || bytes[i] == b'\n';
        if end {
            let line = &bytes[line_start..i.min(bytes.len())];
            if line.len() >= NEEDLE.len()
                && (0..=line.len() - NEEDLE.len()).any(|s| {
                    line[s..s + NEEDLE.len()]
                        .iter()
                        .zip(NEEDLE.iter())
                        .all(|(a, b)| a.to_ascii_lowercase() == *b)
                })
            {
                best = Some(line_start);
            }
            line_start = i + 1;
        }
        i += 1;
    }
    best
}

/// `proxy`: output byte-identical AND `accounted += lens`; never shrinks,
/// truncates, or marks, even over budget. O(1) extra beyond the copies.
pub fn proxy(stdout: &[u8], stderr: &[u8], code: i32, accounted: &mut usize) -> Filtered {
    *accounted = accounted
        .saturating_add(stdout.len())
        .saturating_add(stderr.len());
    raw(stdout, stderr, code)
}

/// Filter each segment by its own kind; exit = last nonzero code.
pub fn filter_chain(segments: &[Segment], cfg: &RtkConfig) -> (Vec<Filtered>, i32) {
    let mut outs = Vec::with_capacity(segments.len());
    let mut exit = 0;
    for s in segments {
        if s.code != 0 {
            exit = s.code;
        }
        outs.push(filter_pass(s.kind, &s.stdout, &s.stderr, s.code, cfg));
    }
    (outs, exit)
}
