#![forbid(unsafe_code)]
//! HEAD-001 frozen RED tests: headless run with inline/block renderers.
//! Module under test lives at `src/run_headless.rs` (owned by HEAD-001);
//! included by path so no shared lib.rs/Cargo.toml wiring is touched.

#[path = "../src/run_headless.rs"]
mod run_headless;

use run_headless::{
    Attachment, FilePort, ModelError, ModelPort, Renderer, RunExit, RunOpts, RunPrompt, RunSink,
    PREVIEW_MAX_BYTES, PREVIEW_MAX_LINES,
};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

struct FixtureModel {
    answer: String,
    seen_prompt: Vec<u8>,
    refuse: bool,
    fail: bool,
    calls: u64,
}

impl FixtureModel {
    fn answering(answer: &str) -> Self {
        Self {
            answer: answer.to_owned(),
            seen_prompt: Vec::new(),
            refuse: false,
            fail: false,
            calls: 0,
        }
    }
    fn refusing() -> Self {
        Self {
            answer: String::new(),
            seen_prompt: Vec::new(),
            refuse: true,
            fail: false,
            calls: 0,
        }
    }
    fn failing() -> Self {
        Self {
            answer: String::new(),
            seen_prompt: Vec::new(),
            refuse: false,
            fail: true,
            calls: 0,
        }
    }
}

impl ModelPort for FixtureModel {
    fn complete(&mut self, prompt: &[u8]) -> Result<String, ModelError> {
        self.calls += 1;
        self.seen_prompt.clear();
        self.seen_prompt.extend_from_slice(prompt);
        if self.fail {
            return Err(ModelError::Failed);
        }
        if self.refuse {
            return Err(ModelError::Refused);
        }
        Ok(self.answer.clone())
    }
}

struct FixtureFiles {
    map: HashMap<String, Vec<u8>>,
}

impl FixtureFiles {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
    fn with(mut self, path: &str, bytes: &[u8]) -> Self {
        self.map.insert(path.to_owned(), bytes.to_vec());
        self
    }
}

impl FilePort for FixtureFiles {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        self.map.get(path).cloned()
    }
}

struct VecSink {
    buf: Vec<u8>,
}

impl VecSink {
    fn new() -> Self {
        Self { buf: Vec::new() }
    }
    fn with_prior(prior: &[u8]) -> Self {
        Self {
            buf: prior.to_vec(),
        }
    }
}

impl RunSink for VecSink {
    fn push(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }
}

fn prompt(text: &str) -> RunPrompt {
    RunPrompt {
        text: text.to_owned(),
        attachments: Vec::new(),
    }
}

fn opts(renderer: Renderer, dir: &PathBuf) -> RunOpts {
    RunOpts {
        renderer,
        spill_dir: dir.clone(),
    }
}

fn mk_spill_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "opencode-rk-head001-{}-{}",
        std::process::id(),
        name
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create spill dir");
    dir
}

fn spill_list(dir: &PathBuf) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn cleanup(dir: &PathBuf) {
    let _ = fs::remove_dir_all(dir);
}

// HEAD-001-T01 (inline)
#[test]
fn head_001_t01_inline_renders_answer_and_streams_prompt_uncompressed() {
    assert_eq!(PREVIEW_MAX_LINES, 2000);
    assert_eq!(PREVIEW_MAX_BYTES, 51_200);
    let dir = mk_spill_dir("t01");
    let mut model = FixtureModel::answering("hello");
    let files = FixtureFiles::new();
    let mut sink = VecSink::new();
    let p = prompt("hi-fixture");
    let exit: RunExit = run_headless::run(
        "s1",
        &p,
        &mut sink,
        &opts(Renderer::Inline, &dir),
        &mut model,
        &files,
    );
    assert_eq!(exit.code, 0, "want ok, got {exit:?}");
    assert_eq!(sink.buf, b"hello\n", "inline must emit answer plus newline");
    assert_eq!(
        model.seen_prompt,
        b"hi-fixture",
        "model port must observe raw prompt bytes"
    );
    assert!(
        !model.seen_prompt.starts_with(&[0x1f, 0x8b]),
        "prompt must be uncompressed (no gzip magic)"
    );
    assert_eq!(spill_list(&dir).len(), 0, "no spill on small answer");
    cleanup(&dir);
}

// HEAD-001-T02 (block)
#[test]
fn head_001_t02_block_wraps_answer_in_one_fenced_block() {
    let dir = mk_spill_dir("t02");
    let mut model = FixtureModel::answering("hello");
    let files = FixtureFiles::new();
    let mut sink = VecSink::new();
    let exit: RunExit = run_headless::run(
        "s1",
        &prompt("hi-fixture"),
        &mut sink,
        &opts(Renderer::Block, &dir),
        &mut model,
        &files,
    );
    assert_eq!(exit.code, 0, "want ok, got {exit:?}");
    assert_eq!(
        sink.buf,
        b"```\nhello\n```\n",
        "block must wrap answer in one fenced block, byte-exact"
    );
    cleanup(&dir);
}

// HEAD-001-T03 (attachments)
#[test]
fn head_001_t03_text_embeds_file_resolves_escape_rejected() {
    let dir = mk_spill_dir("t03");
    let files = FixtureFiles::new().with("data.bin", b"file-bytes-fixture");
    // Text + File embed alongside the answer.
    let mut model = FixtureModel::answering("hello");
    let mut sink = VecSink::new();
    let p = RunPrompt {
        text: "q-fixture".to_owned(),
        attachments: vec![
            Attachment::Text {
                name: "note.txt".to_owned(),
                text: "attached-text-fixture".to_owned(),
            },
            Attachment::File {
                path: "data.bin".to_owned(),
            },
        ],
    };
    let exit = run_headless::run(
        "s1",
        &p,
        &mut sink,
        &opts(Renderer::Inline, &dir),
        &mut model,
        &files,
    );
    assert_eq!(exit.code, 0, "want ok, got {exit:?}");
    let text = String::from_utf8(sink.buf.clone()).expect("valid UTF-8");
    for want in ["hello", "attached-text-fixture", "file-bytes-fixture"] {
        assert!(text.contains(want), "sink must embed {want}");
    }
    assert_eq!(files.map.len(), 1, "fixtures unchanged");
    // Escape rejected atomically.
    let mut sink2 = VecSink::new();
    let bad = RunPrompt {
        text: "q-fixture".to_owned(),
        attachments: vec![Attachment::File {
            path: "../escape".to_owned(),
        }],
    };
    let before = spill_list(&dir);
    let exit2 = run_headless::run(
        "s1",
        &bad,
        &mut sink2,
        &opts(Renderer::Inline, &dir),
        &mut model,
        &files,
    );
    assert_eq!(exit2.code, 1, "escape must be usage error, got {exit2:?}");
    assert_eq!(sink2.buf.len(), 0, "escape must render zero bytes");
    assert_eq!(spill_list(&dir), before, "no spill file on escape");
    assert_eq!(files.map.len(), 1, "fixtures unchanged on escape");
    cleanup(&dir);
}

// HEAD-001-T04 (oversize spill bounded)
#[test]
fn head_001_t04_sixty_kib_answer_spills_with_bounded_sink() {
    let dir = mk_spill_dir("t04");
    let big = "y".repeat(61_440);
    let mut model = FixtureModel::answering(&big);
    let files = FixtureFiles::new();
    let mut sink = VecSink::new();
    let exit = run_headless::run(
        "s1",
        &prompt("q-fixture"),
        &mut sink,
        &opts(Renderer::Inline, &dir),
        &mut model,
        &files,
    );
    assert_eq!(exit.code, 0, "oversize with spill is success, got {exit:?}");
    assert!(
        (sink.buf.len() as u64) < big.len() as u64,
        "sink must be bounded, got {} vs full {}",
        sink.buf.len(),
        big.len()
    );
    assert!(
        sink.buf.len() <= PREVIEW_MAX_BYTES + 1024,
        "sink head plus marker must stay near cap, got {}",
        sink.buf.len()
    );
    let text = String::from_utf8_lossy(&sink.buf);
    assert!(
        text.contains("truncated"),
        "sink must carry the spill marker"
    );
    let listed = spill_list(&dir);
    assert_eq!(listed.len(), 1, "exactly one spill file, got {listed:?}");
    let spilled = fs::read(dir.join(&listed[0])).expect("read spill file");
    assert_eq!(
        spilled.len(),
        big.len(),
        "spill file must hold the full bytes"
    );
    assert_eq!(spilled, big.as_bytes(), "spill bytes must match answer");
    cleanup(&dir);
}

// HEAD-001-T05 (exit codes)
#[test]
fn head_001_t05_exit_codes_leave_prior_state_untouched_on_failure() {
    let dir = mk_spill_dir("t05");
    let files = FixtureFiles::new();
    // Empty prompt => 1, prior sink bytes and spill listing unchanged.
    let mut model = FixtureModel::answering("hello");
    let mut sink = VecSink::with_prior(b"prior");
    let before_len = sink.buf.len();
    let before_list = spill_list(&dir);
    let exit = run_headless::run(
        "s1",
        &prompt(""),
        &mut sink,
        &opts(Renderer::Inline, &dir),
        &mut model,
        &files,
    );
    assert_eq!(exit.code, 1, "empty prompt must be 1, got {exit:?}");
    assert_eq!(sink.buf, b"prior", "failure must not touch prior sink bytes");
    assert_eq!(spill_list(&dir), before_list, "no spill on usage error");
    // Port failure => 2 with zero partial sink bytes.
    let mut failing = FixtureModel::failing();
    let mut sink2 = VecSink::with_prior(b"prior");
    let exit2 = run_headless::run(
        "s1",
        &prompt("q-fixture"),
        &mut sink2,
        &opts(Renderer::Inline, &dir),
        &mut failing,
        &files,
    );
    assert_eq!(exit2.code, 2, "port failure must be 2, got {exit2:?}");
    assert_eq!(sink2.buf, b"prior", "port failure must emit no partial bytes");
    assert_eq!(spill_list(&dir), before_list, "no spill on port failure");
    // Ok => 0.
    let mut sink3 = VecSink::new();
    let exit3 = run_headless::run(
        "s1",
        &prompt("q-fixture"),
        &mut sink3,
        &opts(Renderer::Inline, &dir),
        &mut model,
        &files,
    );
    assert_eq!(exit3.code, 0, "ok must be 0, got {exit3:?}");
    assert_eq!(before_len, 5);
    cleanup(&dir);
}

// Absolute-path attachment rejected.
#[test]
fn head_001_absolute_path_attachment_is_usage_error() {
    let dir = mk_spill_dir("t06");
    let mut model = FixtureModel::answering("hello");
    let files = FixtureFiles::new().with("ok.txt", b"ok");
    let mut sink = VecSink::new();
    let bad = RunPrompt {
        text: "q-fixture".to_owned(),
        attachments: vec![Attachment::Text {
            name: "/abs.txt".to_owned(),
            text: "x".to_owned(),
        }],
    };
    let exit = run_headless::run(
        "s1",
        &bad,
        &mut sink,
        &opts(Renderer::Inline, &dir),
        &mut model,
        &files,
    );
    assert_eq!(exit.code, 1, "absolute name must be 1, got {exit:?}");
    assert_eq!(sink.buf.len(), 0);
    assert_eq!(model.calls, 0, "rejected before any model call");
    cleanup(&dir);
}

// Spill failure => 2 with zero partial bytes.
#[test]
fn head_001_spill_failure_is_internal_with_no_partial_render() {
    let dir = mk_spill_dir("t07");
    // Occupy the spill path with a regular file so dir creation/write fails.
    let _ = fs::remove_dir_all(&dir);
    fs::write(&dir, b"blocker").expect("write blocker file");
    let blocker = dir.clone();
    let big = "z".repeat(61_440);
    let mut model = FixtureModel::answering(&big);
    let files = FixtureFiles::new();
    let mut sink = VecSink::with_prior(b"prior");
    let exit = run_headless::run(
        "s1",
        &prompt("q-fixture"),
        &mut sink,
        &opts(Renderer::Inline, &blocker),
        &mut model,
        &files,
    );
    assert_eq!(exit.code, 2, "spill failure must be 2, got {exit:?}");
    assert_eq!(sink.buf, b"prior", "spill failure must emit no partial bytes");
    let _ = fs::remove_file(&blocker);
}

// Deterministic bytes across identical calls.
#[test]
fn head_001_same_inputs_give_byte_identical_sink_and_spill() {
    let dir = mk_spill_dir("t08");
    let files = FixtureFiles::new();
    let p = prompt("deterministic-fixture");
    let mut m1 = FixtureModel::answering("stable-answer");
    let mut m2 = FixtureModel::answering("stable-answer");
    let mut s1 = VecSink::new();
    let mut s2 = VecSink::new();
    let e1 = run_headless::run(
        "sess",
        &p,
        &mut s1,
        &opts(Renderer::Block, &dir),
        &mut m1,
        &files,
    );
    // Clear spill dir between runs is NOT done: second run overwrites same
    // deterministic names, so bytes must still match.
    let e2 = run_headless::run(
        "sess",
        &p,
        &mut s2,
        &opts(Renderer::Block, &dir),
        &mut m2,
        &files,
    );
    assert_eq!(e1.code, 0);
    assert_eq!(e2.code, 0);
    assert_eq!(s1.buf, s2.buf, "sink bytes must be byte-identical");
    // Exit messages carry variant text only, never prompt content.
    for exit in [e1, e2] {
        assert!(!exit.message.contains("deterministic-fixture"));
        assert!(!exit.message.contains("stable-answer"));
    }
    cleanup(&dir);
}
