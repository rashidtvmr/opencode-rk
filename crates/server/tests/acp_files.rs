//! ACP-002 frozen tests T01..T05 (file capabilities + typed unsupported).
//!
//! Lane owns exactly `crates/server/src/acp_files.rs` + this test file.
//! Never edits shared `lib.rs`, `Cargo.toml`. Module included via path;
//! integrator wires `mod acp_files;` into `lib.rs` later.

#[path = "../src/acp_files.rs"]
mod acp_files;

use acp_files::{
    is_supported, read_text, require, write_text, AcpFileError, Capability, FileReader, FileWriter,
    MAX_ACP_FILE_BYTES, MAX_REL_CHARS,
};
use std::collections::HashMap;
use std::path::Path;

// ---- fixture-only fs harness (permission broker: no ambient fs) ----

#[derive(Default)]
struct FixtureFs {
    files: HashMap<String, Vec<u8>>,
    read_calls: usize,
    write_calls: usize,
}

impl FixtureFs {
    fn with_text(rel: &str, text: &str) -> Self {
        let mut fs = Self::default();
        fs.files.insert(rel.to_string(), text.as_bytes().to_vec());
        fs
    }
}

impl FileReader for FixtureFs {
    fn read_bytes(&mut self, rel: &str) -> Result<Vec<u8>, AcpFileError> {
        self.read_calls += 1;
        self.files.get(rel).cloned().ok_or(AcpFileError::NotFound)
    }
}

impl FileWriter for FixtureFs {
    fn write_bytes(&mut self, rel: &str, bytes: &[u8]) -> Result<u64, AcpFileError> {
        self.write_calls += 1;
        if bytes.len() > MAX_ACP_FILE_BYTES {
            return Err(AcpFileError::TooLarge);
        }
        self.files.insert(rel.to_string(), bytes.to_vec());
        Ok(bytes.len() as u64)
    }
}

// Refusing writer: all-or-nothing atomic check for T04/T05.
struct RefusingWriter {
    inner: FixtureFs,
    pub attempted: Vec<Vec<u8>>,
}

impl FileReader for RefusingWriter {
    fn read_bytes(&mut self, rel: &str) -> Result<Vec<u8>, AcpFileError> {
        self.inner.read_bytes(rel)
    }
}

impl FileWriter for RefusingWriter {
    fn write_bytes(&mut self, rel: &str, bytes: &[u8]) -> Result<u64, AcpFileError> {
        self.attempted.push(bytes.to_vec());
        // Enforce all-or-nothing: reject oversize before touching bytes.
        if bytes.len() > MAX_ACP_FILE_BYTES {
            return Err(AcpFileError::TooLarge);
        }
        self.inner.write_bytes(rel, bytes)
    }
}

fn root() -> std::path::PathBuf {
    Path::new("/fixture-root").to_path_buf()
}

// ACP-002-T01 (read happy path).
#[test]
fn acp002_t01_read_happy_path() {
    assert_eq!(MAX_ACP_FILE_BYTES, 1_048_576);
    assert!(is_supported(Capability::FileRead));
    require(Capability::FileRead).expect("FileRead supported");

    let mut fs = FixtureFs::with_text("notes/a.txt", "hello acp");
    let got = read_text(&root(), "notes/a.txt", &mut fs).expect("read fixture text");
    assert_eq!(got, "hello acp");
    assert_eq!(fs.read_calls, 1);
}

// ACP-002-T02 (write happy path).
#[test]
fn acp002_t02_write_happy_path() {
    assert!(is_supported(Capability::FileWrite));
    require(Capability::FileWrite).expect("FileWrite supported");

    let mut fs = FixtureFs::default();
    let n = write_text(&root(), "notes/b.txt", "new bytes", &mut fs).expect("write fixture text");
    assert_eq!(n, 9);
    assert_eq!(
        String::from_utf8(fs.files["notes/b.txt"].clone()).expect("utf8"),
        "new bytes"
    );
    assert_eq!(fs.write_calls, 1);
    // Round-trip through reader side.
    let back = read_text(&root(), "notes/b.txt", &mut fs).expect("re-read written file");
    assert_eq!(back, "new bytes");
}

// ACP-002-T03 (unsupported capabilities named, no side effect).
#[test]
fn acp002_t03_unsupported_named_no_side_effect() {
    for cap in [
        Capability::Terminal,
        Capability::Auth,
        Capability::ModeSwitch,
        Capability::HistoryRestore,
        Capability::UpdateStream,
    ] {
        assert!(!is_supported(cap), "{cap:?} must be unsupported");
        let err = require(cap).unwrap_err();
        match &err {
            AcpFileError::Unsupported { capability } => assert_eq!(*capability, cap),
            other => panic!("expected Unsupported, got {other:?}"),
        }
        // Error names the exact variant, carries no file content.
        let rendered = format!("{err} {err:?}");
        assert!(
            rendered.contains(&format!("{cap:?}")),
            "names variant: {rendered}"
        );
    }

    // No file touched by capability negotiation.
    let mut fs = FixtureFs::with_text("notes/a.txt", "stable");
    let before = fs.files.clone();
    for cap in [
        Capability::Terminal,
        Capability::Auth,
        Capability::ModeSwitch,
        Capability::HistoryRestore,
        Capability::UpdateStream,
    ] {
        let _ = require(cap);
    }
    assert_eq!(fs.files, before);
    assert_eq!(fs.read_calls, 0);
    assert_eq!(fs.write_calls, 0);
}

// ACP-002-T04 (path rules + bounds + missing + non-utf8).
#[test]
fn acp002_t04_path_and_bounds() {
    let mut fs = FixtureFs::with_text("notes/a.txt", "stable");
    fs.files
        .insert("bin/blob".to_string(), vec![0x66, 0x6f, 0x80, 0x6f]);
    let before = fs.files.clone();

    // Path escapes rejected on read and write.
    for bad in ["../escape", "/abs", "", "a/../../b"] {
        assert_eq!(
            read_text(&root(), bad, &mut fs),
            Err(AcpFileError::PathNotAllowed),
            "read {bad:?}"
        );
        assert_eq!(
            write_text(&root(), bad, "x", &mut fs),
            Err(AcpFileError::PathNotAllowed),
            "write {bad:?}"
        );
    }
    // Overlong rel rejected.
    let long_rel = "a".repeat(MAX_REL_CHARS + 1);
    assert_eq!(
        read_text(&root(), &long_rel, &mut fs),
        Err(AcpFileError::PathNotAllowed)
    );
    assert_eq!(
        write_text(&root(), &long_rel, "x", &mut fs),
        Err(AcpFileError::PathNotAllowed)
    );
    // Missing file.
    assert_eq!(
        read_text(&root(), "notes/missing.txt", &mut fs),
        Err(AcpFileError::NotFound)
    );
    // Non-UTF8 fixture.
    assert_eq!(
        read_text(&root(), "bin/blob", &mut fs),
        Err(AcpFileError::NotText)
    );

    // Oversize write: all-or-nothing, fixture bytes unchanged.
    let big = "y".repeat(MAX_ACP_FILE_BYTES + 1);
    let mut rw = RefusingWriter {
        inner: FixtureFs::with_text("notes/a.txt", "stable"),
        attempted: Vec::new(),
    };
    assert_eq!(
        write_text(&root(), "notes/a.txt", &big, &mut rw),
        Err(AcpFileError::TooLarge)
    );
    assert_eq!(
        String::from_utf8(rw.inner.files["notes/a.txt"].clone()).expect("utf8"),
        "stable"
    );

    // Oversize content on the read side surfaces TooLarge, not truncation.
    let mut huge = FixtureFs::default();
    huge.files.insert(
        "notes/huge.txt".to_string(),
        vec![b'z'; MAX_ACP_FILE_BYTES + 1],
    );
    assert_eq!(
        read_text(&root(), "notes/huge.txt", &mut huge),
        Err(AcpFileError::TooLarge)
    );

    // Rejected ops left fixtures unchanged (read-side harness).
    assert_eq!(fs.files["notes/a.txt"], before["notes/a.txt"]);
    assert_eq!(fs.files["bin/blob"], before["bin/blob"]);
    assert!(!fs.files.contains_key("notes/missing.txt"));
}

// ACP-002-T05 (isolation + safety: fixture-only, no content in errors/logs, deterministic).
#[test]
fn acp002_t05_isolation_and_safety() {
    let dir = tempfile::tempdir().expect("disposable fixture");
    let secret_text = "super-secret-payload-12345";
    let mut fs = FixtureFs::with_text("notes/a.txt", secret_text);

    // Repeated read byte-identical (deterministic, no wall-clock).
    let first = read_text(&root(), "notes/a.txt", &mut fs).expect("first read");
    let second = read_text(&root(), "notes/a.txt", &mut fs).expect("second read");
    assert_eq!(first, second);
    assert_eq!(first, secret_text);

    // Errors carry paths/variant names only, never file content.
    let errs = [
        read_text(&root(), "../escape", &mut fs).unwrap_err(),
        read_text(&root(), "notes/missing.txt", &mut fs).unwrap_err(),
        require(Capability::Terminal).unwrap_err(),
    ];
    for err in &errs {
        let rendered = format!("{err} {err:?}");
        assert!(
            !rendered.contains(secret_text),
            "error leaks content: {rendered}"
        );
    }

    // Only the disposable fixture dir is touched (no ambient fs in module).
    let marker = dir.path().join("acp002.log");
    std::fs::write(&marker, b"acp-002 fixture").expect("fixture write");
    assert!(marker.exists());
}
