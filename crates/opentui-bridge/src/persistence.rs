#![forbid(unsafe_code)]
//! Crash-safe text persistence (mirrors `util/persistence.ts:1-33` @ a0d9b6c).
//!
//! TS notes: `readText` = `Bun.file.text()` (no cap, throws ENOENT on
//! missing); `writeText` = `mkdir recursive` + `Bun.write` (NOT atomic,
//! partial file on crash); only `writeJsonAtomic` (ts:22-33) is tmp+rename
//! atomic with pid+uuid tmp name and tmp cleanup on failure. `appendText`
//! (ts:17-20) = `mkdir recursive` + `appendFile`. Callers: prompt/history.tsx:60,104,107,
//! prompt/frecency.tsx:52,61,68, prompt/stash.tsx:41,64,67,73,82 (dialog-stash
//! JSONL files), context/kv.tsx:58 + local.tsx:175,431 (`writeJsonAtomic`).
//! Epilogue path: `context/epilogue.tsx:1-6` holds an in-memory setter only;
//! the durable write is `app.tsx:361` `process.stdout.write(epilogue+"\n")`.
//! This module hardens all writes to tmp+rename (upgrade over TS `writeText`)
//! and caps reads/writes at 1 MiB. std only, no serde (`JsonShape` is a
//! brace/bracket sniff, not a parser).

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Max bytes accepted for a single read or write (1 MiB).
pub const MAX_READ_BYTES: usize = 1024 * 1024;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Persistence failure modes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistError {
    Io(String),
    TooLarge,
    InvalidUtf8,
    JsonShape(String),
}

impl fmt::Display for PersistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io: {e}"),
            Self::TooLarge => write!(f, "exceeds {MAX_READ_BYTES} bytes"),
            Self::InvalidUtf8 => write!(f, "invalid utf-8"),
            Self::JsonShape(e) => write!(f, "json shape: {e}"),
        }
    }
}

impl std::error::Error for PersistError {}

fn io_err(e: std::io::Error) -> PersistError {
    PersistError::Io(e.to_string())
}

fn tmp_path(path: &Path) -> std::path::PathBuf {
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let stem = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "persist".to_string());
    path.with_file_name(format!("{stem}.{}.{n}.tmp", std::process::id()))
}

/// Read whole file as text. Missing file -> `Io`; over cap -> `TooLarge`.
pub fn read_text(path: &Path) -> Result<String, PersistError> {
    let meta = fs::metadata(path).map_err(io_err)?;
    if meta.len() > MAX_READ_BYTES as u64 {
        return Err(PersistError::TooLarge);
    }
    let bytes = fs::read(path).map_err(io_err)?;
    if bytes.len() > MAX_READ_BYTES {
        return Err(PersistError::TooLarge);
    }
    String::from_utf8(bytes).map_err(|_| PersistError::InvalidUtf8)
}

/// Read file and sniff JSON object/array shape (no serde; std only).
pub fn read_json_raw(path: &Path) -> Result<String, PersistError> {
    let text = read_text(path)?;
    let t = text.trim_start();
    if t.starts_with('{') || t.starts_with('[') {
        Ok(text)
    } else {
        Err(PersistError::JsonShape(format!(
            "expected '{{' or '[', got {:?}",
            t.chars().next()
        )))
    }
}

/// Atomic write: `create_dir_all` parent, write tmp in same dir, rename.
/// Oversize content errors BEFORE touching disk (no partial file).
pub fn write_text(path: &Path, content: &str) -> Result<(), PersistError> {
    if content.len() > MAX_READ_BYTES {
        return Err(PersistError::TooLarge);
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
    }
    let tmp = tmp_path(path);
    let write_res = (|| -> Result<(), PersistError> {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(io_err)?;
        f.write_all(content.as_bytes()).map_err(io_err)?;
        f.sync_all().map_err(io_err)?;
        drop(f);
        fs::rename(&tmp, path).map_err(io_err)?;
        Ok(())
    })();
    if write_res.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    write_res
}

/// Append one line + `\n` (mirrors TS `appendText`; history/stash JSONL use).
pub fn append_line(path: &Path, line: &str) -> Result<(), PersistError> {
    if line.len() + 1 > MAX_READ_BYTES {
        return Err(PersistError::TooLarge);
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
    }
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(io_err)?;
    f.write_all(line.as_bytes()).map_err(io_err)?;
    f.write_all(b"\n").map_err(io_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_file(tag: &str) -> std::path::PathBuf {
        let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "bridge041-{tag}-{}.{n}.txt",
            std::process::id()
        ))
    }

    #[test]
    fn roundtrip() {
        let p = tmp_file("roundtrip");
        write_text(&p, "hello").unwrap();
        assert_eq!(read_text(&p).unwrap(), "hello");
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn write_over_cap_errs_before_touching_disk() {
        let p = tmp_file("bigwrite");
        let big = "x".repeat(MAX_READ_BYTES + 1);
        assert_eq!(write_text(&p, &big), Err(PersistError::TooLarge));
        assert!(!p.exists(), "oversize write must not create file");
    }

    #[test]
    fn read_over_cap_errs() {
        let p = tmp_file("bigread");
        let big = vec![b'y'; MAX_READ_BYTES + 8];
        fs::write(&p, &big).unwrap();
        assert_eq!(read_text(&p), Err(PersistError::TooLarge));
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn missing_file_errs_io() {
        let p = tmp_file("missing");
        let _ = fs::remove_file(&p);
        assert!(matches!(read_text(&p), Err(PersistError::Io(_))));
    }

    #[test]
    fn failed_write_leaves_original_intact() {
        let p = tmp_file("atomic");
        write_text(&p, "original").unwrap();
        let big = "z".repeat(MAX_READ_BYTES + 1);
        assert_eq!(write_text(&p, &big), Err(PersistError::TooLarge));
        assert_eq!(read_text(&p).unwrap(), "original");
        let stray = p.with_extension("tmp");
        assert!(!stray.exists());
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn append_line_adds_newline() {
        let p = tmp_file("append");
        let _ = fs::remove_file(&p);
        append_line(&p, r#"{"a":1}"#).unwrap();
        append_line(&p, r#"{"a":2}"#).unwrap();
        assert_eq!(read_text(&p).unwrap(), "{\"a\":1}\n{\"a\":2}\n");
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn invalid_utf8_errs() {
        let p = tmp_file("utf8");
        fs::write(&p, [0xff, 0xfe]).unwrap();
        assert_eq!(read_text(&p), Err(PersistError::InvalidUtf8));
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn json_shape_sniff() {
        let p = tmp_file("json");
        write_text(&p, r#"{"k":1}"#).unwrap();
        assert!(read_json_raw(&p).is_ok());
        write_text(&p, "plain text").unwrap();
        assert!(matches!(
            read_json_raw(&p),
            Err(PersistError::JsonShape(_))
        ));
        let _ = fs::remove_file(&p);
    }
}
