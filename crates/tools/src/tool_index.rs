//! TOOL-016 native codebase index/snapshot: bounded local-only scan.
//!
//! Single-scan file list (relative path, size, SHA-256 hex) plus a symbol
//! table (name, kind, file, line span) for one workspace root. Serves
//! file/symbol lookup without re-walking the tree. Default-on with opt-out
//! ([`IndexConfig::disabled`]); zero filesystem I/O when off. No network, no
//! DB writes, no logging of file contents (bodies are hashed then dropped).

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Chunk size for streaming file-body hashes (64 KiB per contract).
const HASH_CHUNK: usize = 64 * 1024;
/// Upper bound on bytes retained in memory to parse symbols from one file.
/// Bodies are dropped after hashing; this only bounds the transient parse
/// buffer so a giant `*.rs` cannot OOM the scan.
const PARSE_MAX_BYTES: u64 = 8 * 1024 * 1024;

/// Bounded scan budget. `Default` is default-on:
/// `enabled: true, max_files: 20000, max_bytes: 200_000_000,
/// max_symbols: 100000`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexConfig {
    pub enabled: bool,
    pub max_files: usize,
    pub max_bytes: u64,
    pub max_symbols: usize,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_files: 20_000,
            max_bytes: 200_000_000,
            max_symbols: 100_000,
        }
    }
}

impl IndexConfig {
    /// Opt-out: scan nothing, touch nothing.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            max_files: 0,
            max_bytes: 0,
            max_symbols: 0,
        }
    }
}

/// One indexed file: path relative to the snapshot root (lossy UTF-8 label
/// for display; lookup uses the same label), byte size, hex content hash.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub hash: String,
}

/// One indexed symbol: name, kind, owning file (relative path), 1-based line
/// span.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
}

/// Symbol kinds covered by the line parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SymbolKind {
    Fn,
    Struct,
    Enum,
    Trait,
    Mod,
    Const,
}

/// Point-in-time snapshot of one workspace root. `scanned_at_ms` is
/// informational only and never affects ordering. `skipped` counts
/// unreadable single files (permissions, broken symlinks) that did not abort
/// the scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub root: PathBuf,
    pub files: Vec<FileEntry>,
    pub symbols: Vec<Symbol>,
    pub scanned_at_ms: u64,
    pub truncated: bool,
    pub skipped: u32,
}

/// Index failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IndexError {
    #[error("root unreadable")]
    RootUnreadable,
    #[error("cancelled")]
    Cancelled,
}

/// SHA-256 hex of `bytes` (same streaming algorithm as the scanner; bodies
/// are never retained, only this digest).
#[must_use]
pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex_digest(&h.finish())
}

/// Summed admitted `size` over snapshot files.
#[must_use]
pub fn total_bytes(snap: &Snapshot) -> u64 {
    snap.files.iter().map(|f| f.size).sum()
}

/// Build a snapshot of `root` in one bounded scan.
pub fn build_snapshot(root: &Path, cfg: &IndexConfig) -> Result<Snapshot, IndexError> {
    build_inner(root, cfg, None)
}

/// Build a snapshot, aborting promptly with [`IndexError::Cancelled`] when
/// `cancel` is set. The flag is checked before start and at every directory
/// and file step; no thread is spawned, the caller owns the result.
pub fn build_snapshot_cancel(
    root: &Path,
    cfg: &IndexConfig,
    cancel: &AtomicBool,
) -> Result<Snapshot, IndexError> {
    build_inner(root, cfg, Some(cancel))
}

/// Exact-name file lookup by relative path label.
#[must_use]
pub fn lookup_file<'a>(snap: &'a Snapshot, path: &str) -> Option<&'a FileEntry> {
    snap.files.iter().find(|f| f.path == path)
}

/// Exact-name symbol lookup, sorted by (file, start_line).
pub fn lookup_symbol<'a>(snap: &'a Snapshot, name: &str) -> Vec<&'a Symbol> {
    let mut hits: Vec<&'a Symbol> = snap.symbols.iter().filter(|s| s.name == name).collect();
    hits.sort_by(|a, b| {
        (a.file.clone(), a.start_line).cmp(&(b.file.clone(), b.start_line))
    });
    hits
}

fn cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.is_some_and(|c| c.load(Ordering::Relaxed))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn build_inner(
    root: &Path,
    cfg: &IndexConfig,
    cancel: Option<&AtomicBool>,
) -> Result<Snapshot, IndexError> {
    if !cfg.enabled {
        return Ok(Snapshot {
            root: root.to_path_buf(),
            files: Vec::new(),
            symbols: Vec::new(),
            scanned_at_ms: now_ms(),
            truncated: false,
            skipped: 0,
        });
    }
    if cancelled(cancel) {
        return Err(IndexError::Cancelled);
    }
    if !is_readable_dir(root) {
        return Err(IndexError::RootUnreadable);
    }
    let mut snap = Snapshot {
        root: root.to_path_buf(),
        files: Vec::new(),
        symbols: Vec::new(),
        scanned_at_ms: now_ms(),
        truncated: false,
        skipped: 0,
    };
    let mut admitted_bytes: u64 = 0;
    // Bounded depth-first work stack; cap-checked before push is enforced by
    // stopping admission once caps are hit (vectors never exceed caps).
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if cancelled(cancel) {
            return Err(IndexError::Cancelled);
        }
        let entries = match sorted_dir_entries(&dir) {
            Ok(e) => e,
            Err(()) => {
                snap.skipped = snap.skipped.saturating_add(1);
                continue;
            }
        };
        for path in entries {
            if cancelled(cancel) {
                return Err(IndexError::Cancelled);
            }
            let rel = match path.strip_prefix(root).map(rel_label) {
                Ok(r) => r,
                Err(_) => {
                    snap.skipped = snap.skipped.saturating_add(1);
                    continue;
                }
            };
            let ftype = match std::fs::symlink_metadata(&path).map(|m| m.file_type()) {
                Ok(t) => t,
                Err(_) => {
                    snap.skipped = snap.skipped.saturating_add(1);
                    continue;
                }
            };
            if ftype.is_dir() {
                stack.push(path);
                continue;
            }
            if !ftype.is_file() {
                // Symlinks (broken or not), sockets, etc: skip, never follow.
                snap.skipped = snap.skipped.saturating_add(1);
                continue;
            }
            if snap.files.len() >= cfg.max_files {
                snap.truncated = true;
                return Ok(finish(snap));
            }
            match hash_file_streaming(&path) {
                Ok((size, hash)) => {
                    if admitted_bytes.saturating_add(size) > cfg.max_bytes {
                        snap.truncated = true;
                        return Ok(finish(snap));
                    }
                    if snap.files.len() >= cfg.max_files {
                        snap.truncated = true;
                        return Ok(finish(snap));
                    }
                    admitted_bytes = admitted_bytes.saturating_add(size);
                    snap.files.push(FileEntry {
                        path: rel.clone(),
                        size,
                        hash,
                    });
                    if path.extension().is_some_and(|e| e == "rs")
                        && size <= PARSE_MAX_BYTES
                        && snap.symbols.len() < cfg.max_symbols
                    {
                        let mut added_truncated = false;
                        extract_symbols(&path, &rel, cfg, &mut snap, &mut added_truncated);
                        if added_truncated {
                            snap.truncated = true;
                            return Ok(finish(snap));
                        }
                    }
                }
                Err(()) => {
                    snap.skipped = snap.skipped.saturating_add(1);
                }
            }
        }
    }
    Ok(finish(snap))
}

/// Deterministic order: files by path; symbols by (file, line, name).
fn finish(mut snap: Snapshot) -> Snapshot {
    snap.files.sort();
    snap.symbols.sort_by(|a, b| {
        (
            a.file.clone(),
            a.start_line,
            a.name.clone(),
            a.kind,
            a.end_line,
        )
            .cmp(&(b.file.clone(), b.start_line, b.name.clone(), b.kind, b.end_line))
    });
    snap.scanned_at_ms = now_ms();
    snap
}

fn is_readable_dir(root: &Path) -> bool {
    std::fs::metadata(root)
        .map(|m| m.is_dir())
        .unwrap_or(false)
        && std::fs::read_dir(root).is_ok()
}

/// Sorted directory listing (by lossy file name, then full path) so the scan
/// order is deterministic for identical trees.
fn sorted_dir_entries(dir: &Path) -> Result<Vec<PathBuf>, ()> {
    let rd = std::fs::read_dir(dir).map_err(|_| ())?;
    let mut out = Vec::new();
    for e in rd {
        match e {
            Ok(entry) => out.push(entry.path()),
            Err(_) => return Err(()),
        }
    }
    out.sort_by(|a, b| {
        (file_label(a), a).cmp(&(file_label(b), b))
    });
    Ok(out)
}

/// Lossy display label for a path (non-UTF8 paths keep scanning; the label
/// is display-only).
fn rel_label(rel: &Path) -> String {
    rel.to_string_lossy().into_owned()
}

fn file_label(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Stream a file in 64 KiB chunks, returning (size, sha256 hex). Bodies are
/// dropped after hashing. Any I/O failure (permissions, broken symlink,
/// mid-read error) reports `Err(())` so the caller skips the entry.
fn hash_file_streaming(path: &Path) -> Result<(u64, String), ()> {
    let mut f = File::open(path).map_err(|_| ())?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; HASH_CHUNK];
    let mut size: u64 = 0;
    loop {
        let n = f.read(&mut buf).map_err(|_| ())?;
        if n == 0 {
            break;
        }
        size = size.saturating_add(n as u64);
        h.update(&buf[..n]);
    }
    Ok((size, hex_digest(&h.finish())))
}

/// Parse `*.rs` symbol definitions line-by-line. Never logs bodies; pushes
/// at most up to `cfg.max_symbols` and signals cap-truncation via
/// `hit_symbol_cap`.
fn extract_symbols(
    path: &Path,
    rel: &str,
    cfg: &IndexConfig,
    snap: &mut Snapshot,
    hit_symbol_cap: &mut bool,
) {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return,
    };
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if snap.symbols.len() >= cfg.max_symbols {
            *hit_symbol_cap = true;
            return;
        }
        if let Some((kind, name)) = parse_symbol_decl(lines[i]) {
            let start = (i + 1) as u32;
            let end = brace_end(&lines, i) as u32;
            snap.symbols.push(Symbol {
                name,
                kind,
                file: rel.to_string(),
                start_line: start,
                end_line: end,
            });
        }
        i += 1;
    }
}

/// Match one Rust item declaration line: optional visibility prefix
/// (`pub`, `pub(...)`, with arbitrary whitespace), then one of
/// `fn|struct|enum|trait|mod|const`, then the item identifier. Returns
/// (kind, name). Operates on the declaration line only; no bodies retained.
fn parse_symbol_decl(line: &str) -> Option<(SymbolKind, String)> {
    let mut rest = line.trim_start();
    if rest.starts_with("pub") {
        let after = &rest[3..];
        if after.starts_with('(') {
            let close = after.find(')')?;
            rest = after[close + 1..].trim_start();
        } else if after.is_empty() || after.starts_with(char::is_whitespace) {
            rest = after.trim_start();
        } else {
            return None;
        }
    }
    // `unsafe` / `extern` / `async` prefixes before the item keyword.
    for prefix in ["unsafe ", "extern ", "async "] {
        if let Some(s) = rest.strip_prefix(prefix) {
            rest = s.trim_start();
            if rest.starts_with('"') {
                // `extern "C" fn ...`: skip the ABI string.
                let end = rest[1..].find('"')? + 2;
                rest = rest[end..].trim_start();
            }
        }
    }
    let (kind, after_kw) = [
        ("fn", SymbolKind::Fn),
        ("struct", SymbolKind::Struct),
        ("enum", SymbolKind::Enum),
        ("trait", SymbolKind::Trait),
        ("mod", SymbolKind::Mod),
        ("const", SymbolKind::Const),
    ]
    .into_iter()
    .find_map(|(kw, kind)| {
        rest.strip_prefix(kw).filter(|after| {
            after.is_empty()
                || after
                    .chars()
                    .next()
                    .is_none_or(|c| !is_ident_char(c))
        }).map(|after| (kind, after))
    })?;
    let name: String = after_kw
        .trim_start()
        .chars()
        .take_while(|c| is_ident_char(*c))
        .collect();
    if name.is_empty() || name == "_" {
        return None;
    }
    Some((kind, name))
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// End line (1-based) of the item starting at `decl_idx`: brace-balanced
/// scan for block items, otherwise the declaration line itself. Bounded by
/// the file length.
fn brace_end(lines: &[&str], decl_idx: usize) -> usize {
    let decl = lines[decl_idx];
    let mut depth: i32 = 0;
    let mut seen_open = false;
    for (offset, line) in lines[decl_idx..].iter().enumerate() {
        let code = strip_line_comment(line);
        for c in code.chars() {
            if c == '{' {
                depth += 1;
                seen_open = true;
            } else if c == '}' {
                depth -= 1;
            }
        }
        let _ = decl;
        if seen_open && depth <= 0 {
            return decl_idx + offset + 1;
        }
        if !seen_open && offset > 0 {
            return decl_idx + 1;
        }
        if offset > 0 && !seen_open && line.trim_end().ends_with(';') {
            return decl_idx + offset + 1;
        }
    }
    if seen_open {
        lines.len()
    } else {
        decl_idx + 1
    }
}

fn strip_line_comment(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

// --- Minimal SHA-256 (FIPS 180-4), std-only: real content hashes with no new
// --- dependencies (lane may not touch Cargo.toml).

struct Sha256 {
    h: [u32; 8],
    buf: [u8; 64],
    buf_len: usize,
    total_len: u64,
}

impl Sha256 {
    fn new() -> Self {
        Self {
            h: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c,
                0x1f83d9ab, 0x5be0cd19,
            ],
            buf: [0u8; 64],
            buf_len: 0,
            total_len: 0,
        }
    }

    fn update(&mut self, mut data: &[u8]) {
        self.total_len = self.total_len.saturating_add(data.len() as u64);
        while !data.is_empty() {
            let take = (64 - self.buf_len).min(data.len());
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];
            if self.buf_len == 64 {
                let block = self.buf;
                Self::compress(&mut self.h, &block);
                self.buf_len = 0;
            }
        }
    }

    fn finish(mut self) -> [u8; 32] {
        let bit_len = self.total_len.wrapping_mul(8);
        self.update(&[0x80]);
        while self.buf_len != 56 {
            if self.buf_len > 56 {
                while self.buf_len != 64 {
                    let len = self.buf_len;
                    self.buf[len] = 0;
                    self.buf_len += 1;
                }
                let block = self.buf;
                Self::compress(&mut self.h, &block);
                self.buf_len = 0;
                self.buf = [0u8; 64];
            } else {
                let len = self.buf_len;
                self.buf[len] = 0;
                self.buf_len += 1;
            }
        }
        let mut tail = [0u8; 8];
        tail.copy_from_slice(&bit_len.to_be_bytes());
        for b in tail {
            let len = self.buf_len;
            self.buf[len] = b;
            self.buf_len += 1;
        }
        debug_assert_eq!(self.buf_len, 64);
        let block = self.buf;
        Self::compress(&mut self.h, &block);
        let mut out = [0u8; 32];
        for (i, w) in self.h.iter().enumerate() {
            out[4 * i..4 * i + 4].copy_from_slice(&w.to_be_bytes());
        }
        out
    }

    fn compress(h: &mut [u32; 8], block: &[u8; 64]) {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1,
            0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
            0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
            0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
            0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
            0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
            0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
            0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
            0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
            0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
        ];
        let mut w = [0u32; 64];
        for (i, chunk) in block.chunks_exact(4).enumerate().take(16) {
            w[i] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] =
            [h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]];
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
}

fn hex_digest(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(64);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0xf) as usize] as char);
    }
    s
}
