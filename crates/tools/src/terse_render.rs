//! Terse response renderer (TOOL-022 / CV-SLICE-01).
//!
//! Pure presenter over caller text: compresses agent responses into
//! `thing-action-reason-next-step` fragments at increasing intensities.
//! Renderer-only: never mutates, stores, persists, or reorders caller data.
//! No I/O, no clock, no network, no global state. Single pass, O(n) time,
//! output length always `<=` input length.
//!
//! Fail-closed presenter guard (bypass list): lines tagged `SECURITY:` /
//! `CONFIRM:` and ordered `1. 2. 3.` sequences render verbatim, never
//! compressed.

#![forbid(unsafe_code)]

/// Render intensity. Higher levels compress more; all levels keep
/// identifiers, numbers, paths, commands, errors, and URLs byte-exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intensity {
    Lite,
    Full,
    Ultra,
}

/// Oversize passthrough threshold: inputs longer than this render unchanged
/// with no compression allocation.
pub const MAX_INPUT_BYTES: usize = 1_048_576;

/// Status/filler phrases stripped at every intensity (exact substrings).
const FILLER: &[&str] = &[
    "Sure!",
    "Of course!",
    "Of course.",
    "I'd be happy to help.",
    "I would be happy to help.",
    "Certainly!",
    "Great question!",
    "Hope this helps!",
    "Let me know if you need anything else.",
    "Is there anything else I can help with?",
];

/// Standalone conjunctions dropped at Full and above (lowercase core word).
const CONJUNCTIONS: &[&str] = &["and", "but", "or", "so", "yet", "nor"];

/// Function words dropped only at Ultra (lowercase core word).
/// Content nouns/verbs/adjectives/adverbs are never in this set.
const ULTRA_STOP: &[&str] = &[
    "the", "a", "an", "uses", "use", "used", "using", "is", "are", "was", "were", "be", "been",
    "being", "has", "have", "had", "do", "does", "did", "it", "its", "this", "that", "these",
    "those", "with", "of", "to", "in", "on", "at", "by", "from", "for", "as", "i", "you", "we",
    "they", "he", "she", "my", "your", "our", "please",
];

/// Terse renderer. Default: enabled at [`Intensity::Full`].
#[derive(Debug, Clone)]
pub struct TerseRenderer {
    enabled: bool,
    intensity: Intensity,
}

impl TerseRenderer {
    /// Default renderer: enabled, [`Intensity::Full`].
    pub fn new() -> Self {
        Self {
            enabled: true,
            intensity: Intensity::Full,
        }
    }

    /// Opt-out toggle. Disabled `render` returns input unchanged.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Switch intensity; affects subsequent `render` calls only.
    pub fn set_intensity(&mut self, intensity: Intensity) {
        self.intensity = intensity;
    }

    /// Current intensity.
    pub fn intensity(&self) -> Intensity {
        self.intensity
    }

    /// Whether compression is enabled.
    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Render `input` at the current level. Pure function of
    /// `(enabled, intensity, input)`; never mutates caller data.
    pub fn render(&self, input: &str) -> String {
        // Zero cost when off: single branch, clone only.
        if !self.enabled {
            return input.to_string();
        }
        if input.is_empty() {
            return String::new();
        }
        // Oversize: passthrough, no compression allocation.
        if input.len() > MAX_INPUT_BYTES {
            return input.to_string();
        }
        // Fail-closed bypass list: guarded lines render verbatim.
        if is_guarded(input) {
            return input.to_string();
        }
        match self.intensity {
            Intensity::Lite => strip_filler(input),
            Intensity::Full => compress(input, false),
            Intensity::Ultra => compress(input, true),
        }
    }
}

impl Default for TerseRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Fail-closed presenter guard: tagged warnings/confirmations and ordered
/// multi-step sequences must render verbatim, never compressed.
fn is_guarded(input: &str) -> bool {
    let mut expected = 1u32;
    let mut numbered = 0u32;
    for line in input.lines() {
        let t = line.trim_start();
        if t.starts_with("SECURITY:") || t.starts_with("CONFIRM:") {
            return true;
        }
        if let Some(n) = leading_step(t) {
            if n == expected {
                expected += 1;
                numbered += 1;
            }
        }
    }
    numbered >= 3
}

/// Leading `N.` ordered-step marker on one line, if present.
fn leading_step(line: &str) -> Option<u32> {
    let dots = line.find('.')?;
    let (num, _) = line.split_at(dots);
    if num.is_empty() || !num.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    num.parse::<u32>().ok()
}

/// Lite: strip filler/status phrases, collapse whitespace, keep sentences.
fn strip_filler(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    out.push_str(input);
    for f in FILLER {
        while let Some(pos) = out.find(f) {
            out.replace_range(pos..pos + f.len(), "");
        }
    }
    collapse(&out)
}

/// Full/Ultra: filler strip, then word filtering. Ultra also drops
/// function words. Protected tokens (identifiers, numbers, paths,
/// commands, errors, URLs) always survive byte-exact.
fn compress(input: &str, ultra: bool) -> String {
    let lite = strip_filler(input);
    let mut words: Vec<&str> = Vec::new();
    for tok in lite.split_whitespace() {
        if is_protected(tok) {
            words.push(tok);
            continue;
        }
        let core = core_word(tok);
        if CONJUNCTIONS.contains(&core.as_str()) {
            continue;
        }
        if ultra && ULTRA_STOP.contains(&core.as_str()) {
            continue;
        }
        words.push(tok);
    }
    // Removal-only: never longer than the filler-stripped input.
    let joined = words.join(" ");
    debug_assert!(joined.len() <= lite.len());
    if joined.is_empty() { lite } else { joined }
}

/// Tokens that must survive compression byte-exact: code spans, paths,
/// URLs, scoped names, numbers/units, error codes, snake_case idents.
fn is_protected(tok: &str) -> bool {
    tok.contains('`')
        || tok.contains("::")
        || tok.contains('/')
        || tok.contains('\\')
        || tok.contains('_')
        || tok.bytes().any(|b| b.is_ascii_digit())
}

/// Lowercase alphanumeric core of a token for stopword comparison.
/// Strips leading/trailing punctuation so `"and,"` matches `"and"`
/// while `"reuse"` never matches `"use"`.
fn core_word(tok: &str) -> String {
    let core = tok.trim_matches(|c: char| !c.is_alphanumeric());
    core.to_lowercase()
}

/// Collapse runs of whitespace to one space, trim ends, drop spaces
/// left before punctuation by filler removal. Removal-only.
fn collapse(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            pending_space = true;
        } else {
            if pending_space && !out.is_empty() && !is_closing_punct(ch) {
                out.push(' ');
            }
            pending_space = false;
            out.push(ch);
        }
    }
    debug_assert!(out.len() <= s.len());
    out
}

/// Punctuation that must hug the previous word after filler removal.
fn is_closing_punct(ch: char) -> bool {
    matches!(ch, '.' | ',' | '!' | '?' | ':' | ';' | ')' | ']' | '}')
}
