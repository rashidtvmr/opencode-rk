#![forbid(unsafe_code)]
//! HEAD-001: headless run with inline/block renderers.
//!
//! Caller-owned ports only; no filesystem access except the caller spill dir,
//! no network, no thread, no statics, no cache. Failed runs emit zero sink
//! bytes (atomic render per answer).

/// Max preview lines rendered per answer/attachment before spill.
pub const PREVIEW_MAX_LINES: usize = 2000;
/// Max preview bytes rendered per answer/attachment before spill (50 KiB).
pub const PREVIEW_MAX_BYTES: usize = 51_200;
/// Max prompt chars (1..=65_536).
const MAX_PROMPT_CHARS: usize = 65_536;
/// Max session chars (1..=128).
const MAX_SESSION_CHARS: usize = 128;
/// Max attachment name/path chars (1..=512).
const MAX_ATTACH_CHARS: usize = 512;
/// Max text attachment bytes (256 KiB).
const MAX_TEXT_BYTES: usize = 262_144;

/// Answer renderer: Inline lines or one fenced Block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Renderer {
    Inline,
    Block,
}

/// Prompt attachment: inline text or fixture-dir file reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attachment {
    Text { name: String, text: String },
    File { path: String },
}

/// Headless prompt: text plus attachments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunPrompt {
    pub text: String,
    pub attachments: Vec<Attachment>,
}

/// Run options: caller renderer plus caller-owned spill dir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOpts {
    pub renderer: Renderer,
    pub spill_dir: std::path::PathBuf,
}

/// Typed exit: 0 ok, 1 usage/model, 2 internal/limit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunExit {
    pub code: u8,
    pub message: String,
}

/// Model-port failures: refusal is usage (1), failure is internal (2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelError {
    Refused,
    Failed,
}

/// Caller-owned model port. Receives raw prompt bytes (uncompressed).
pub trait ModelPort {
    fn complete(&mut self, prompt: &[u8]) -> Result<String, ModelError>;
}

/// Caller-owned file port over a disposable fixture dir.
pub trait FilePort {
    fn read(&self, path: &str) -> Option<Vec<u8>>;
}

/// Caller-owned byte sink. Receives at most one write per call.
pub trait RunSink {
    fn push(&mut self, data: &[u8]);
}

fn usage(msg: &str) -> RunExit {
    RunExit {
        code: 1,
        message: msg.to_owned(),
    }
}

fn internal(msg: &str) -> RunExit {
    RunExit {
        code: 2,
        message: msg.to_owned(),
    }
}

fn ok(msg: &str) -> RunExit {
    RunExit {
        code: 0,
        message: msg.to_owned(),
    }
}

/// Reject `..`, absolute, empty, overlong attachment paths/names.
fn valid_name(s: &str) -> bool {
    if s.is_empty() || s.chars().count() > MAX_ATTACH_CHARS {
        return false;
    }
    if s.starts_with('/') || s.starts_with('\\') {
        return false;
    }
    if s.contains("..") {
        return false;
    }
    true
}

/// Render one text section; spill oversize to caller dir.
/// Returns (rendered_bytes, spill_path_if_any).
fn preview_or_spill(
    text: &str,
    session: &str,
    tag: &str,
    spill_dir: &std::path::Path,
) -> Result<(Vec<u8>, bool), RunExit> {
    let bytes = text.as_bytes();
    let over_lines = text.lines().count() > PREVIEW_MAX_LINES;
    let over_bytes = bytes.len() > PREVIEW_MAX_BYTES;
    if !over_lines && !over_bytes {
        return Ok((bytes.to_vec(), false));
    }
    // Head bounded by both caps.
    let mut head: &[u8] = bytes;
    if head.len() > PREVIEW_MAX_BYTES {
        head = &head[..PREVIEW_MAX_BYTES];
        // Snap back to a char boundary.
        let mut len = head.len();
        while len > 0 && !text.is_char_boundary(len) {
            len -= 1;
        }
        head = &head[..len];
    }
    let head_str = String::from_utf8_lossy(head);
    let mut lines: Vec<&str> = head_str.lines().collect();
    if lines.len() > PREVIEW_MAX_LINES {
        lines.truncate(PREVIEW_MAX_LINES);
    }
    let head_text = lines.join("\n");
    let head_bytes = head_text.as_bytes();
    let spilled_extra = bytes.len().saturating_sub(head_bytes.len());
    let spill_name = format!("{session}-{tag}.spill");
    let spill_path = spill_dir.join(&spill_name);
    if let Some(parent) = spill_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return Err(internal(&format!("spill failed: {e}")));
        }
    }
    if let Err(e) = std::fs::write(&spill_path, bytes) {
        return Err(internal(&format!("spill failed: {e}")));
    }
    let mut out = Vec::with_capacity(head_bytes.len() + 128);
    out.extend_from_slice(head_bytes);
    out.extend_from_slice(b"\n");
    out.extend_from_slice(
        format!("...truncated ({spilled_extra} more bytes spilled to {spill_name})").as_bytes(),
    );
    out.extend_from_slice(b"\n");
    Ok((out, true))
}

/// Run one headless session turn: stream prompt, render answer + attachments.
pub fn run(
    session: &str,
    prompt: &RunPrompt,
    out: &mut dyn RunSink,
    opts: &RunOpts,
    model: &mut dyn ModelPort,
    files: &dyn FilePort,
) -> RunExit {
    if session.is_empty() || session.chars().count() > MAX_SESSION_CHARS {
        return usage("invalid session");
    }
    if prompt.text.is_empty() || prompt.text.chars().count() > MAX_PROMPT_CHARS {
        return usage("invalid prompt");
    }
    // Validate + convert attachments before any model call or sink byte.
    let mut parts: Vec<(String, String)> = Vec::with_capacity(prompt.attachments.len());
    for att in &prompt.attachments {
        match att {
            Attachment::Text { name, text } => {
                if !valid_name(name) {
                    return usage("invalid attachment name");
                }
                if text.as_bytes().len() > MAX_TEXT_BYTES {
                    return usage("attachment too large");
                }
                parts.push((name.clone(), text.clone()));
            }
            Attachment::File { path } => {
                if !valid_name(path) {
                    return usage("invalid attachment path");
                }
                match files.read(path) {
                    Some(bytes) => {
                        if bytes.len() > MAX_TEXT_BYTES {
                            return usage("attachment too large");
                        }
                        let text = String::from_utf8_lossy(&bytes).into_owned();
                        parts.push((path.clone(), text));
                    }
                    None => return usage("attachment not found"),
                }
            }
        }
    }
    // Stream prompt uncompressed; no gzip/deflate framing ever.
    let answer = match model.complete(prompt.text.as_bytes()) {
        Ok(a) => a,
        Err(ModelError::Refused) => return usage("model refused"),
        Err(ModelError::Failed) => return internal("model failed"),
    };
    // Render everything into a staging buffer: failures emit zero sink bytes.
    let mut staged: Vec<u8> = Vec::new();
    match preview_or_spill(&answer, session, "answer", &opts.spill_dir) {
        Ok((head, _)) => {
            let rendered = match opts.renderer {
                Renderer::Inline => {
                    let mut v = head;
                    if v.last() != Some(&b'\n') {
                        v.push(b'\n');
                    }
                    v
                }
                Renderer::Block => {
                    let mut v = Vec::with_capacity(head.len() + 16);
                    v.extend_from_slice(b"```\n");
                    v.extend_from_slice(&head);
                    if v.last() != Some(&b'\n') {
                        v.push(b'\n');
                    }
                    v.extend_from_slice(b"```\n");
                    v
                }
            };
            staged.extend_from_slice(&rendered);
        }
        Err(e) => return e,
    }
    for (i, (name, text)) in parts.iter().enumerate() {
        let tag = format!("attach{i}");
        match preview_or_spill(text, session, &tag, &opts.spill_dir) {
            Ok((head, _)) => {
                staged.extend_from_slice(b"--- ");
                staged.extend_from_slice(name.as_bytes());
                staged.extend_from_slice(b"\n");
                staged.extend_from_slice(&head);
                if staged.last() != Some(&b'\n') {
                    staged.push(b'\n');
                }
            }
            Err(e) => return e,
        }
    }
    out.push(&staged);
    ok("ok")
}
